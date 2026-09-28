//! Native page sets for the M18.
//!
//! Pages deliberately sit above the existing profile store. A page owns one
//! profile layout, but page navigation is a first-class M18 concept rather
//! than an action-plugin request to switch an arbitrary OpenDeck profile.

use crate::shared::config_dir;
use crate::store::{NotProfile, Store};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, command};
use tokio::{sync::Mutex, time::Instant};

/// Serialises every read-modify-write of a page set. Page changes arrive from
/// M18 keys (one worker per key), the editor, and the application watcher at
/// the same time; without this, two of them could interleave and save a page
/// selection that no longer matches the profile shown on the device.
///
/// Lock order: this lock is always taken before the profile store locks.
static PAGE_LOCK: Mutex<()> = Mutex::const_new(());

const MAX_PAGE_NAME_LENGTH: usize = 40;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct M18Page {
	pub id: String,
	/// A custom label. Empty, or a plain number from older versions and VSD
	/// Craft imports, means the editor shows the page's position instead.
	pub name: String,
	pub profile: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct M18PageSet {
	pub pages: Vec<M18Page>,
	pub selected: usize,
	#[serde(default)]
	pub folder_history: Vec<usize>,
}

impl NotProfile for M18PageSet {}

fn store_id(device: &str) -> String {
	format!("m18-pages-{device}")
}

fn default_page_set() -> M18PageSet {
	M18PageSet {
		pages: vec![M18Page {
			id: "default".to_owned(),
			name: "1".to_owned(),
			profile: "Default".to_owned(),
		}],
		selected: 0,
		folder_history: vec![],
	}
}

fn push_folder_history(history: &mut Vec<usize>, current: usize, destination: usize) {
	if current != destination {
		history.push(current);
	}
}

fn pop_folder_history(history: &mut Vec<usize>, page_count: usize) -> Option<usize> {
	history.pop().filter(|index| *index < page_count)
}

fn load(device: &str) -> Result<Store<M18PageSet>, anyhow::Error> {
	let mut store = Store::new(&store_id(device), &config_dir(), default_page_set())?;
	if store.value.pages.is_empty() {
		store.value = default_page_set();
		store.save()?;
	}
	if store.value.selected >= store.value.pages.len() {
		store.value.selected = 0;
		store.save()?;
	}
	if store.value.folder_history.iter().any(|index| *index >= store.value.pages.len()) {
		store.value.folder_history.retain(|index| *index < store.value.pages.len());
		store.save()?;
	}
	Ok(store)
}

pub fn get(device: &str) -> Result<M18PageSet, anyhow::Error> {
	Ok(load(device)?.value)
}

pub async fn replace(device: &str, pages: Vec<M18Page>) -> Result<M18PageSet, anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	let previous_profile = store.value.pages.get(store.value.selected).map(|page| page.profile.clone());
	store.value.pages = if pages.is_empty() { default_page_set().pages } else { pages };
	store.value.selected = previous_profile.and_then(|profile| store.value.pages.iter().position(|page| page.profile == profile)).unwrap_or(0);
	store.value.folder_history.clear();
	store.save()?;
	crate::key_images::forget(device, None, None).await;
	Ok(store.value)
}

/// Record that `profile` was selected outside page navigation (for example by
/// a plugin), so the page bar and page numbers follow it.
pub async fn sync_selected_profile(device: &str, profile: &str) -> Result<(), anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	if let Some(index) = store.value.pages.iter().position(|page| page.profile == profile)
		&& store.value.selected != index
	{
		store.value.selected = index;
		store.save()?;
		emit(device, &store.value);
	}
	Ok(())
}

fn emit(device: &str, page_set: &M18PageSet) {
	if let Some(app) = crate::APP_HANDLE.get()
		&& let Err(error) = app.emit("m18_pages_changed", serde_json::json!({ "device": device, "pageSet": page_set }))
	{
		log::warn!("Failed to announce M18 page changes: {error}");
	}
}

async fn show_profile(device: &str, profile: &str, requested_at: Instant, origin: crate::m18::PageTurnOrigin) -> Result<(), anyhow::Error> {
	crate::events::frontend::profiles::select_profile_for_page(device, profile, requested_at, origin).await
}

pub async fn switch_to(device: &str, target: Option<&str>, index: Option<usize>, delta: isize) -> Result<(), anyhow::Error> {
	switch_to_with_origin(device, target, index, delta, crate::m18::PageTurnOrigin::Other).await
}

pub async fn switch_to_with_origin(device: &str, target: Option<&str>, index: Option<usize>, delta: isize, origin: crate::m18::PageTurnOrigin) -> Result<(), anyhow::Error> {
	switch_to_with_origin_at(device, target, index, delta, origin, Instant::now()).await
}

pub async fn switch_to_with_origin_at(device: &str, target: Option<&str>, index: Option<usize>, delta: isize, origin: crate::m18::PageTurnOrigin, requested_at: Instant) -> Result<(), anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	let current = store.value.selected;
	let destination = if let Some(target) = target.filter(|target| !target.is_empty()) {
		store.value.pages.iter().position(|page| page.id == target || page.profile == target).unwrap_or(current)
	} else if let Some(index) = index {
		index.min(store.value.pages.len().saturating_sub(1))
	} else {
		let count = store.value.pages.len() as isize;
		if count == 0 { 0 } else { (current as isize + delta).rem_euclid(count) as usize }
	};

	let page = store.value.pages.get(destination).cloned().ok_or_else(|| anyhow::anyhow!("M18 page set is empty"))?;
	if destination != current {
		log::info!("M18 page switch requested: origin={}, target_profile={}", origin.label(), page.profile);
		show_profile(device, &page.profile, requested_at, origin).await?;
	}
	store.value.selected = destination;
	store.save()?;
	emit(device, &store.value);
	Ok(())
}

/// Show `profile` on the device: as a page switch when it belongs to the page
/// set, otherwise as a plain profile switch (for example a plugin's target).
pub async fn switch_to_profile(device: &str, profile: &str) -> Result<(), anyhow::Error> {
	if get(device)?.pages.iter().any(|page| page.profile == profile || page.id == profile) {
		return switch_to(device, Some(profile), None, 0).await;
	}
	let _guard = PAGE_LOCK.lock().await;
	show_profile(device, profile, Instant::now(), crate::m18::PageTurnOrigin::Other).await
}

/// Enter an imported folder target, remembering the current page for Go back.
pub async fn open_folder_at(device: &str, target: &str, requested_at: Instant) -> Result<(), anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	let Some(destination) = store.value.pages.iter().position(|page| page.id == target || page.profile == target) else {
		log::warn!("M18 folder target is not present in this device's page set");
		return Ok(());
	};
	let current = store.value.selected;
	if current == destination {
		return Ok(());
	}
	push_folder_history(&mut store.value.folder_history, current, destination);
	let page = store.value.pages[destination].clone();
	show_profile(device, &page.profile, requested_at, crate::m18::PageTurnOrigin::Folder).await?;
	store.value.selected = destination;
	store.save()?;
	emit(device, &store.value);
	Ok(())
}

/// Return to the page that opened the current folder, if there is one.
pub async fn go_back_at(device: &str, requested_at: Instant) -> Result<(), anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	let Some(destination) = pop_folder_history(&mut store.value.folder_history, store.value.pages.len()) else {
		return Ok(());
	};
	let page = store.value.pages[destination].clone();
	show_profile(device, &page.profile, requested_at, crate::m18::PageTurnOrigin::Folder).await?;
	store.value.selected = destination;
	store.save()?;
	emit(device, &store.value);
	Ok(())
}

/// The first profile ID from `candidates` that the device does not use yet.
fn unused_profile_id(device: &str, candidates: impl IntoIterator<Item = String>) -> Result<String, anyhow::Error> {
	let existing = crate::store::profiles::get_device_profiles(device)?;
	candidates
		.into_iter()
		.find(|candidate| !existing.iter().any(|profile| profile.eq_ignore_ascii_case(candidate)))
		.ok_or_else(|| anyhow::anyhow!("No free page name is available"))
}

fn clean_page_name(name: &str) -> String {
	name.trim().chars().filter(|character| !character.is_control()).take(MAX_PAGE_NAME_LENGTH).collect()
}

/// Add an empty page after the last one and show it.
pub async fn add_page(device: &str) -> Result<M18PageSet, anyhow::Error> {
	let requested_at = Instant::now();
	let _guard = PAGE_LOCK.lock().await;
	let device_info = crate::store::profiles::device_info(device).map_err(|_| anyhow::anyhow!("Connect the M18 to add a page"))?;
	let mut store = load(device)?;
	let profile = unused_profile_id(device, (store.value.pages.len() + 1..).map(|number| format!("Page {number}")))?;
	{
		// Creating the store writes an empty profile for the new page.
		let mut locks = crate::store::profiles::acquire_locks_mut().await;
		locks.profile_stores.get_profile_store_mut(&device_info, &profile).await?;
	}
	store.value.pages.push(M18Page {
		id: profile.clone(),
		name: String::new(),
		profile: profile.clone(),
	});
	let destination = store.value.pages.len() - 1;
	show_profile(device, &profile, requested_at, crate::m18::PageTurnOrigin::Other).await?;
	store.value.selected = destination;
	store.value.folder_history.clear();
	store.save()?;
	emit(device, &store.value);
	Ok(store.value)
}

pub async fn rename_page(device: &str, index: usize, name: &str) -> Result<M18PageSet, anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	let page = store.value.pages.get_mut(index).ok_or_else(|| anyhow::anyhow!("That page no longer exists"))?;
	page.name = clean_page_name(name);
	store.save()?;
	emit(device, &store.value);
	Ok(store.value)
}

/// Copy a page, with its keys and artwork, and insert the copy after it.
pub async fn duplicate_page(device: &str, index: usize) -> Result<M18PageSet, anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let device_info = crate::store::profiles::device_info(device).map_err(|_| anyhow::anyhow!("Connect the M18 to duplicate a page"))?;
	let mut store = load(device)?;
	let source = store.value.pages.get(index).cloned().ok_or_else(|| anyhow::anyhow!("That page no longer exists"))?;
	// Copy the latest edits, not the last autosave.
	crate::store::profiles::flush_stale_profiles().await?;
	let profile = unused_profile_id(
		device,
		(1..).map(|number| {
			if number == 1 {
				format!("{} copy", source.profile)
			} else {
				format!("{} copy {number}", source.profile)
			}
		}),
	)?;
	{
		let mut locks = crate::store::profiles::acquire_locks_mut().await;
		locks.profile_stores.rename_profile(&device_info, &source.profile, &profile, true).await?;
	}
	let name = if source.name.trim().is_empty() || source.name.trim().parse::<usize>().is_ok() {
		String::new()
	} else {
		clean_page_name(&format!("{} copy", source.name.trim()))
	};
	store.value.pages.insert(index + 1, M18Page { id: profile.clone(), name, profile });
	if store.value.selected > index {
		store.value.selected += 1;
	}
	store.value.folder_history.clear();
	store.save()?;
	emit(device, &store.value);
	Ok(store.value)
}

/// Remove a page and delete its keys. The last page cannot be removed.
pub async fn delete_page(device: &str, index: usize) -> Result<M18PageSet, anyhow::Error> {
	let requested_at = Instant::now();
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	if store.value.pages.len() <= 1 {
		return Err(anyhow::anyhow!("The M18 needs at least one page"));
	}
	let removed = store.value.pages.get(index).cloned().ok_or_else(|| anyhow::anyhow!("That page no longer exists"))?;
	let selected_profile = if index == store.value.selected {
		// Show a neighbouring page before this one disappears.
		let fallback = if index + 1 < store.value.pages.len() { index + 1 } else { index - 1 };
		let fallback_profile = store.value.pages[fallback].profile.clone();
		show_profile(device, &fallback_profile, requested_at, crate::m18::PageTurnOrigin::Other).await?;
		fallback_profile
	} else {
		store.value.pages[store.value.selected].profile.clone()
	};
	store.value.pages.remove(index);
	store.value.selected = store.value.pages.iter().position(|page| page.profile == selected_profile).unwrap_or(0);
	store.value.folder_history.clear();
	store.save()?;

	if !store.value.pages.iter().any(|page| page.profile == removed.profile) {
		crate::store::profiles::PROFILE_STORES.write().await.delete_profile(device, &removed.profile);
		crate::application_watcher::forget_profile(device, &removed.profile).await;
	}
	emit(device, &store.value);
	Ok(store.value)
}

/// Move a page to another position, keeping the current page selected.
pub async fn move_page(device: &str, from: usize, to: usize) -> Result<M18PageSet, anyhow::Error> {
	let _guard = PAGE_LOCK.lock().await;
	let mut store = load(device)?;
	if from >= store.value.pages.len() {
		return Err(anyhow::anyhow!("That page no longer exists"));
	}
	let selected_profile = store.value.pages[store.value.selected].profile.clone();
	let page = store.value.pages.remove(from);
	let to = to.min(store.value.pages.len());
	store.value.pages.insert(to, page);
	store.value.selected = store.value.pages.iter().position(|page| page.profile == selected_profile).unwrap_or(0);
	// Folder history stores positions, which no longer mean the same pages.
	store.value.folder_history.clear();
	store.save()?;
	emit(device, &store.value);
	Ok(store.value)
}

#[command]
pub fn get_m18_pages(device: String) -> Result<M18PageSet, crate::events::frontend::Error> {
	get(&device).map_err(Into::into)
}

#[command]
pub async fn switch_m18_page(device: String, page: String) -> Result<(), crate::events::frontend::Error> {
	switch_to_with_origin(&device, Some(&page), None, 0, crate::m18::PageTurnOrigin::PageTab).await.map_err(Into::into)
}

#[command]
pub async fn switch_m18_page_index(device: String, index: usize) -> Result<(), crate::events::frontend::Error> {
	switch_to_with_origin(&device, None, Some(index), 0, crate::m18::PageTurnOrigin::PageTab).await.map_err(Into::into)
}

#[command]
pub async fn add_m18_page(device: String) -> Result<M18PageSet, crate::events::frontend::Error> {
	add_page(&device).await.map_err(Into::into)
}

#[command]
pub async fn rename_m18_page(device: String, index: usize, name: String) -> Result<M18PageSet, crate::events::frontend::Error> {
	rename_page(&device, index, &name).await.map_err(Into::into)
}

#[command]
pub async fn duplicate_m18_page(device: String, index: usize) -> Result<M18PageSet, crate::events::frontend::Error> {
	duplicate_page(&device, index).await.map_err(Into::into)
}

#[command]
pub async fn delete_m18_page(device: String, index: usize) -> Result<M18PageSet, crate::events::frontend::Error> {
	delete_page(&device, index).await.map_err(Into::into)
}

#[command]
pub async fn move_m18_page(device: String, from: usize, to: usize) -> Result<M18PageSet, crate::events::frontend::Error> {
	move_page(&device, from, to).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
	use super::{clean_page_name, pop_folder_history, push_folder_history};

	#[test]
	fn negative_page_delta_wraps_without_underflow() {
		assert_eq!((0isize - 1).rem_euclid(2), 1);
	}

	#[test]
	fn folder_navigation_remembers_nested_parent_pages_in_lifo_order() {
		let mut history = vec![];
		push_folder_history(&mut history, 0, 2);
		push_folder_history(&mut history, 2, 4);
		assert_eq!(pop_folder_history(&mut history, 5), Some(2));
		assert_eq!(pop_folder_history(&mut history, 5), Some(0));
		assert_eq!(pop_folder_history(&mut history, 5), None);
	}

	#[test]
	fn page_names_are_trimmed_single_line_and_bounded() {
		assert_eq!(clean_page_name("  Media  "), "Media");
		assert_eq!(clean_page_name("Line\nbreak"), "Linebreak");
		assert_eq!(clean_page_name(&"x".repeat(100)).chars().count(), 40);
	}
}
