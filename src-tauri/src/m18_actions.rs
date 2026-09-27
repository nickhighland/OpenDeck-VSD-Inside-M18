//! Native actions for the M18 workflow.
//!
//! These actions live in the application core. They do not require a second
//! action-plugin process, which is important for the actions used by the
//! user's everyday M18 profile.

use crate::shared::{ActionInstance, ActionState};

use base64::Engine;
use enigo::{
	Enigo, Keyboard, Mouse, Settings,
	agent::{Agent, Token},
};
use image::{Rgb, RgbImage};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

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
static ICON_LOOKUPS: OnceLock<Mutex<HashMap<String, IconLookup>>> = OnceLock::new();

/// One icon lookup per launch target. Requests made while it runs share it,
/// so the editor and the core never start the same slow lookup twice.
struct IconLookup {
	started: std::time::Instant,
	icon: Arc<tokio::sync::OnceCell<Option<String>>>,
}

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

pub(crate) fn image_data_url(image: &str) -> Option<String> {
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

/// A macOS script that finds an app the way `open -a` does (by path, bundle
/// identifier, or name), or takes any file or folder, and writes the icon
/// Finder shows for it as a 256-pixel PNG. It prints the resolved path, or
/// `missing` when there is nothing to show.
#[cfg(target_os = "macos")]
const ICON_SCRIPT: &str = r#"
function run(argv) {
	ObjC.import("AppKit");
	const target = argv[0];
	const output = argv[1];
	const size = 256;
	const workspace = $.NSWorkspace.sharedWorkspace;
	let path = null;
	if (target.startsWith("/") || target.startsWith("~")) {
		path = $(target).stringByExpandingTildeInPath.js;
	} else {
		const byIdentifier = workspace.URLForApplicationWithBundleIdentifier(target);
		if (byIdentifier && !byIdentifier.isNil()) path = byIdentifier.path.js;
		if (!path) {
			const byName = workspace.fullPathForApplication(target.replace(/\.app$/i, ""));
			if (byName && !byName.isNil()) path = byName.js;
		}
	}
	if (!path || !$.NSFileManager.defaultManager.fileExistsAtPath(path)) return "missing";
	const icon = workspace.iconForFile(path);
	const rep = $.NSBitmapImageRep.alloc.initWithBitmapDataPlanesPixelsWidePixelsHighBitsPerSampleSamplesPerPixelHasAlphaIsPlanarColorSpaceNameBytesPerRowBitsPerPixel(null, size, size, 8, 4, true, false, $.NSDeviceRGBColorSpace, 0, 0);
	$.NSGraphicsContext.saveGraphicsState;
	$.NSGraphicsContext.setCurrentContext($.NSGraphicsContext.graphicsContextWithBitmapImageRep(rep));
	icon.drawInRectFromRectOperationFraction($.NSMakeRect(0, 0, size, size), $.NSZeroRect, $.NSCompositingOperationSourceOver, 1.0);
	$.NSGraphicsContext.restoreGraphicsState;
	rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $()).writeToFileAtomically(output, true);
	return path;
}
"#;

/// Missing icons are looked up again after this long, so an app installed
/// later gets its icon without restarting.
const MISSING_ICON_RETRY: std::time::Duration = std::time::Duration::from_secs(60);

/// What a key that launches or opens something shows by default: the app's,
/// file's, or folder's own icon. `None` for actions that open nothing.
pub fn icon_target(uuid: &str, settings: &Value) -> Option<String> {
	let first = |keys: &[&str]| keys.iter().find_map(|key| string_setting(settings, key));
	let uuid = uuid.to_ascii_lowercase();
	match uuid.as_str() {
		OPEN_APPS_UUID => string_setting(settings, "appPath"),
		"com.hotspot.streamdock.system.openapps" => first(&["appPath", "path", "application", "app"]),
		// Websites have no Finder icon; files, folders, and apps do.
		"com.hotspot.streamdock.system.open" => first(&["path", "Path", "file", "folder"]).filter(|target| !target.contains("://")),
		_ => crate::vsd_actions::app_for_uuid(&uuid).map(str::to_owned),
	}
}

#[cfg(target_os = "macos")]
fn generate_icon(target: &str) -> Option<String> {
	use std::sync::atomic::{AtomicU64, Ordering};
	static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

	let directory = std::env::temp_dir().join("opendeck-vsd-m18-icons");
	fs::create_dir_all(&directory).ok()?;
	let output = directory.join(format!("{}-{}.png", std::process::id(), NEXT_FILE.fetch_add(1, Ordering::Relaxed)));
	let mut child = Command::new("/usr/bin/osascript")
		.args(["-l", "JavaScript", "-e", ICON_SCRIPT, target])
		.arg(&output)
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.spawn()
		.ok()?;
	// A slow volume or a stuck LaunchServices query must not hold a key's
	// render forever.
	let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
	let finished = loop {
		match child.try_wait() {
			Ok(Some(status)) => break status.success(),
			Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(20)),
			_ => {
				let _ = child.kill();
				let _ = child.wait();
				break false;
			}
		}
	};
	let data = finished.then(|| fs::read(&output).ok()).flatten();
	let _ = fs::remove_file(&output);
	data.filter(|data| !data.is_empty())
		.map(|data| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(data)))
}

#[cfg(not(target_os = "macos"))]
fn generate_icon(_target: &str) -> Option<String> {
	None
}

fn icon_lookup(target: &str) -> Arc<tokio::sync::OnceCell<Option<String>>> {
	let mut lookups = ICON_LOOKUPS.get_or_init(Default::default).lock().unwrap_or_else(PoisonError::into_inner);
	if let Some(lookup) = lookups.get(target)
		&& !(matches!(lookup.icon.get(), Some(None)) && lookup.started.elapsed() >= MISSING_ICON_RETRY)
	{
		return lookup.icon.clone();
	}
	let icon = Arc::new(tokio::sync::OnceCell::new());
	lookups.insert(
		target.to_owned(),
		IconLookup {
			started: std::time::Instant::now(),
			icon: icon.clone(),
		},
	);
	icon
}

/// The default icon for a launching action, generated once per target and cached.
pub async fn action_icon(uuid: &str, settings: &Value) -> Option<String> {
	let target = icon_target(uuid, settings)?;
	icon_lookup(&target)
		.get_or_init(|| async move { tokio::task::spawn_blocking(move || generate_icon(&target)).await.ok().flatten() })
		.await
		.clone()
}

/// The icon if it has already been looked up. Otherwise the lookup starts in
/// the background and this returns `None`: a page switch must never wait for
/// Launch Services, and the editor draws the key once the icon is ready.
fn cached_action_icon(uuid: &str, settings: &Value) -> Option<String> {
	let target = icon_target(uuid, settings)?;
	if let Some(icon) = icon_lookup(&target).get() {
		return icon.clone();
	}
	let (uuid, settings) = (uuid.to_owned(), settings.clone());
	tauri::async_runtime::spawn(async move { action_icon(&uuid, &settings).await });
	None
}

/// The editor asks for this to draw launching keys that have no custom image.
#[tauri::command]
pub async fn get_action_icon(uuid: String, settings: Value) -> Option<String> {
	action_icon(&uuid, &settings).await
}

/// Whether a state shows the action's automatic artwork rather than an image
/// the user chose.
pub fn uses_default_artwork(image: &str) -> bool {
	image.is_empty() || image.starts_with("opendeck/")
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

/// How much of the key an app icon covers, in percent, without and with a
/// title along the bottom. Mirrors `resolveState()` in `src/lib/appIcons.ts`.
const APP_ICON_SCALE: u32 = 84;
const TITLED_APP_ICON_SCALE: u32 = 62;

fn shows_bottom_title(state: &ActionState) -> bool {
	state.show && !state.text.trim().is_empty() && state.alignment == "bottom"
}

fn hex_colour(colour: &str) -> Option<Rgb<u8>> {
	let hex = colour.strip_prefix('#')?;
	let channel = |index: usize| hex.get(index..index + 2).and_then(|value| u8::from_str_radix(value, 16).ok());
	let alpha = if hex.len() == 8 { channel(6)? as u16 } else { 255 };
	// The editor paints the colour over black, so translucency darkens it.
	let blend = |value: u8| (value as u16 * alpha / 255) as u8;
	(hex.len() == 6 || hex.len() == 8).then_some(Rgb([blend(channel(0)?), blend(channel(2)?), blend(channel(4)?)]))
}

/// Lay out an app icon the way the editor does (on the key's background
/// colour, raised above a bottom title), so the editor's redraw, which adds
/// the title, does not visibly move or resize it on the M18.
fn app_icon_face(icon: &str, state: Option<&ActionState>) -> Option<String> {
	const SIZE: u32 = 144;
	let bytes = base64::engine::general_purpose::STANDARD.decode(icon.split_once(',')?.1).ok()?;
	let decoded = image::load_from_memory(&bytes).ok()?;
	let title = state.filter(|state| shows_bottom_title(state));
	// The key's own image scale applies on top of the automatic one.
	let chosen = state.map_or(100, |state| if state.image_scale == 0 { 100 } else { state.image_scale.max(10) as u32 });
	let percent = ((if title.is_some() { TITLED_APP_ICON_SCALE } else { APP_ICON_SCALE }) * chosen + 50) / 100;
	let side = (SIZE * percent / 100).max(1);
	let x = (SIZE as i64 - side as i64) / 2;
	let y = match title {
		Some(state) if side < SIZE => {
			let lines = state.text.split('\n').count() as f32;
			let title_height = state.size.0 as f32 * 2.0 * lines + state.stroke_size.0 as f32 * 2.0;
			((SIZE as f32 - title_height - side as f32) / 2.0).max(SIZE as f32 * 0.04).round() as i64
		}
		_ => x,
	};
	let Rgb([red, green, blue]) = state.and_then(|state| hex_colour(&state.background_colour)).unwrap_or(Rgb([0, 0, 0]));
	let mut face = image::RgbaImage::from_pixel(SIZE, SIZE, image::Rgba([red, green, blue, 255]));
	image::imageops::overlay(&mut face, &decoded.resize_exact(side, side, image::imageops::FilterType::Lanczos3).to_rgba8(), x, y);
	encode_key_image(&image::DynamicImage::ImageRgba8(face).to_rgb8())
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
	fn app_icons_are_laid_out_like_the_editor_draws_them() {
		let mut bytes = std::io::Cursor::new(Vec::new());
		image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(256, 256, image::Rgba([220, 30, 30, 255])))
			.write_to(&mut bytes, image::ImageFormat::Png)
			.unwrap();
		let icon = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes.into_inner()));
		let decode = |face: String| {
			let data = face.strip_prefix("data:image/jpeg;base64,").expect("JPEG data URL");
			image::load_from_memory(&base64::engine::general_purpose::STANDARD.decode(data).unwrap()).unwrap().to_rgb8()
		};
		let red = |pixel: &Rgb<u8>| pixel[0] > 180 && pixel[1] < 70;
		let black = |pixel: &Rgb<u8>| pixel.0.iter().all(|channel| *channel < 40);

		// Without a title the icon is centred with a margin all round.
		let plain = ActionState { show: false, ..Default::default() };
		let face = decode(app_icon_face(&icon, Some(&plain)).unwrap());
		assert_eq!(face.dimensions(), (144, 144));
		assert!(red(face.get_pixel(72, 72)) && red(face.get_pixel(72, 128)));
		assert!(black(face.get_pixel(3, 3)) && black(face.get_pixel(72, 140)));

		// A bottom title gets the lower part of the key to itself.
		let titled = ActionState {
			show: true,
			text: "Safari".to_owned(),
			alignment: "bottom".to_owned(),
			background_colour: "#1e40af".to_owned(),
			..Default::default()
		};
		let face = decode(app_icon_face(&icon, Some(&titled)).unwrap());
		assert!(red(face.get_pixel(72, 40)));
		let below = face.get_pixel(72, 122);
		assert!(below[2] > 140 && below[0] < 70, "the title area keeps the key's background colour");

		assert_eq!(hex_colour("#00ff00"), Some(Rgb([0, 255, 0])));
		assert_eq!(hex_colour("#ff000080"), Some(Rgb([128, 0, 0])));
		assert_eq!(hex_colour("#abc"), None);
		assert_eq!(hex_colour("red"), None);
	}

	/// Needs a macOS desktop session: cargo test finds_app_icons -- --ignored
	#[cfg(target_os = "macos")]
	#[test]
	#[ignore = "runs osascript in a macOS desktop session"]
	fn finds_app_icons_with_launch_services() {
		for target in ["Calculator", "com.apple.Safari", "/System/Applications/Music.app"] {
			let icon = generate_icon(target).unwrap_or_else(|| panic!("no icon for {target}"));
			let data = icon.strip_prefix("data:image/png;base64,").expect("PNG data URL");
			let bytes = base64::engine::general_purpose::STANDARD.decode(data).unwrap();
			let decoded = image::load_from_memory(&bytes).unwrap();
			assert_eq!((decoded.width(), decoded.height()), (256, 256), "{target}");
		}
		assert!(generate_icon("No Such Application 7f3a").is_none());
	}

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
	let current = instance.states.get(instance.current_state as usize);
	let image = if instance.action.uuid == PAGE_INDICATOR_UUID {
		current_page_number(instance).and_then(page_number_image).or_else(|| state_image(instance))
	} else if instance.action.uuid == PAGE_GOTO_UUID && instance.settings.get("showPageNumber").and_then(Value::as_bool).unwrap_or(true) && current.is_some_and(|state| state.text.trim().is_empty()) {
		let page_number = instance.settings.get("pageIndex").and_then(Value::as_u64).unwrap_or(0).saturating_add(1) as usize;
		page_number_over_image(page_number, current.map(|state| state.image.as_str())).or_else(|| state_image(instance))
	} else if current.is_none_or(|state| uses_default_artwork(&state.image))
		&& let Some(icon) = cached_action_icon(&instance.action.uuid, &instance.settings)
	{
		// A launching key shows its app's icon until the user picks an image.
		app_icon_face(&icon, current).or(Some(icon))
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

/// Logged, and shown on the key, when macOS refuses simulated input.
const INPUT_PERMISSION_ERROR: &str = "macOS does not allow this version to send keystrokes (System Settings > Privacy & Security > Accessibility)";

/// Create the input engine. macOS's own permission prompt appears for the
/// first attempt in each launch only. macOS ties the Accessibility
/// permission to the exact build, so after an update the app can still look
/// allowed in System Settings while this build is refused; rather than the
/// prompt reopening on every key press, the editor explains how to fix it.
fn new_input_engine() -> Result<Enigo, anyhow::Error> {
	static PROMPTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
	let settings = Settings {
		open_prompt_to_get_permissions: !PROMPTED.swap(true, std::sync::atomic::Ordering::Relaxed),
		..Settings::default()
	};
	Enigo::new(&settings).map_err(|error| match error {
		enigo::NewConError::NoPermission => {
			if let Some(app) = crate::APP_HANDLE.get() {
				let _ = tauri::Emitter::emit(app, "input_permission_missing", ());
			}
			anyhow::anyhow!(INPUT_PERMISSION_ERROR)
		}
		error => error.into(),
	})
}

/// Whether macOS lets this build send keystrokes and mouse input; `None` on
/// systems that need no such permission.
#[tauri::command]
pub async fn get_input_permission() -> Option<bool> {
	#[cfg(target_os = "macos")]
	{
		#[link(name = "ApplicationServices", kind = "framework")]
		unsafe extern "C" {
			fn AXIsProcessTrusted() -> bool;
		}
		// Takes no arguments; it only asks the system about this process.
		Some(unsafe { AXIsProcessTrusted() })
	}
	#[cfg(not(target_os = "macos"))]
	{
		None
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
			guard.replace(new_input_engine()?);
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
			guard.replace(new_input_engine().map_err(|error| error.to_string())?);
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
		UNSUPPORTED_VSD_UUID => Err(anyhow::anyhow!(
			"The imported VSD Craft action \"{}\" is not supported yet",
			instance.settings.get("sourceName").and_then(Value::as_str).unwrap_or("unknown")
		)),
		_ => Ok(false),
	}
}
