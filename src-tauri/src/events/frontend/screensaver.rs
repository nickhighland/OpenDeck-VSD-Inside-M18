use std::fs;
use std::path::{Path, PathBuf};

use tauri::command;

use super::Error;

fn media_extension(path: &Path, fallback: &str) -> String {
	path.extension()
		.and_then(|extension| extension.to_str())
		.filter(|extension| !extension.is_empty() && extension.chars().all(|character| character.is_ascii_alphanumeric()))
		.unwrap_or(fallback)
		.to_ascii_lowercase()
}

fn copy_selected_file(source: &Path, destination: &Path) -> Result<(), anyhow::Error> {
	if !source.is_file() {
		return Err(anyhow::anyhow!("Selected media file does not exist: {}", source.display()));
	}
	if let Some(parent) = destination.parent() {
		fs::create_dir_all(parent)?;
	}
	fs::copy(source, destination)?;
	Ok(())
}

#[command]
pub fn import_screensaver_video(path: String) -> Result<String, Error> {
	let source = PathBuf::from(path);
	let extension = media_extension(&source, "mp4");
	let directory = crate::shared::config_dir().join("screensaver").join("video");
	fs::create_dir_all(&directory)?;

	for entry in fs::read_dir(&directory)? {
		let entry = entry?;
		if entry.path().is_file() {
			let _ = fs::remove_file(entry.path());
		}
	}

	let destination = directory.join(format!("source.{extension}"));
	copy_selected_file(&source, &destination)?;
	Ok(destination.to_string_lossy().into_owned())
}

#[command]
pub fn import_screensaver_photos(paths: Vec<String>) -> Result<Vec<String>, Error> {
	if paths.is_empty() {
		return Ok(Vec::new());
	}

	let directory = crate::shared::config_dir().join("screensaver").join("photos").join("current");
	if directory.exists() {
		fs::remove_dir_all(&directory)?;
	}
	fs::create_dir_all(&directory)?;

	let mut imported = Vec::with_capacity(paths.len());
	for (index, path) in paths.iter().enumerate() {
		let source = PathBuf::from(path);
		let extension = media_extension(&source, "jpg");
		let destination = directory.join(format!("{index:04}.{extension}"));
		copy_selected_file(&source, &destination)?;
		imported.push(destination.to_string_lossy().into_owned());
	}

	Ok(imported)
}

#[command]
pub async fn set_screensaver_frame(device: String, background: String, images: Vec<String>) -> Result<(), Error> {
	if !crate::screensaver::is_active(&device) {
		return Ok(());
	}
	if images.len() != 15 {
		return Err(anyhow::anyhow!("An M18 screensaver frame must contain exactly 15 LCD images").into());
	}

	crate::m18::set_screensaver_frame(&device, background, images).await?;
	Ok(())
}

#[command]
pub fn get_active_screensavers() -> Vec<String> {
	crate::screensaver::active_devices()
}

#[command]
pub async fn stop_screensaver(device: String) {
	crate::screensaver::stop_device(&device).await;
}
