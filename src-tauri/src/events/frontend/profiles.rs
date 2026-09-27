use super::Error;

use crate::shared::DEVICES;
use crate::store::profiles::{PROFILE_STORES, acquire_locks_mut, get_device_profiles, save_profile_now};

use tauri::{AppHandle, Emitter, command};

#[command]
pub fn get_profiles(device: &str) -> Result<Vec<String>, Error> {
	Ok(get_device_profiles(device)?)
}

#[command]
pub async fn get_selected_profile(device: String) -> Result<crate::shared::Profile, Error> {
	let mut locks = acquire_locks_mut().await;
	let Some(device_info) = DEVICES.get(&device).map(|entry| entry.value().clone()) else {
		return Err(Error::new(format!("device {device} not found")));
	};

	let selected_profile = locks.device_stores.get_selected_profile(&device)?;
	// Load the store if needed: a page that was never opened has no store yet.
	let profile = locks.profile_stores.get_profile_store_mut(&device_info, &selected_profile).await?;

	Ok(profile.value.clone())
}

/// Instances that receive appear/disappear events for a slot: composite
/// parents (Multi Action, Cycle, Carousel) are drawn by the editor, and only
/// their children belong to plugins.
fn event_targets(instance: &crate::shared::ActionInstance) -> Vec<&crate::shared::ActionInstance> {
	match instance.children.as_ref() {
		Some(children) if matches!(instance.action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction" | "opendeck.carouselaction") => children.iter().collect(),
		_ => vec![instance],
	}
}

#[command]
pub async fn set_selected_profile(device: String, id: String) -> Result<(), Error> {
	select_profile(&device, &id).await?;
	if crate::m18::is_m18(&device) {
		crate::m18_pages::sync_selected_profile(&device, &id).await?;
	}
	Ok(())
}

/// Make `id` the device's active profile and render it. The M18 page set is
/// not touched here: page navigation calls this while holding the page lock.
#[allow(clippy::flat_map_identity)]
pub async fn select_profile(device: &str, id: &str) -> Result<(), anyhow::Error> {
	let mut locks = acquire_locks_mut().await;
	// Clone the device info instead of holding a registry guard across awaits.
	let device_info = DEVICES.get(device).map(|entry| entry.value().clone()).ok_or_else(|| anyhow::anyhow!("device {device} not found"))?;

	// If a profile save is pending for this device, save it immediately to prevent losing profile data
	if let Err(error) = save_profile_now(device, &mut locks).await {
		log::error!("Failed to save profile for device {device}: {error}");
	}

	let selected_profile = locks.device_stores.get_selected_profile(device)?;
	let switching = selected_profile != id;

	if switching {
		let old_profile = &locks.profile_stores.get_profile_store(&device_info, &selected_profile)?.value;
		for instance in old_profile
			.keys
			.iter()
			.flatten()
			.chain(old_profile.sliders.iter().flatten())
			.chain(old_profile.infobars.iter().flatten())
		{
			for target in event_targets(instance) {
				let _ = crate::events::outbound::will_appear::will_disappear(target, false).await;
			}
		}
	}

	// We must use the mutable version of get_profile_store in order to create the store if it does not exist.
	let store = locks.profile_stores.get_profile_store_mut(&device_info, id).await?;
	let new_profile = &store.value;
	if switching {
		// The old page stays up until the new page's keys are drawn, then they
		// all appear in one update, instead of the screen going blank and
		// filling in over several.
		let positions: Vec<u8> = new_profile.keys.iter().enumerate().filter(|(_, key)| key.is_some()).map(|(position, _)| position as u8).collect();
		let _ = crate::events::outbound::devices::begin_page(device, positions).await;
	}
	for instance in new_profile
		.keys
		.iter()
		.flatten()
		.chain(new_profile.sliders.iter().flatten())
		.chain(new_profile.infobars.iter().flatten())
	{
		for target in event_targets(instance) {
			let _ = crate::events::outbound::will_appear::will_appear(target).await;
		}
	}
	store.save()?;

	locks.device_stores.set_selected_profile(device, id.to_owned())?;
	Ok(())
}

#[command]
pub async fn delete_profile(device: String, profile: String) {
	let mut profile_stores = PROFILE_STORES.write().await;
	profile_stores.delete_profile(&device, &profile);
}

#[command]
pub async fn rename_profile(device: String, old_id: String, new_id: String, retain: bool) -> Result<(), Error> {
	let mut locks = acquire_locks_mut().await;
	if !DEVICES.contains_key(&device) {
		return Err(Error::new(format!("device {device} not found")));
	}

	locks.profile_stores.rename_profile(&DEVICES.get(&device).unwrap(), &old_id, &new_id, retain).await?;

	Ok(())
}

pub async fn rerender_images(app: &AppHandle) -> Result<(), anyhow::Error> {
	app.emit("rerender_images", ())?;
	Ok(())
}
