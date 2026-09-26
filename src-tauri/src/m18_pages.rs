//! Native page sets for the M18.
//!
//! Pages deliberately sit above the existing profile store. A page owns one
//! profile layout, but page navigation is a first-class M18 concept rather
//! than an action-plugin request to switch an arbitrary OpenDeck profile.

use crate::shared::config_dir;
use crate::store::{NotProfile, Store};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, command};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct M18Page {
	pub id: String,
	pub name: String,
	pub profile: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct M18PageSet {
	pub pages: Vec<M18Page>,
	pub selected: usize,
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
	}
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
	Ok(store)
}

pub fn get(device: &str) -> Result<M18PageSet, anyhow::Error> {
	Ok(load(device)?.value)
}

pub fn replace(device: &str, pages: Vec<M18Page>) -> Result<M18PageSet, anyhow::Error> {
	let mut store = load(device)?;
	let previous_profile = store.value.pages.get(store.value.selected).map(|page| page.profile.clone());
	store.value.pages = if pages.is_empty() { default_page_set().pages } else { pages };
	store.value.selected = previous_profile.and_then(|profile| store.value.pages.iter().position(|page| page.profile == profile)).unwrap_or(0);
	store.save()?;
	Ok(store.value)
}

pub fn sync_selected_profile(device: &str, profile: &str) -> Result<(), anyhow::Error> {
	let mut store = load(device)?;
	if let Some(index) = store.value.pages.iter().position(|page| page.profile == profile)
		&& store.value.selected != index
	{
		store.value.selected = index;
		store.save()?;
	}
	Ok(())
}

fn emit(app: &AppHandle, device: &str, page_set: &M18PageSet) -> Result<(), anyhow::Error> {
	app.emit("m18_pages_changed", serde_json::json!({ "device": device, "pageSet": page_set }))?;
	Ok(())
}

pub async fn switch_to(device: &str, target: Option<&str>, index: Option<usize>, delta: isize) -> Result<(), anyhow::Error> {
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
		crate::events::frontend::profiles::set_selected_profile(device.to_owned(), page.profile.clone()).await?;
	}
	store.value.selected = destination;
	store.save()?;
	emit(crate::APP_HANDLE.get().unwrap(), device, &store.value)?;
	Ok(())
}

#[command]
pub fn get_m18_pages(device: String) -> Result<M18PageSet, crate::events::frontend::Error> {
	get(&device).map_err(Into::into)
}

#[command]
pub async fn switch_m18_page(device: String, page: String) -> Result<(), crate::events::frontend::Error> {
	switch_to(&device, Some(&page), None, 0).await.map_err(Into::into)
}

#[command]
pub async fn switch_m18_page_index(device: String, index: usize) -> Result<(), crate::events::frontend::Error> {
	switch_to(&device, None, Some(index), 0).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
	#[test]
	fn negative_page_delta_wraps_without_underflow() {
		assert_eq!((0isize - 1).rem_euclid(2), 1);
	}
}
