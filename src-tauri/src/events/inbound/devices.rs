use super::PayloadEvent;

use crate::shared::DEVICES;
use crate::store::profiles::get_device_profiles;

use serde::Deserialize;
use tokio::time::Instant;

pub async fn register_device(uuid: &str, mut event: PayloadEvent<crate::shared::DeviceInfo>) -> Result<(), anyhow::Error> {
	if uuid.is_empty() {
		if let Ok(profiles) = get_device_profiles(&event.payload.id) {
			let mut profile_stores = crate::store::profiles::PROFILE_STORES.write().await;
			for profile in profiles {
				// This is called to initialise the store for each profile when the device is registered.
				if let Err(e) = profile_stores.get_profile_store_mut(&event.payload, &profile).await {
					log::error!("{}", e);
				}
			}
		}

		event.payload.plugin = uuid.to_owned();
		let _ = crate::events::outbound::devices::device_did_connect(&event.payload.id, (&event.payload).into()).await;
		DEVICES.insert(event.payload.id.clone(), event.payload.clone());
		let _ = crate::device_sleep::apply_initial_device_sleep(&event.payload.id).await;
		crate::events::frontend::update_devices().await;

		let mut locks = crate::store::profiles::acquire_locks_mut().await;
		let selected_profile = locks.device_stores.get_selected_profile(&event.payload.id)?;
		let profile = locks.profile_stores.get_profile_store(&DEVICES.get(&event.payload.id).unwrap(), &selected_profile)?;
		// The first page appears in one update once the editor has drawn it.
		let positions: Vec<u8> = profile.value.keys.iter().enumerate().filter(|(_, key)| key.is_some()).map(|(position, _)| position as u8).collect();
		let _ = crate::events::outbound::devices::begin_page(&event.payload.id, &selected_profile, positions, None, crate::m18::PageTurnOrigin::Connect).await;
		for instance in profile
			.value
			.keys
			.iter()
			.flatten()
			.chain(profile.value.sliders.iter().flatten())
			.chain(profile.value.infobars.iter().flatten())
		{
			for target in crate::events::frontend::profiles::event_targets(instance) {
				let _ = crate::events::outbound::will_appear::will_appear(target).await;
			}
		}
		// The editor draws keys with their titles and built-in artwork; after
		// a reconnect it must draw them again even if nothing else changed.
		let _ = crate::events::frontend::profiles::rerender_images(crate::APP_HANDLE.get().unwrap()).await;

		use tauri_plugin_aptabase::EventTracker;
		let _ = crate::APP_HANDLE
			.get()
			.unwrap()
			.track_event("device_registered", Some(serde_json::json!({ "name": event.payload.name })));

		Ok(())
	} else {
		Err(anyhow::anyhow!("plugin {uuid} is not registered for device namespace {}", &event.payload.id[..2]))
	}
}

pub async fn deregister_device(uuid: &str, event: PayloadEvent<String>) -> Result<(), anyhow::Error> {
	if uuid.is_empty() {
		if !DEVICES.contains_key(&event.payload) {
			return Ok(());
		}

		let mut locks = crate::store::profiles::acquire_locks_mut().await;

		let selected_profile = locks.device_stores.get_selected_profile(&event.payload)?;
		let profile = locks.profile_stores.get_profile_store(&DEVICES.get(&event.payload).unwrap(), &selected_profile)?;
		for instance in profile
			.value
			.keys
			.iter()
			.flatten()
			.chain(profile.value.sliders.iter().flatten())
			.chain(profile.value.infobars.iter().flatten())
		{
			for target in crate::events::frontend::profiles::event_targets(instance) {
				let _ = crate::events::outbound::will_appear::will_disappear(target, false).await;
			}
		}

		// Flush any pending profile writes before removing the device.
		if let Err(error) = crate::store::profiles::save_profile_now(&event.payload, &mut locks).await {
			log::error!("Failed to flush profile for device {}: {error}", event.payload);
		}

		if let Ok(profiles) = get_device_profiles(&event.payload) {
			for profile in profiles {
				locks.profile_stores.remove_profile(&event.payload, &profile);
			}
		}

		drop(locks);

		let _ = crate::events::outbound::devices::device_did_disconnect(&event.payload).await;
		DEVICES.remove(&event.payload);
		crate::device_sleep::deregister_device(&event.payload);
		crate::events::frontend::update_devices().await;

		Ok(())
	} else {
		Err(anyhow::anyhow!("plugin {uuid} is not registered for device namespace {}", &event.payload[..2]))
	}
}

#[derive(Deserialize)]
pub struct PressPayload {
	pub device: String,
	pub position: u8,
}

pub async fn key_down(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	key_down_at(event, Instant::now()).await
}

/// Process a physical M18 press while preserving the HID-read timestamp for
/// page-turn latency instrumentation. Plugin-originated key events use the
/// wrapper above and start their trace when they enter the core.
pub async fn key_down_at(event: PayloadEvent<PressPayload>, requested_at: Instant) -> Result<(), anyhow::Error> {
	// A press that only wakes the display must not also run the key's action.
	if crate::device_sleep::note_key_down(&event.payload.device, event.payload.position).await {
		return Ok(());
	}
	crate::events::outbound::keypad::key_down_at(&event.payload.device, event.payload.position, requested_at).await
}

pub async fn key_up(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_key_up(&event.payload.device, event.payload.position) {
		return Ok(());
	}
	crate::events::outbound::keypad::key_up(&event.payload.device, event.payload.position).await
}

#[derive(Deserialize)]
pub struct TicksPayload {
	pub device: String,
	pub position: u8,
	pub ticks: i16,
}

pub async fn encoder_change(event: PayloadEvent<TicksPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_activity(&event.payload.device).await {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_rotate(&event.payload.device, event.payload.position, event.payload.ticks).await
}

pub async fn encoder_down(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_activity(&event.payload.device).await {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_press(&event.payload.device, "dialDown", event.payload.position).await
}

pub async fn encoder_up(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_activity(&event.payload.device).await {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_press(&event.payload.device, "dialUp", event.payload.position).await
}

#[derive(Deserialize)]
pub struct TouchscreenPressPayload {
	pub device: String,
	pub position: u8,
	pub x: u16,
	pub y: u16,
	#[serde(default)]
	pub hold: bool,
}

pub async fn touchscreen_press(event: PayloadEvent<TouchscreenPressPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_activity(&event.payload.device).await {
		return Ok(());
	}
	crate::events::outbound::encoder::touch_tap(&event.payload.device, event.payload.position, event.payload.x, event.payload.y, event.payload.hold).await
}

pub async fn rerender_images(_event: PayloadEvent<String>) -> Result<(), anyhow::Error> {
	crate::events::frontend::profiles::rerender_images(crate::APP_HANDLE.get().unwrap()).await?;
	Ok(())
}
