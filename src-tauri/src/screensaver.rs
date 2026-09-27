use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Serialize;
use tauri::Emitter;

static ENABLED: AtomicBool = AtomicBool::new(false);
static TIMEOUT_MINUTES: AtomicU64 = AtomicU64::new(5);
// Used as the idle clock only on platforms without a computer-wide idle API.
static LAST_DEVICE_INTERACTION: LazyLock<DashMap<String, Instant>> = LazyLock::new(DashMap::new);
static ACTIVE_DEVICES: LazyLock<DashMap<String, ()>> = LazyLock::new(DashMap::new);
// Dismissing the saver on the M18 must leave it awake while the Mac remains
// idle; otherwise the expired system-idle clock would turn it back on at once.
static AWAITING_COMPUTER_ACTIVITY: LazyLock<DashMap<String, ()>> = LazyLock::new(DashMap::new);
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

fn idle_duration_for_screensaver(computer_idle: Option<Duration>, device_idle: Duration) -> Option<Duration> {
	#[cfg(target_os = "macos")]
	{
		let _ = device_idle;
		computer_idle
	}
	#[cfg(not(target_os = "macos"))]
	{
		let _ = computer_idle;
		Some(device_idle)
	}
}

fn should_activate_screensaver(enabled: bool, sleeping: bool, awaiting_computer_activity: bool, idle: Option<Duration>, idle_after: Duration) -> bool {
	enabled && !sleeping && !awaiting_computer_activity && idle.is_some_and(|duration| duration >= idle_after)
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
		let device_idle = LAST_DEVICE_INTERACTION.get(&device).map(|entry| now.duration_since(*entry.value())).unwrap_or_else(|| {
			LAST_DEVICE_INTERACTION.insert(device.clone(), now);
			Duration::ZERO
		});
		let idle = idle_duration_for_screensaver(computer_idle, device_idle);
		if computer_idle.is_some_and(|duration| duration < idle_after) {
			AWAITING_COMPUTER_ACTIVITY.remove(&device);
		}
		let should_activate = should_activate_screensaver(
			enabled,
			crate::device_sleep::is_device_sleeping(&device),
			AWAITING_COMPUTER_ACTIVITY.contains_key(&device),
			idle,
			idle_after,
		);
		if !should_activate {
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
		AWAITING_COMPUTER_ACTIVITY.remove(device);
	}
}

pub fn deregister_device(device: &str) {
	LAST_DEVICE_INTERACTION.remove(device);
	ACTIVE_DEVICES.remove(device);
	AWAITING_COMPUTER_ACTIVITY.remove(device);
	WAKE_PRESSES.retain(|(id, _), _| id != device);
}

/// Returns true when this press was consumed as a screensaver wake-up.
pub async fn note_button_activity(device: &str, position: u8) -> bool {
	LAST_DEVICE_INTERACTION.insert(device.to_owned(), Instant::now());
	if ACTIVE_DEVICES.remove(device).is_some() {
		#[cfg(target_os = "macos")]
		AWAITING_COMPUTER_ACTIVITY.insert(device.to_owned(), ());
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
		AWAITING_COMPUTER_ACTIVITY.clear();
	} else {
		let now = Instant::now();
		for device in crate::shared::DEVICES.iter().map(|entry| entry.key().clone()) {
			LAST_DEVICE_INTERACTION.insert(device, now);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[cfg(target_os = "macos")]
	#[test]
	fn mac_screensaver_uses_computer_idle_not_m18_idle() {
		let timeout = Duration::from_secs(300);
		assert_eq!(idle_duration_for_screensaver(Some(Duration::from_secs(20)), Duration::from_secs(900)), Some(Duration::from_secs(20)));
		assert_eq!(idle_duration_for_screensaver(Some(Duration::from_secs(900)), Duration::from_secs(20)), Some(Duration::from_secs(900)));
		assert!(!should_activate_screensaver(true, false, false, Some(Duration::from_secs(20)), timeout));
		assert!(should_activate_screensaver(true, false, false, Some(Duration::from_secs(900)), timeout));
	}

	#[cfg(target_os = "macos")]
	#[test]
	fn unknown_computer_idle_does_not_fall_back_to_m18_idle_on_macos() {
		let timeout = Duration::from_secs(300);
		let idle = idle_duration_for_screensaver(None, Duration::from_secs(900));
		assert_eq!(idle, None);
		assert!(!should_activate_screensaver(true, false, false, idle, timeout));
	}

	#[test]
	fn waking_screensaver_waits_until_mac_activity_before_rearming() {
		let timeout = Duration::from_secs(300);
		assert!(!should_activate_screensaver(true, false, true, Some(Duration::from_secs(900)), timeout));
		assert!(!should_activate_screensaver(true, false, false, Some(Duration::from_secs(10)), timeout));
		assert!(should_activate_screensaver(true, false, false, Some(Duration::from_secs(900)), timeout));
	}

	#[cfg(not(target_os = "macos"))]
	#[test]
	fn other_platforms_keep_the_device_idle_fallback() {
		assert_eq!(idle_duration_for_screensaver(None, Duration::from_secs(900)), Some(Duration::from_secs(900)));
	}
}
