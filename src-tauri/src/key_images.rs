//! Finished key images, saved for page turns.
//!
//! Whenever the editor draws a key, on any page, the core converts the image
//! to the M18's format (a 64×64 JPEG, rotated for the panel) and keeps it
//! here, in memory and in the app's cache folder. A page turn then shows every
//! key at once from these images instead of waiting for the editor to draw the
//! page again, so it is as quick as the USB transfer.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use tauri::Manager;
use tokio::sync::RwLock;

type Key = (String, String, u8);

static IMAGES: LazyLock<RwLock<HashMap<Key, Arc<Vec<u8>>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// `v1` changes whenever the M18 image format does, so old images are not used.
fn device_dir(device: &str) -> Option<PathBuf> {
	let cache = crate::APP_HANDLE.get()?.path().app_cache_dir().ok()?;
	Some(cache.join("key-images-v1").join(device))
}

/// Profile IDs can contain `/`, so directories use their bytes in hex.
fn profile_dir(device: &str, profile: &str) -> Option<PathBuf> {
	let name: String = profile.bytes().map(|byte| format!("{byte:02x}")).collect();
	Some(device_dir(device)?.join(name))
}

fn image_path(device: &str, profile: &str, position: u8) -> Option<PathBuf> {
	Some(profile_dir(device, profile)?.join(format!("{position}.jpg")))
}

/// Keep a key's finished image.
pub async fn remember(device: &str, profile: &str, position: u8, image: Arc<Vec<u8>>) {
	let key = (device.to_owned(), profile.to_owned(), position);
	if IMAGES.read().await.get(&key).is_some_and(|saved| saved == &image) {
		return;
	}
	IMAGES.write().await.insert(key, image.clone());
	if let Some(path) = image_path(device, profile, position) {
		// Written to a temporary file first so a page turn never reads half an image.
		tokio::task::spawn_blocking(move || {
			let temporary = path.with_extension("tmp");
			let written = path
				.parent()
				.map_or(Ok(()), std::fs::create_dir_all)
				.and_then(|_| std::fs::write(&temporary, image.as_slice()))
				.and_then(|_| std::fs::rename(&temporary, &path));
			if let Err(error) = written {
				log::debug!("Failed to save a key image to {}: {error}", path.display());
			}
		});
	}
}

/// A key's finished image, if the editor has drawn it before.
pub async fn saved(device: &str, profile: &str, position: u8) -> Option<Arc<Vec<u8>>> {
	let key = (device.to_owned(), profile.to_owned(), position);
	if let Some(image) = IMAGES.read().await.get(&key) {
		return Some(image.clone());
	}
	let image = Arc::new(tokio::fs::read(image_path(device, profile, position)?).await.ok()?);
	IMAGES.write().await.insert(key, image.clone());
	Some(image)
}

/// Load several keys for a page in parallel. Page turns are serialized by the
/// output worker, but cache misses are independent disk reads and must not
/// make the page-start path wait on them one at a time.
pub async fn saved_many(device: &str, profile: &str, positions: impl IntoIterator<Item = u8>) -> Vec<(u8, Arc<Vec<u8>>)> {
	let reads = positions
		.into_iter()
		.map(|position| async move { saved(device, profile, position).await.map(|image| (position, image)) });
	let mut images = futures::future::join_all(reads).await.into_iter().flatten().collect::<Vec<_>>();
	images.sort_unstable_by_key(|(position, _)| *position);
	images
}

/// Drop saved images: of one key, of a whole page (`position` is `None`), or
/// of every page of a device (`profile` is `None`).
pub async fn forget(device: &str, profile: Option<&str>, position: Option<u8>) {
	IMAGES.write().await.retain(|(saved_device, saved_profile, saved_position), _| {
		!(saved_device == device && profile.is_none_or(|profile| saved_profile == profile) && position.is_none_or(|position| *saved_position == position))
	});
	let target = match (profile, position) {
		(Some(profile), Some(position)) => image_path(device, profile, position),
		(Some(profile), None) => profile_dir(device, profile),
		(None, _) => device_dir(device),
	};
	if let Some(target) = target {
		let _ = tokio::task::spawn_blocking(move || if target.is_dir() { std::fs::remove_dir_all(&target) } else { std::fs::remove_file(&target) }).await;
	}
}

/// Drop every saved image, for example after the key images' rotation changes.
pub async fn forget_all() {
	IMAGES.write().await.clear();
	if let Some(root) = crate::APP_HANDLE.get().and_then(|app| app.path().app_cache_dir().ok()).map(|cache| cache.join("key-images-v1")) {
		let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(root)).await;
	}
}
