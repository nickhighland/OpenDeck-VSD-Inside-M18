//! Native actions for the M18 workflow.
//!
//! These actions live in the application core. They do not require a second
//! action-plugin process, which is important for the actions used by the
//! user's everyday M18 profile.

use crate::shared::ActionInstance;

use base64::Engine;
use enigo::{
	Enigo, Keyboard, Mouse, Settings,
	agent::{Agent, Token},
};
use image::{Rgb, RgbImage};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

pub const OPEN_APPS_UUID: &str = "opendeck.m18.open-apps";
pub const SUPER_HOTKEYS_UUID: &str = "opendeck.m18.super-hotkeys";
pub const HOTKEY_SWITCH_UUID: &str = "opendeck.m18.hotkey-switch";
pub const SUPER_HOTKEY_SWITCH_UUID: &str = "opendeck.m18.super-hotkey-switch";
pub const VOLUME_DOWN_UUID: &str = "opendeck.m18.volume-down";
pub const VOLUME_UP_UUID: &str = "opendeck.m18.volume-up";
pub const MUTE_UUID: &str = "opendeck.m18.mute";
pub const SIRI_UUID: &str = "opendeck.m18.siri";
pub const DISPATCH_CENTER_UUID: &str = "opendeck.m18.dispatch-center";
pub const SCREENSHOT_UUID: &str = "opendeck.m18.screenshot";
pub const LAUNCHPAD_UUID: &str = "opendeck.m18.launchpad";
pub const DESKTOP_SAVER_UUID: &str = "opendeck.m18.desktop-saver";
pub const SLEEP_UUID: &str = "opendeck.m18.sleep";
pub const SCREEN_BRIGHTNESS_UP_UUID: &str = "opendeck.m18.screen-brightness-up";
pub const SCREEN_BRIGHTNESS_DOWN_UUID: &str = "opendeck.m18.screen-brightness-down";
pub const PREVIOUS_TRACK_UUID: &str = "opendeck.m18.previous-track";
pub const PLAY_PAUSE_UUID: &str = "opendeck.m18.play-pause";
pub const NEXT_TRACK_UUID: &str = "opendeck.m18.next-track";
pub const PAGE_PREVIOUS_UUID: &str = "opendeck.m18.page-previous";
pub const PAGE_NEXT_UUID: &str = "opendeck.m18.page-next";
pub const PAGE_GOTO_UUID: &str = "opendeck.m18.page-goto";
pub const PAGE_INDICATOR_UUID: &str = "opendeck.m18.page-indicator";
/// Import-only marker for VSD actions whose behavior has not been ported.
/// It is deliberately excluded from the selectable action catalog.
pub const UNSUPPORTED_VSD_UUID: &str = "opendeck.m18.unsupported-vsd-action";

static ENIGO: OnceLock<Mutex<Option<Enigo>>> = OnceLock::new();
static APP_ICON_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

pub fn is_native_action(uuid: &str) -> bool {
	matches!(
		uuid,
		OPEN_APPS_UUID
			| SUPER_HOTKEYS_UUID
			| HOTKEY_SWITCH_UUID
			| SUPER_HOTKEY_SWITCH_UUID
			| VOLUME_DOWN_UUID
			| VOLUME_UP_UUID
			| MUTE_UUID
			| SIRI_UUID
			| DISPATCH_CENTER_UUID
			| SCREENSHOT_UUID
			| LAUNCHPAD_UUID
			| DESKTOP_SAVER_UUID
			| SLEEP_UUID
			| SCREEN_BRIGHTNESS_UP_UUID
			| SCREEN_BRIGHTNESS_DOWN_UUID
			| PREVIOUS_TRACK_UUID
			| PLAY_PAUSE_UUID
			| NEXT_TRACK_UUID
			| PAGE_PREVIOUS_UUID
			| PAGE_NEXT_UUID
			| PAGE_GOTO_UUID
			| PAGE_INDICATOR_UUID
			| UNSUPPORTED_VSD_UUID
	) || (crate::vsd_actions::is_vsd_action(uuid) && !crate::vsd_actions::is_composite_action(uuid))
}

pub fn is_switch_action(uuid: &str) -> bool {
	matches!(uuid, HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID) || crate::vsd_actions::is_hotkey_switch(uuid)
}

pub fn default_settings(uuid: &str) -> Value {
	match uuid {
		OPEN_APPS_UUID => serde_json::json!({ "appPath": "" }),
		SUPER_HOTKEYS_UUID => serde_json::json!({ "down": "", "up": "" }),
		HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID => serde_json::json!({ "hotkeys": [{ "down": "", "up": "" }, { "down": "", "up": "" }], "index": 0 }),
		PAGE_GOTO_UUID => serde_json::json!({ "page": "", "pageIndex": 0, "showPageNumber": true }),
		_ => crate::vsd_actions::default_settings(uuid),
	}
}

fn string_setting(settings: &Value, key: &str) -> Option<String> {
	settings.get(key).and_then(Value::as_str).map(str::to_owned).filter(|value| !value.trim().is_empty())
}

fn image_data_url(image: &str) -> Option<String> {
	if image.starts_with("data:") {
		return (!image.starts_with("data:image/svg+xml")).then(|| image.to_owned());
	}

	let (bytes, extension) = if image == "opendeck/multi-action.png" {
		(include_bytes!("../../static/multi-action.png").to_vec(), "png")
	} else {
		let path = Path::new(image);
		(fs::read(path).ok()?, path.extension().and_then(|extension| extension.to_str()).unwrap_or("png"))
	};
	let mime = match extension.to_ascii_lowercase().as_str() {
		"jpg" | "jpeg" => "image/jpeg",
		"gif" => "image/gif",
		"webp" => "image/webp",
		_ => "image/png",
	};
	Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn application_bundle(value: &str) -> Option<PathBuf> {
	let value = value.trim();
	if value.is_empty() {
		return None;
	}

	let direct = PathBuf::from(value);
	if direct.is_dir() {
		return Some(direct);
	}

	// The inspector accepts a path, bundle identifier, or application name.
	// Resolve the latter two through LaunchServices' metadata index so the
	// icon follows the same application that `/usr/bin/open` will launch.
	let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
	let queries = if value.starts_with("com.") {
		vec![format!("kMDItemCFBundleIdentifier == '{escaped}'c")]
	} else {
		vec![format!("kMDItemFSName == '{escaped}.app'c"), format!("kMDItemDisplayName == '{escaped}'c")]
	};

	for query in queries {
		let output = Command::new("/usr/bin/mdfind").arg(query).output().ok()?;
		for line in String::from_utf8_lossy(&output.stdout).lines() {
			let path = PathBuf::from(line.trim());
			if path.is_dir() {
				return Some(path);
			}
		}
	}

	None
}

fn plist_value(info_plist: &Path, key: &str) -> Option<String> {
	let output = Command::new("/usr/bin/plutil").args(["-extract", key, "raw", "-o", "-", info_plist.to_str()?]).output().ok()?;
	if !output.status.success() {
		return None;
	}
	let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
	(!value.is_empty()).then_some(value)
}

fn icon_file(bundle: &Path) -> Option<PathBuf> {
	let resources = bundle.join("Contents/Resources");
	let info_plist = bundle.join("Contents/Info.plist");
	let mut names = vec![];
	for key in ["CFBundleIconFile", "CFBundleIconName"] {
		if let Some(name) = plist_value(&info_plist, key) {
			names.push(name);
		}
	}

	for name in names {
		let path = resources.join(&name);
		if path.is_file() {
			return Some(path);
		}
		if path.extension().is_none() {
			let with_extension = path.with_extension("icns");
			if with_extension.is_file() {
				return Some(with_extension);
			}
		}
	}

	for name in ["AppIcon.icns", "app.icns", "icon.icns", "electron.icns", "mac_icon.icns"] {
		let path = resources.join(name);
		if path.is_file() {
			return Some(path);
		}
	}

	let mut candidates = fs::read_dir(resources)
		.ok()?
		.flatten()
		.map(|entry| entry.path())
		.filter(|path| {
			path.extension()
				.and_then(|extension| extension.to_str())
				.is_some_and(|extension| extension.eq_ignore_ascii_case("icns"))
		})
		.filter(|path| path.file_name().and_then(|name| name.to_str()) != Some(".VolumeIcon.icns"))
		.collect::<Vec<_>>();
	candidates.sort();
	candidates.into_iter().next()
}

fn generated_app_icon(app: &str) -> Option<String> {
	let bundle = application_bundle(app)?;
	let source = icon_file(&bundle)?;
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	bundle.hash(&mut hasher);
	let temp_dir = std::env::temp_dir().join(format!("opendeck-vsd-m18-app-icon-{}", hasher.finish()));
	fs::create_dir_all(&temp_dir).ok()?;
	let output = temp_dir.join("icon.png");
	let result = Command::new("/usr/bin/sips")
		.args(["-s", "format", "png", "--resampleHeightWidth", "256", "256"])
		.arg(&source)
		.args(["--out", output.to_str()?])
		.output()
		.ok();
	let data = result.filter(|result| result.status.success()).and_then(|_| fs::read(&output).ok());
	let _ = fs::remove_file(&output);
	let _ = fs::remove_dir(&temp_dir);
	data.map(|data| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(data)))
}

fn app_icon_data_url(settings: &Value) -> Option<String> {
	let app = string_setting(settings, "appPath")?;
	let cache = APP_ICON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
	if let Ok(icons) = cache.lock()
		&& let Some(icon) = icons.get(&app)
	{
		return icon.clone();
	}

	let icon = generated_app_icon(&app);
	if let Ok(mut icons) = cache.lock() {
		icons.insert(app, icon.clone());
	}
	icon
}

/// Refreshes the visible state artwork for a native OpenApps instance.
///
/// This is called when an instance is imported, created, edited, or loaded so
/// both the editor and the physical M18 show the selected application's icon.
pub fn refresh_open_app_icon(instance: &mut ActionInstance) {
	if instance.action.uuid != OPEN_APPS_UUID {
		return;
	}
	let Some(icon) = app_icon_data_url(&instance.settings) else {
		return;
	};
	for state in &mut instance.states {
		state.image = icon.clone();
	}
}

fn page_number_for_profile(pages: &[crate::m18_pages::M18Page], selected: usize, profile: &str) -> Option<usize> {
	if pages.is_empty() {
		return None;
	}
	Some(pages.iter().position(|page| page.profile == profile).unwrap_or_else(|| selected.min(pages.len() - 1)) + 1)
}

fn current_page_number(instance: &ActionInstance) -> Option<usize> {
	let page_set = crate::m18_pages::get(&instance.context.device).ok()?;
	page_number_for_profile(&page_set.pages, page_set.selected, &instance.context.profile)
}

fn draw_page_number(image: &mut RgbImage, number: usize) {
	const DIGITS: [[u8; 7]; 10] = [
		[0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
		[0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
		[0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
		[0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
		[0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
		[0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
		[0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
		[0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
		[0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
		[0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100],
	];

	let text = number.to_string();
	let digits: Vec<usize> = text.bytes().rev().take(12).collect::<Vec<_>>().into_iter().rev().map(|byte| (byte - b'0') as usize).collect();
	let scale = (60 / (digits.len() * 5 + digits.len().saturating_sub(1))).clamp(1, 7) as u32;
	let character_width = 5 * scale;
	let gap = scale;
	let text_width = digits.len() as u32 * character_width + digits.len().saturating_sub(1) as u32 * gap;
	let left = (72 - text_width) / 2;
	let top = (72 - 7 * scale) / 2;

	for (digit_index, digit) in digits.iter().enumerate() {
		for (row, bits) in DIGITS[*digit].iter().enumerate() {
			for column in 0..5 {
				if bits & (1 << (4 - column)) == 0 {
					continue;
				}
				let x = left + digit_index as u32 * (character_width + gap) + column * scale;
				let y = top + row as u32 * scale;
				for dy in 0..scale {
					for dx in 0..scale {
						if x + dx + 1 < image.width() && y + dy + 1 < image.height() {
							image.put_pixel(x + dx + 1, y + dy + 1, Rgb([5, 7, 12]));
						}
						image.put_pixel(x + dx, y + dy, Rgb([245, 247, 250]));
					}
				}
			}
		}
	}
}

fn encode_key_image(image: &RgbImage) -> Option<String> {
	let mut bytes = Vec::new();
	image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92)
		.encode_image(&image::DynamicImage::ImageRgb8(image.clone()))
		.ok()?;
	Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn page_number_image(number: usize) -> Option<String> {
	let mut image = RgbImage::from_pixel(72, 72, Rgb([12, 18, 31]));
	draw_page_number(&mut image, number);
	encode_key_image(&image)
}

fn page_number_over_image(number: usize, source: Option<&str>) -> Option<String> {
	let decoded = source
		.and_then(image_data_url)
		.and_then(|data_url| data_url.split_once(',').map(|(_, data)| data.to_owned()))
		.and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
		.and_then(|bytes| image::load_from_memory(&bytes).ok());
	let mut image = decoded
		.map(|image| image.resize_exact(72, 72, image::imageops::FilterType::Lanczos3).to_rgb8())
		.unwrap_or_else(|| RgbImage::from_pixel(72, 72, Rgb([12, 18, 31])));
	draw_page_number(&mut image, number);
	encode_key_image(&image)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn page_indicator_uses_its_profile_position_then_selected_page_as_fallback() {
		let pages = vec![
			crate::m18_pages::M18Page {
				id: "one".to_owned(),
				name: "First".to_owned(),
				profile: "Profile A".to_owned(),
			},
			crate::m18_pages::M18Page {
				id: "two".to_owned(),
				name: "Second".to_owned(),
				profile: "Profile B".to_owned(),
			},
		];

		assert_eq!(page_number_for_profile(&pages, 0, "Profile B"), Some(2));
		assert_eq!(page_number_for_profile(&pages, 1, "unlisted profile"), Some(2));
		assert_eq!(page_number_for_profile(&[], 0, "unlisted profile"), None);
	}

	#[test]
	fn page_indicator_artwork_is_a_full_key_image() {
		let data_url = page_number_image(2).expect("page number should render");
		let encoded = data_url.strip_prefix("data:image/jpeg;base64,").expect("jpeg data URL");
		let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).expect("base64 image");
		let image = image::load_from_memory(&bytes).expect("valid JPEG image");
		assert_eq!((image.width(), image.height()), (72, 72));
		let number_pixel = image.to_rgb8().get_pixel(28, 14).0;
		assert!(number_pixel.iter().all(|channel| *channel > 180), "page number glyph should be visible");

		let red_background = encode_key_image(&RgbImage::from_pixel(72, 72, Rgb([210, 20, 30]))).expect("encode source image");
		let overlaid = page_number_over_image(1, Some(&red_background)).expect("page number overlays a source image");
		let encoded_overlay = overlaid.strip_prefix("data:image/jpeg;base64,").expect("overlay data URL");
		let overlay_bytes = base64::engine::general_purpose::STANDARD.decode(encoded_overlay).expect("overlay base64");
		let overlay_image = image::load_from_memory(&overlay_bytes).expect("valid overlaid JPEG image").to_rgb8();
		assert_eq!((overlay_image.width(), overlay_image.height()), (72, 72));
		let preserved_background = overlay_image.get_pixel(1, 1).0;
		assert!(
			preserved_background[0] > 170 && preserved_background[1] < 70 && preserved_background[2] < 80,
			"overlay should preserve the source artwork"
		);
	}
}

/// The current state's own artwork, when the core can draw it. Bundled
/// artwork ("opendeck/…", including SVG key faces) is drawn by the editor,
/// which also adds the title and background colour.
fn state_image(instance: &ActionInstance) -> Option<String> {
	let image = &instance.states.get(instance.current_state as usize)?.image;
	if image.starts_with("opendeck/") {
		return None;
	}
	image_data_url(image)
}

pub async fn render(instance: &ActionInstance) -> Result<(), anyhow::Error> {
	let image = if instance.action.uuid == OPEN_APPS_UUID {
		let settings = instance.settings.clone();
		tokio::task::spawn_blocking(move || app_icon_data_url(&settings)).await.ok().flatten().or_else(|| state_image(instance))
	} else if instance.action.uuid == PAGE_INDICATOR_UUID {
		current_page_number(instance).and_then(page_number_image).or_else(|| state_image(instance))
	} else if instance.action.uuid == PAGE_GOTO_UUID
		&& instance.settings.get("showPageNumber").and_then(Value::as_bool).unwrap_or(true)
		&& instance.states.get(instance.current_state as usize).is_some_and(|state| state.text.trim().is_empty())
	{
		let page_number = instance.settings.get("pageIndex").and_then(Value::as_u64).unwrap_or(0).saturating_add(1) as usize;
		let state = instance.states.get(instance.current_state as usize);
		page_number_over_image(page_number, state.map(|state| state.image.as_str())).or_else(|| state_image(instance))
	} else {
		state_image(instance)
	};
	// Nothing the core can draw: the editor renders this key, so leave the LCD
	// untouched instead of blanking it.
	let Some(image) = image else { return Ok(()) };
	crate::events::outbound::devices::update_image((&instance.context).into(), Some(image)).await
}

/// Lock the shared input engine. If an earlier action panicked while holding
/// the lock, the engine is recreated instead of failing every future hotkey.
fn input_engine() -> std::sync::MutexGuard<'static, Option<Enigo>> {
	let store = ENIGO.get_or_init(|| Mutex::new(None));
	store.lock().unwrap_or_else(|poisoned| {
		log::warn!("Recovering the M18 input engine after a panic in an earlier action");
		let mut guard = poisoned.into_inner();
		*guard = None;
		guard
	})
}

/// Release every key or raw key code that was pressed after `before` was
/// captured. Called when an input sequence fails midway, so a modifier such
/// as ⌘ or ⇧ can never stay stuck for all later keyboard input.
fn release_keys_pressed_since(enigo: &mut Enigo, before: &(Vec<enigo::Key>, Vec<u16>)) {
	let (keys, raw_codes) = enigo.held();
	for key in keys.into_iter().rev().filter(|key| !before.0.contains(key)) {
		let _ = enigo.key(key, enigo::Direction::Release);
	}
	for code in raw_codes.into_iter().rev().filter(|code| !before.1.contains(code)) {
		let _ = enigo.raw(code, enigo::Direction::Release);
	}
}

pub(crate) async fn execute_input(input: Option<String>) -> Result<(), anyhow::Error> {
	let Some(input) = input.filter(|value| !value.trim().is_empty()) else {
		return Ok(());
	};
	let tokens: Vec<Token> = ron::from_str(&input).map_err(|error| anyhow::anyhow!("The saved key sequence is not valid ({error}). Record the shortcut again."))?;

	tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
		let mut guard = input_engine();
		if guard.is_none() {
			guard.replace(Enigo::new(&Settings::default())?);
		}
		let enigo = guard.as_mut().expect("input engine was initialised");
		let held_before = enigo.held();
		for token in &tokens {
			if let Err(error) = enigo.execute(token) {
				release_keys_pressed_since(enigo, &held_before);
				return Err(error.into());
			}
		}
		Ok(())
	})
	.await??;
	Ok(())
}

#[tauri::command]
pub async fn get_mouse_position() -> Result<(i32, i32), String> {
	tokio::task::spawn_blocking(|| -> Result<(i32, i32), String> {
		let mut guard = input_engine();
		if guard.is_none() {
			guard.replace(Enigo::new(&Settings::default()).map_err(|error| error.to_string())?);
		}
		guard.as_ref().expect("input engine was initialised").location().map_err(|error| error.to_string())
	})
	.await
	.map_err(|error| error.to_string())?
}

async fn run_process(program: &'static str, args: Vec<String>) -> Result<(), anyhow::Error> {
	tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
		let status = Command::new(program).args(args).status()?;
		if status.success() { Ok(()) } else { Err(anyhow::anyhow!("{program} exited with {status}")) }
	})
	.await??;
	Ok(())
}

async fn open_application(settings: &Value) -> Result<(), anyhow::Error> {
	let Some(app) = string_setting(settings, "appPath") else {
		return Ok(());
	};
	if app.starts_with("/") || app.ends_with(".app") {
		run_process("/usr/bin/open", vec![app]).await
	} else if app.starts_with("com.") {
		run_process("/usr/bin/open", vec!["-b".to_owned(), app]).await
	} else {
		run_process("/usr/bin/open", vec!["-a".to_owned(), app]).await
	}
}

async fn system_command(uuid: &str) -> Result<(), anyhow::Error> {
	match uuid {
		VOLUME_DOWN_UUID => {
			run_process(
				"/usr/bin/osascript",
				vec!["-e".to_owned(), "set volume output volume ((output volume of (get volume settings)) - 6)".to_owned()],
			)
			.await
		}
		VOLUME_UP_UUID => {
			run_process(
				"/usr/bin/osascript",
				vec!["-e".to_owned(), "set volume output volume ((output volume of (get volume settings)) + 6)".to_owned()],
			)
			.await
		}
		MUTE_UUID => {
			run_process(
				"/usr/bin/osascript",
				vec!["-e".to_owned(), "set volume output muted (not (output muted of (get volume settings)))".to_owned()],
			)
			.await
		}
		SIRI_UUID => run_process("/usr/bin/open", vec!["-a".to_owned(), "Siri".to_owned()]).await,
		// VSD Craft's "Dispatch Center" is a translation of 调度中心, which is
		// macOS's Chinese name for Mission Control, not Control Center.
		DISPATCH_CENTER_UUID => run_process("/usr/bin/open", vec!["-a".to_owned(), "Mission Control".to_owned()]).await,
		SCREENSHOT_UUID => run_process("/usr/sbin/screencapture", vec!["-i".to_owned(), "-c".to_owned()]).await,
		LAUNCHPAD_UUID => run_process("/usr/bin/open", vec!["-a".to_owned(), "Launchpad".to_owned()]).await,
		DESKTOP_SAVER_UUID => run_process("/usr/bin/open", vec!["-a".to_owned(), "ScreenSaverEngine".to_owned()]).await,
		SLEEP_UUID => run_process("/usr/bin/pmset", vec!["displaysleepnow".to_owned()]).await,
		SCREEN_BRIGHTNESS_UP_UUID => run_process("/usr/bin/osascript", vec!["-e".to_owned(), "tell application \"System Events\" to key code 144".to_owned()]).await,
		SCREEN_BRIGHTNESS_DOWN_UUID => run_process("/usr/bin/osascript", vec!["-e".to_owned(), "tell application \"System Events\" to key code 145".to_owned()]).await,
		// Real media keys, like the hardware keys: they control whichever player is
		// active (Music, Spotify, a browser) and never launch Music unasked.
		PREVIOUS_TRACK_UUID => execute_input(Some("[k(MediaPrevTrack)]".to_owned())).await,
		PLAY_PAUSE_UUID => execute_input(Some("[k(MediaPlayPause)]".to_owned())).await,
		NEXT_TRACK_UUID => execute_input(Some("[k(MediaNextTrack)]".to_owned())).await,
		_ => Ok(()),
	}
}

pub async fn key_down(instance: &ActionInstance) -> Result<(), anyhow::Error> {
	match instance.action.uuid.as_str() {
		SUPER_HOTKEYS_UUID => execute_input(string_setting(&instance.settings, "down")).await,
		HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID => {
			let input = instance
				.settings
				.get("hotkeys")
				.and_then(Value::as_array)
				.and_then(|hotkeys| hotkeys.get(instance.current_state as usize))
				.and_then(|hotkey| hotkey.get("down"))
				.and_then(Value::as_str)
				.map(str::to_owned);
			execute_input(input).await
		}
		_ if crate::vsd_actions::is_vsd_action(&instance.action.uuid) => crate::vsd_actions::key_down(instance).await,
		_ => Ok(()),
	}
}

/// Executes the action's release behavior. The return value tells the caller
/// whether the action's visual state should advance.
pub async fn key_up(instance: &ActionInstance) -> Result<bool, anyhow::Error> {
	match instance.action.uuid.as_str() {
		SUPER_HOTKEYS_UUID => {
			execute_input(string_setting(&instance.settings, "up")).await?;
			Ok(false)
		}
		HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID => {
			let input = instance
				.settings
				.get("hotkeys")
				.and_then(Value::as_array)
				.and_then(|hotkeys| hotkeys.get(instance.current_state as usize))
				.and_then(|hotkey| hotkey.get("up"))
				.and_then(Value::as_str)
				.map(str::to_owned);
			execute_input(input).await?;
			Ok(true)
		}
		OPEN_APPS_UUID => {
			open_application(&instance.settings).await?;
			Ok(false)
		}
		VOLUME_DOWN_UUID
		| VOLUME_UP_UUID
		| MUTE_UUID
		| SIRI_UUID
		| DISPATCH_CENTER_UUID
		| SCREENSHOT_UUID
		| LAUNCHPAD_UUID
		| DESKTOP_SAVER_UUID
		| SLEEP_UUID
		| SCREEN_BRIGHTNESS_UP_UUID
		| SCREEN_BRIGHTNESS_DOWN_UUID
		| PREVIOUS_TRACK_UUID
		| PLAY_PAUSE_UUID
		| NEXT_TRACK_UUID => {
			system_command(&instance.action.uuid).await?;
			Ok(false)
		}
		PAGE_PREVIOUS_UUID => {
			crate::m18_pages::switch_to(&instance.context.device, None, None, -1).await?;
			Ok(false)
		}
		PAGE_NEXT_UUID => {
			crate::m18_pages::switch_to(&instance.context.device, None, None, 1).await?;
			Ok(false)
		}
		PAGE_GOTO_UUID => {
			let target = string_setting(&instance.settings, "page");
			let index = instance.settings.get("pageIndex").and_then(Value::as_u64).map(|value| value as usize);
			crate::m18_pages::switch_to(&instance.context.device, target.as_deref(), index, 0).await?;
			Ok(false)
		}
		_ if crate::vsd_actions::is_vsd_action(&instance.action.uuid) => crate::vsd_actions::key_up(instance).await,
		_ => Ok(false),
	}
}
