//! M18 display sleep, driven by the computer's state rather than by M18 use.
//!
//! - While the computer is in use, the M18 display stays on, however long it
//!   has been since an M18 key was pressed.
//! - Once the computer has had no keyboard, mouse, or trackpad input for the
//!   configured number of minutes, the display turns off. It turns back on as
//!   soon as the computer is used again.
//! - Optionally, the display also turns off while the computer is locked.
//! - Pressing an M18 key while its display is off only wakes the display; the
//!   key's action does not run. The display then stays on for the timeout after
//!   the last press, so it does not go dark again while the computer is idle.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

static SLEEP_TIMEOUT_MINUTES: AtomicU16 = AtomicU16::new(0);
static SLEEP_WHEN_COMPUTER_LOCKED: AtomicBool = AtomicBool::new(false);
static COMPUTER_LOCKED: AtomicBool = AtomicBool::new(false);

/// When each device's keys were last pressed.
static LAST_DEVICE_PRESS: LazyLock<DashMap<String, Instant>> = LazyLock::new(DashMap::new);
static SLEEPING_DEVICES: LazyLock<DashMap<String, SleepReason>> = LazyLock::new(DashMap::new);
/// Keys whose press only woke a display; their release is swallowed too.
static WAKE_PRESSES: LazyLock<DashMap<(String, u8), ()>> = LazyLock::new(DashMap::new);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SleepReason {
	/// The computer has been idle; computer activity wakes the display.
	Idle,
	/// The computer is locked; unlocking wakes the display.
	Locked,
	/// Requested explicitly (`--sleep-device`); only a key press or
	/// `--wake-device` wakes the display.
	Manual,
}

/// How long the computer has gone without keyboard, mouse, or trackpad input,
/// or `None` where the platform cannot report it.
#[cfg(target_os = "macos")]
fn computer_idle_duration() -> Option<Duration> {
	#[link(name = "CoreGraphics", kind = "framework")]
	unsafe extern "C" {
		fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
	}
	// The combined session state covers input from every application and
	// device, not only this app's own events.
	const COMBINED_SESSION_STATE: i32 = 0;
	const ANY_INPUT_EVENT: u32 = u32::MAX;
	let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT) };
	(seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

#[cfg(windows)]
fn computer_idle_duration() -> Option<Duration> {
	#[repr(C)]
	struct LastInputInfo {
		cb_size: u32,
		time: u32,
	}
	#[link(name = "user32")]
	unsafe extern "system" {
		fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
	}
	#[link(name = "kernel32")]
	unsafe extern "system" {
		fn GetTickCount() -> u32;
	}
	let mut info = LastInputInfo {
		cb_size: std::mem::size_of::<LastInputInfo>() as u32,
		time: 0,
	};
	if unsafe { GetLastInputInfo(&mut info) } == 0 {
		return None;
	}
	// Both values are 32-bit millisecond tick counts that wrap every ~49 days.
	let now = unsafe { GetTickCount() };
	Some(Duration::from_millis(u64::from(now.wrapping_sub(info.time))))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn computer_idle_duration() -> Option<Duration> {
	None
}

fn sleep_timeout() -> Duration {
	Duration::from_secs(u64::from(SLEEP_TIMEOUT_MINUTES.load(Ordering::Relaxed)) * 60)
}

fn locked_sleep_active() -> bool {
	SLEEP_WHEN_COMPUTER_LOCKED.load(Ordering::Relaxed) && COMPUTER_LOCKED.load(Ordering::Relaxed)
}

/// Whether an awake display should turn off for inactivity. Both the computer
/// and the M18 itself must have been idle for the whole timeout. An unknown
/// computer idle time never puts the display to sleep.
fn idle_sleep_due(timeout: Duration, computer_idle: Option<Duration>, since_last_press: Option<Duration>) -> bool {
	!timeout.is_zero() && computer_idle.is_some_and(|idle| idle >= timeout) && since_last_press.is_none_or(|elapsed| elapsed >= timeout)
}

/// Whether a display that turned off for inactivity should wake because the
/// computer is in use again (or idle sleep has been switched off).
fn idle_wake_due(timeout: Duration, computer_idle: Option<Duration>) -> bool {
	timeout.is_zero() || computer_idle.is_none_or(|idle| idle < timeout)
}

pub fn is_device_sleeping(device: &str) -> bool {
	SLEEPING_DEVICES.contains_key(device)
}

pub fn init_device_sleep() {
	let settings = crate::store::current_settings();
	SLEEP_TIMEOUT_MINUTES.store(settings.sleep_timeout_minutes, Ordering::Relaxed);
	SLEEP_WHEN_COMPUTER_LOCKED.store(settings.sleep_when_computer_locked, Ordering::Relaxed);

	#[cfg(not(any(target_os = "macos", windows)))]
	if settings.sleep_timeout_minutes > 0 {
		log::warn!("This platform cannot report computer idle time; the M18 idle sleep setting has no effect here");
	}

	tokio::spawn(async {
		loop {
			update_idle_sleep().await;
			tokio::time::sleep(POLL_INTERVAL).await;
		}
	});
}

async fn update_idle_sleep() {
	let timeout = sleep_timeout();
	let computer_idle = computer_idle_duration();
	let now = Instant::now();
	let devices = crate::shared::DEVICES.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
	for device in devices {
		let reason = SLEEPING_DEVICES.get(&device).map(|entry| *entry.value());
		match reason {
			Some(SleepReason::Idle) if idle_wake_due(timeout, computer_idle) => {
				wake_device(&device).await;
			}
			None => {
				let since_last_press = LAST_DEVICE_PRESS.get(&device).map(|entry| now.saturating_duration_since(*entry.value()));
				if idle_sleep_due(timeout, computer_idle, since_last_press) {
					put_to_sleep(&device, SleepReason::Idle).await;
				}
			}
			_ => {}
		}
	}
}

async fn put_to_sleep(device: &str, reason: SleepReason) {
	if let Some(mut existing) = SLEEPING_DEVICES.get_mut(device) {
		// Already dark: a lock supersedes idleness, so unlocking wakes it.
		if reason == SleepReason::Locked {
			*existing = reason;
		}
		return;
	}
	if let Err(error) = crate::events::outbound::devices::set_device_brightness(device, 0).await {
		log::warn!("Failed to turn off the display of {device}: {error:#}");
		return;
	}
	// The LEDs go dark with the display.
	if let Err(error) = crate::m18::set_led_brightness(device, 0).await {
		log::warn!("Failed to turn off the LEDs of {device}: {error:#}");
	}
	SLEEPING_DEVICES.insert(device.to_owned(), reason);
	log::debug!("Turned off the display of {device} ({reason:?})");
}

/// Turn a sleeping display back on. Returns whether it was asleep.
async fn wake_device(device: &str) -> bool {
	if SLEEPING_DEVICES.remove(device).is_none() {
		return false;
	}
	let settings = crate::store::current_settings();
	if let Err(error) = crate::events::outbound::devices::set_device_brightness(device, settings.brightness).await {
		log::warn!("Failed to turn on the display of {device}: {error:#}");
	}
	if let Err(error) = crate::m18::set_led_brightness(device, settings.led_brightness).await {
		log::warn!("Failed to turn on the LEDs of {device}: {error:#}");
	}
	true
}

/// Whether a device's display is off because it is asleep. Brightness
/// changes wait for it to wake.
pub fn is_sleeping(device: &str) -> bool {
	SLEEPING_DEVICES.contains_key(device)
}

/// Record an M18 key press. Returns `true` when the press must not run its
/// action: it woke the display, or the display stays off because the
/// computer is locked.
pub async fn note_key_down(device: &str, position: u8) -> bool {
	LAST_DEVICE_PRESS.insert(device.to_owned(), Instant::now());
	if locked_sleep_active() || wake_device(device).await {
		WAKE_PRESSES.insert((device.to_owned(), position), ());
		return true;
	}
	false
}

/// Returns `true` when this release belongs to a press that only woke the display.
pub fn note_key_up(device: &str, position: u8) -> bool {
	WAKE_PRESSES.remove(&(device.to_owned(), position)).is_some()
}

/// Record input from a control without a press/release pair. Returns `true`
/// when the input only woke the display.
pub async fn note_activity(device: &str) -> bool {
	LAST_DEVICE_PRESS.insert(device.to_owned(), Instant::now());
	locked_sleep_active() || wake_device(device).await
}

pub fn deregister_device(device: &str) {
	LAST_DEVICE_PRESS.remove(device);
	SLEEPING_DEVICES.remove(device);
	WAKE_PRESSES.retain(|(id, _), _| id != device);
}

/// Turn a device's display off until a key press or `wake_device_now`.
pub async fn sleep_device(device: String) -> Result<(), anyhow::Error> {
	put_to_sleep(&device, SleepReason::Manual).await;
	Ok(())
}

/// Turn a device's display back on immediately.
pub async fn wake_device_now(device: &str) -> Result<(), anyhow::Error> {
	LAST_DEVICE_PRESS.insert(device.to_owned(), Instant::now());
	wake_device(device).await;
	Ok(())
}

pub async fn update_sleep_timeout_minutes(minutes: u16) -> Result<(), anyhow::Error> {
	SLEEP_TIMEOUT_MINUTES.store(minutes, Ordering::Relaxed);
	// Re-evaluate at once: a shorter timeout may already have elapsed, and
	// switching idle sleep off must wake displays it turned off.
	update_idle_sleep().await;
	Ok(())
}

pub async fn update_sleep_when_computer_locked(enabled: bool) -> Result<(), anyhow::Error> {
	SLEEP_WHEN_COMPUTER_LOCKED.store(enabled, Ordering::Relaxed);
	if enabled && COMPUTER_LOCKED.load(Ordering::Relaxed) {
		sleep_all(SleepReason::Locked).await;
	} else if !enabled {
		wake_all(|reason| reason == SleepReason::Locked).await;
	}
	Ok(())
}

async fn sleep_all(reason: SleepReason) {
	let devices = crate::shared::DEVICES.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
	for device in devices {
		put_to_sleep(&device, reason).await;
	}
}

async fn wake_all(should_wake: impl Fn(SleepReason) -> bool) {
	let devices = SLEEPING_DEVICES.iter().filter(|entry| should_wake(*entry.value())).map(|entry| entry.key().clone()).collect::<Vec<_>>();
	for device in devices {
		wake_device(&device).await;
	}
}

pub async fn sleep_for_computer_lock() -> Result<(), anyhow::Error> {
	COMPUTER_LOCKED.store(true, Ordering::Relaxed);
	if SLEEP_WHEN_COMPUTER_LOCKED.load(Ordering::Relaxed) {
		sleep_all(SleepReason::Locked).await;
	}
	Ok(())
}

pub async fn wake_from_computer_lock() -> Result<(), anyhow::Error> {
	COMPUTER_LOCKED.store(false, Ordering::Relaxed);
	// Unlocking means someone is at the computer, so idle-dark displays wake
	// too; a display turned off on request stays off.
	wake_all(|reason| reason != SleepReason::Manual).await;
	Ok(())
}

pub async fn apply_initial_device_sleep(device: &str) -> Result<(), anyhow::Error> {
	// A newly connected device stays on for at least one timeout period.
	LAST_DEVICE_PRESS.insert(device.to_owned(), Instant::now());
	if locked_sleep_active() {
		put_to_sleep(device, SleepReason::Locked).await;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	const MINUTE: Duration = Duration::from_secs(60);

	#[test]
	fn the_display_stays_on_while_the_computer_is_in_use() {
		// An hour since the last M18 press, but the computer was used just now.
		assert!(!idle_sleep_due(5 * MINUTE, Some(Duration::from_secs(3)), Some(60 * MINUTE)));
		assert!(!idle_sleep_due(5 * MINUTE, Some(4 * MINUTE), None));
	}

	#[test]
	fn the_display_turns_off_once_the_computer_has_been_idle_for_the_timeout() {
		assert!(idle_sleep_due(5 * MINUTE, Some(5 * MINUTE), None));
		assert!(idle_sleep_due(5 * MINUTE, Some(20 * MINUTE), Some(6 * MINUTE)));
	}

	#[test]
	fn a_recent_m18_press_keeps_the_display_on_while_the_computer_is_idle() {
		assert!(!idle_sleep_due(5 * MINUTE, Some(20 * MINUTE), Some(MINUTE)));
	}

	#[test]
	fn idle_sleep_is_off_when_disabled_or_when_idle_time_is_unknown() {
		assert!(!idle_sleep_due(Duration::ZERO, Some(20 * MINUTE), None));
		assert!(!idle_sleep_due(5 * MINUTE, None, None));
	}

	#[test]
	fn computer_activity_wakes_an_idle_display() {
		assert!(idle_wake_due(5 * MINUTE, Some(Duration::from_secs(1))));
		assert!(!idle_wake_due(5 * MINUTE, Some(6 * MINUTE)));
		assert!(idle_wake_due(Duration::ZERO, Some(6 * MINUTE)));
		assert!(idle_wake_due(5 * MINUTE, None));
	}
}
