//! Core playback runtime for VSD Craft's M18 soundboard actions.
//!
//! Playback stays in the app process supervisor: Play/Stop, Replay, Overlap,
//! Loop/Stop, output routing, per-action volume, and fades do not depend on a
//! VSD action-plugin process.

use crate::shared::ActionInstance;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaybackMode {
	PlayStop,
	PlayOverlap,
	PlayReplay,
	LoopStop,
}

impl PlaybackMode {
	fn parse(value: Option<&str>) -> Self {
		match value.unwrap_or("Play/Stop").to_ascii_lowercase().as_str() {
			"play/overlap" => Self::PlayOverlap,
			"play/replay" => Self::PlayReplay,
			"loop/stop" => Self::LoopStop,
			_ => Self::PlayStop,
		}
	}

	fn loops(self) -> bool {
		self == Self::LoopStop
	}

	fn toggles(self) -> bool {
		matches!(self, Self::PlayStop | Self::LoopStop)
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FadeType {
	None,
	In,
	Out,
	InOut,
}

impl FadeType {
	fn parse(value: Option<&str>) -> Self {
		match value.unwrap_or("none").to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
			"fadein" | "in" => Self::In,
			"fadeout" | "out" => Self::Out,
			"fadeinout" | "inout" | "both" => Self::InOut,
			_ => Self::None,
		}
	}
}

struct Playback {
	mode: PlaybackMode,
	cancel: CancellationToken,
	active: Arc<AtomicBool>,
}

static PLAYBACKS: LazyLock<Mutex<HashMap<String, Vec<Playback>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn playback_key(instance: &ActionInstance) -> String {
	format!(
		"{}:{}:{}:{}:{}",
		instance.context.device, instance.context.profile, instance.context.controller, instance.context.position, instance.context.index
	)
}

fn string_setting<'a>(settings: &'a Value, keys: &[&str]) -> Option<&'a str> {
	keys.iter().find_map(|key| settings.get(*key).and_then(Value::as_str).filter(|value| !value.trim().is_empty()))
}

fn volume_setting(settings: &Value) -> u8 {
	settings
		.get("volume")
		.and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
		.unwrap_or(100)
		.clamp(0, 100) as u8
}

fn output_device_setting(settings: &Value) -> Option<String> {
	for key in ["outputDevice", "device", "QAudioDevice", "outputType"] {
		let Some(value) = settings.get(key) else { continue };
		if let Some(name) = value.as_str().filter(|name| !name.trim().is_empty()) {
			return Some(name.to_owned());
		}
		if let Some(object) = value.as_object() {
			for field in ["name", "description", "deviceName", "id"] {
				if let Some(name) = object.get(field).and_then(Value::as_str).filter(|name| !name.trim().is_empty()) {
					return Some(name.to_owned());
				}
			}
		}
	}
	None
}

fn fade_duration_setting(settings: &Value) -> Duration {
	let seconds = settings
		.get("fadeDuration")
		.and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
		.unwrap_or(0)
		.min(3600);
	Duration::from_secs(seconds)
}

fn fade_factor(fade_type: FadeType, elapsed: Duration, total: Option<Duration>, fade: Duration) -> f32 {
	if fade.is_zero() {
		return 1.0;
	}
	let ramp_in = || (elapsed.as_secs_f32() / fade.as_secs_f32()).clamp(0.0, 1.0);
	let ramp_out = || total.map(|total| (total.saturating_sub(elapsed).as_secs_f32() / fade.as_secs_f32()).clamp(0.0, 1.0)).unwrap_or(1.0);
	match fade_type {
		FadeType::None => 1.0,
		FadeType::In => ramp_in(),
		FadeType::Out => ramp_out(),
		FadeType::InOut => ramp_in().min(ramp_out()),
	}
}

#[cfg(target_os = "macos")]
fn output_stream(device_name: Option<&str>) -> Result<rodio::stream::MixerDeviceSink, anyhow::Error> {
	use cpal::traits::{DeviceTrait, HostTrait};
	use rodio::stream::DeviceSinkBuilder;

	let Some(device_name) = device_name.filter(|name| !name.is_empty() && *name != "default") else {
		return Ok(DeviceSinkBuilder::open_default_sink()?);
	};
	let device = cpal::default_host()
		.output_devices()?
		.find(|device| device.id().is_ok_and(|id| id.to_string() == device_name) || device.description().is_ok_and(|description| description.name() == device_name))
		.ok_or_else(|| anyhow::anyhow!("Selected audio output device is no longer available: {device_name}"))?;
	Ok(DeviceSinkBuilder::from_device(device)?.open_stream()?)
}

#[cfg(not(target_os = "macos"))]
fn output_stream(_device_name: Option<&str>) -> Result<(), anyhow::Error> {
	Err(anyhow::anyhow!("VSD soundboard playback is currently supported on macOS only"))
}

#[cfg(target_os = "macos")]
fn play_file_blocking(path: &str, volume: u8, device: Option<&str>, looping: bool, fade_type: FadeType, fade: Duration, cancel: &CancellationToken) -> Result<(), anyhow::Error> {
	use rodio::{Decoder, Player, source::Source};
	use std::fs::File;

	let stream = output_stream(device)?;
	let player = Player::connect_new(stream.mixer());
	let target_volume = f32::from(volume) / 100.0;
	loop {
		if cancel.is_cancelled() {
			break;
		}
		let decoder = Decoder::try_from(File::open(path)?)?;
		let total = decoder.total_duration();
		player.append(decoder);
		let started = std::time::Instant::now();
		while !player.empty() && !cancel.is_cancelled() {
			player.set_volume(target_volume * fade_factor(fade_type, started.elapsed(), total, fade));
			std::thread::sleep(Duration::from_millis(20));
		}
		if !looping || cancel.is_cancelled() {
			break;
		}
	}
	player.stop();
	drop(player);
	drop(stream);
	Ok(())
}

#[cfg(not(target_os = "macos"))]
fn play_file_blocking(_path: &str, _volume: u8, _device: Option<&str>, _looping: bool, _fade_type: FadeType, _fade: Duration, _cancel: &CancellationToken) -> Result<(), anyhow::Error> {
	Err(anyhow::anyhow!("VSD soundboard playback is currently supported on macOS only"))
}

async fn play_file(path: String, volume: u8, device: Option<String>, looping: bool, fade_type: FadeType, fade: Duration, cancel: CancellationToken, active: Arc<AtomicBool>) {
	let worker_cancel = cancel.clone();
	let result: Result<(), anyhow::Error> = match tokio::task::spawn_blocking(move || play_file_blocking(&path, volume, device.as_deref(), looping, fade_type, fade, &worker_cancel)).await {
		Ok(result) => result,
		Err(error) => Err(error.into()),
	};
	if let Err(error) = result
		&& !cancel.is_cancelled()
	{
		log::warn!("VSD soundboard playback failed: {error:#}");
	}
	active.store(false, Ordering::Release);
}

#[tauri::command]
pub fn get_audio_output_devices() -> Vec<AudioOutputDevice> {
	#[cfg(target_os = "macos")]
	{
		use cpal::traits::{DeviceTrait, HostTrait};
		let Ok(devices) = cpal::default_host().output_devices() else { return vec![] };
		let mut devices = devices
			.filter_map(|device| {
				Some(AudioOutputDevice {
					id: device.id().ok()?.to_string(),
					name: device.description().ok()?.name().to_owned(),
				})
			})
			.collect::<Vec<_>>();
		devices.sort_by(|left, right| left.name.cmp(&right.name).then_with(|| left.id.cmp(&right.id)));
		devices
	}
	#[cfg(not(target_os = "macos"))]
	{
		vec![]
	}
}

#[derive(Serialize)]
pub struct AudioOutputDevice {
	pub id: String,
	pub name: String,
}

/// Start/toggle a configured VSD Craft Play Audio action.
///
/// Returns whether the action's two visual states should advance.
pub async fn play(instance: &ActionInstance) -> Result<bool, anyhow::Error> {
	let mode = PlaybackMode::parse(string_setting(&instance.settings, &["mode", "playMode", "select"]));
	let Some(path) = string_setting(&instance.settings, &["path", "filePath", "audio", "musicUrl"]) else {
		log::warn!("VSD soundboard action has no selected audio file");
		return Ok(false);
	};
	if !Path::new(path).is_file() {
		log::warn!("VSD soundboard audio file does not exist: {path}");
		return Ok(false);
	}

	let key = playback_key(instance);
	let mut playbacks = PLAYBACKS.lock().map_err(|_| anyhow::anyhow!("Soundboard playback registry lock was poisoned"))?;
	let entries = playbacks.entry(key).or_default();
	entries.retain(|playback| playback.active.load(Ordering::Acquire));
	let same_mode_active = entries.iter().any(|playback| playback.mode == mode);
	if matches!(mode, PlaybackMode::PlayStop | PlaybackMode::LoopStop) && same_mode_active {
		for playback in entries.iter_mut().filter(|playback| playback.mode == mode) {
			playback.cancel.cancel();
		}
		return Ok(true);
	}
	if mode == PlaybackMode::PlayReplay {
		for playback in entries.iter_mut().filter(|playback| playback.mode == PlaybackMode::PlayReplay) {
			playback.cancel.cancel();
		}
	}

	let cancel = CancellationToken::new();
	let active = Arc::new(AtomicBool::new(true));
	entries.push(Playback {
		mode,
		cancel: cancel.clone(),
		active: active.clone(),
	});
	let volume = volume_setting(&instance.settings);
	let device = output_device_setting(&instance.settings);
	let fade_type = FadeType::parse(string_setting(&instance.settings, &["fadeType", "fade"]));
	let fade = fade_duration_setting(&instance.settings);
	tokio::spawn(play_file(path.to_owned(), volume, device, mode.loops(), fade_type, fade, cancel, active));
	Ok(mode.toggles())
}

/// Stop every active VSD Craft soundboard stream.
pub fn stop_all() -> Result<(), anyhow::Error> {
	let mut playbacks = PLAYBACKS.lock().map_err(|_| anyhow::anyhow!("Soundboard playback registry lock was poisoned"))?;
	for playback in playbacks.values_mut().flatten() {
		playback.cancel.cancel();
	}
	playbacks.clear();
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn playback_modes_match_vsd_toggle_and_overlap_groups() {
		assert!(PlaybackMode::parse(Some("Play/Stop")).toggles());
		assert!(PlaybackMode::parse(Some("Loop/Stop")).loops());
		assert!(!PlaybackMode::parse(Some("Play/Overlap")).toggles());
		assert!(!PlaybackMode::parse(Some("Play/Replay")).toggles());
	}

	#[test]
	fn volume_is_clamped_and_defaults_to_full_volume() {
		assert_eq!(volume_setting(&serde_json::json!({})), 100);
		assert_eq!(volume_setting(&serde_json::json!({ "volume": -1 })), 0);
		assert_eq!(volume_setting(&serde_json::json!({ "volume": 150 })), 100);
		assert_eq!(volume_setting(&serde_json::json!({ "volume": "42" })), 42);
	}

	#[test]
	fn output_device_and_fade_settings_accept_vsd_shapes() {
		assert_eq!(output_device_setting(&serde_json::json!({ "outputDevice": "Headphones" })).as_deref(), Some("Headphones"));
		assert_eq!(output_device_setting(&serde_json::json!({ "device": { "description": "USB DAC" } })).as_deref(), Some("USB DAC"));
		assert_eq!(output_device_setting(&serde_json::json!({})), None);
		assert_eq!(FadeType::parse(Some("FadeInOut")), FadeType::InOut);
		assert_eq!(FadeType::parse(Some("FadeOut")), FadeType::Out);
		assert_eq!(fade_duration_setting(&serde_json::json!({ "fadeDuration": "4" })), Duration::from_secs(4));
	}

	#[test]
	fn fade_envelopes_ramp_in_and_out_without_exceeding_target_volume() {
		let fade = Duration::from_secs(2);
		assert_eq!(fade_factor(FadeType::None, Duration::ZERO, Some(Duration::from_secs(10)), fade), 1.0);
		assert_eq!(fade_factor(FadeType::In, Duration::from_secs(1), Some(Duration::from_secs(10)), fade), 0.5);
		assert_eq!(fade_factor(FadeType::Out, Duration::from_secs(9), Some(Duration::from_secs(10)), fade), 0.5);
		assert_eq!(fade_factor(FadeType::InOut, Duration::from_secs(1), Some(Duration::from_secs(10)), fade), 0.5);
		assert_eq!(fade_factor(FadeType::Out, Duration::from_secs(9), None, fade), 1.0);
	}
}
