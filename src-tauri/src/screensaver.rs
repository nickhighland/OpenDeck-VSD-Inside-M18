use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Serialize;
use tauri::Emitter;

static ENABLED: AtomicBool = AtomicBool::new(false);
static TIMEOUT_MINUTES: AtomicU64 = AtomicU64::new(5);
// M18 presses are a fallback wake signal: they do not necessarily reset macOS's
// keyboard/mouse idle clock, but must not immediately restart the screensaver.
static LAST_DEVICE_INTERACTION: LazyLock<DashMap<String, Instant>> = LazyLock::new(DashMap::new);
static ACTIVE_DEVICES: LazyLock<DashMap<String, ()>> = LazyLock::new(DashMap::new);
static WAKE_PRESSES: LazyLock<DashMap<(String, u8), ()>> = LazyLock::new(DashMap::new);

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
	fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

#[cfg(target_os = "macos")]
fn computer_idle_duration() -> Option<Duration> {
	// CombinedSessionState counts all user keyboard, mouse and trackpad input
	// across applications, unlike our own M18 key activity.
	const COMBINED_SESSION_STATE: i32 = 0;
	const ANY_INPUT_EVENT: u32 = u32::MAX;
	let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) };
	(seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

#[cfg(not(target_os = "macos"))]
fn computer_idle_duration() -> Option<Duration> {
	None
}

#[derive(Clone, Serialize)]
struct DeviceEvent {
	device: String,
}

fn is_m18(device: &str) -> bool {
	device.starts_with("18-")
}

fn has_media_configured() -> bool {
	let settings = crate::store::get_settings().value;
	match settings.screensaver_mode.as_str() {
		"slideshow" => !settings.screensaver_photo_paths.is_empty(),
		_ => !settings.screensaver_video_path.is_empty(),
	}
}

fn emit(event: &str, device: &str) {
	let Some(app) = crate::APP_HANDLE.get() else { return };
	if let Err(error) = app.emit(event, DeviceEvent { device: device.to_owned() }) {
		log::warn!("Failed to emit {event} for {device}: {error}");
	}
}

async fn restore_profile_images() {
	if let Some(app) = crate::APP_HANDLE.get()
		&& let Err(error) = crate::events::frontend::profiles::rerender_images(app).await
	{
		log::warn!("Failed to request profile image redraw after screensaver: {error}");
	}
}

async fn clear_device_background(device: &str) {
	if let Err(error) = crate::m18::clear_screensaver_background(device).await {
		log::warn!("Failed to clear M18 screensaver background: {error}");
	}
}

pub fn init_screensaver() {
	let settings = crate::store::get_settings().value;
	ENABLED.store(settings.screensaver_enabled, Ordering::Relaxed);
	TIMEOUT_MINUTES.store(settings.screensaver_timeout_minutes as u64, Ordering::Relaxed);

	tokio::spawn(async {
		loop {
			check_idle_devices().await;
			tokio::time::sleep(Duration::from_secs(1)).await;
		}
	});
}

async fn check_idle_devices() {
	let enabled = ENABLED.load(Ordering::Relaxed) && TIMEOUT_MINUTES.load(Ordering::Relaxed) != 0 && has_media_configured();
	let idle_after = Duration::from_secs(TIMEOUT_MINUTES.load(Ordering::Relaxed) * 60);
	let now = Instant::now();
	let computer_idle = computer_idle_duration();
	let devices = crate::shared::DEVICES.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();

	for device in devices {
		if !is_m18(&device) {
			continue;
		}
		let Some(last_interaction) = LAST_DEVICE_INTERACTION.get(&device).map(|entry| *entry.value()) else {
			LAST_DEVICE_INTERACTION.insert(device, now);
			continue;
		};
		let device_idle = now.duration_since(last_interaction);
		// On macOS both clocks must be idle: ordinary computer use blocks the
		// saver, while an M18 wake press starts a fresh timeout on its own.
		let idle = computer_idle.map_or(device_idle, |system_idle| system_idle.min(device_idle));
		if !enabled || crate::device_sleep::is_device_sleeping(&device) || idle < idle_after {
			if ACTIVE_DEVICES.contains_key(&device) {
				stop_device(&device).await;
			}
			continue;
		}

		if ACTIVE_DEVICES.insert(device.clone(), ()).is_none() {
			emit("screensaver_start", &device);
		}
	}
}

pub fn apply_initial_device(device: &str) {
	if is_m18(device) {
		LAST_DEVICE_INTERACTION.insert(device.to_owned(), Instant::now());
		ACTIVE_DEVICES.remove(device);
	}
}

pub fn deregister_device(device: &str) {
	LAST_DEVICE_INTERACTION.remove(device);
	ACTIVE_DEVICES.remove(device);
	WAKE_PRESSES.retain(|(id, _), _| id != device);
}

/// Returns true when this press was consumed as a screensaver wake-up.
pub async fn note_button_activity(device: &str, position: u8) -> bool {
	LAST_DEVICE_INTERACTION.insert(device.to_owned(), Instant::now());
	if ACTIVE_DEVICES.remove(device).is_some() {
		WAKE_PRESSES.insert((device.to_owned(), position), ());
		emit("screensaver_stop", device);
		clear_device_background(device).await;
		restore_profile_images().await;
		return true;
	}

	false
}

/// The release half of a wake press must also be swallowed: several actions
/// (including OpenApps) execute on key-up rather than key-down.
pub fn consume_wake_release(device: &str, position: u8) -> bool {
	WAKE_PRESSES.remove(&(device.to_owned(), position)).is_some()
}

pub fn is_active(device: &str) -> bool {
	ACTIVE_DEVICES.contains_key(device)
}

/// Start the configured device screensaver immediately from a VSD Craft
/// screensaver action. This does not change the global idle-trigger setting.
pub async fn start_device(device: &str) {
	if !is_m18(device) || !has_media_configured() || crate::device_sleep::is_device_sleeping(device) {
		return;
	}
	LAST_DEVICE_INTERACTION.insert(device.to_owned(), Instant::now());
	if ACTIVE_DEVICES.insert(device.to_owned(), ()).is_none() {
		emit("screensaver_start", device);
	}
}

pub fn active_devices() -> Vec<String> {
	ACTIVE_DEVICES.iter().map(|entry| entry.key().clone()).collect()
}

pub async fn stop_device(device: &str) {
	if ACTIVE_DEVICES.remove(device).is_some() {
		emit("screensaver_stop", device);
		clear_device_background(device).await;
		restore_profile_images().await;
	}
}

pub async fn update_settings(enabled: bool, timeout_minutes: u16) {
	ENABLED.store(enabled, Ordering::Relaxed);
	TIMEOUT_MINUTES.store(timeout_minutes as u64, Ordering::Relaxed);

	if !enabled || timeout_minutes == 0 {
		let devices = ACTIVE_DEVICES.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
		for device in devices {
			stop_device(&device).await;
		}
	} else {
		let now = Instant::now();
		for device in crate::shared::DEVICES.iter().map(|entry| entry.key().clone()) {
			LAST_DEVICE_INTERACTION.insert(device, now);
		}
	}
}
