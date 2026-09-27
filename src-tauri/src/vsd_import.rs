//! VSD Craft profile migration.
//!
//! VSD Craft stores an M18 as five LCD columns with three rows, plus a separate
//! bottom row of three physical buttons. OpenDeck's internal position numbering
//! is retained for action/plugin compatibility: LCD positions are `x + y * 5`
//! and the bottom buttons are `15 + y`.

use crate::events::frontend::Error;
use crate::shared::{Action, ActionContext, ActionInstance, ActionState, CATEGORIES, Category, DeviceInfo, Profile};
use crate::store::profiles::acquire_locks_mut;

use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, command};
use tauri_plugin_dialog::{DialogExt, FilePath};

const M18_DEVICE_NAMESPACE: &str = "18";
const STARTER_PLUGIN: &str = "com.amansprojects.starterpack.sdPlugin";
const PROFILE_PREFIX: &str = "VSD Craft";

#[derive(Clone, Debug, serde::Serialize)]
pub struct ImportSummary {
	pub device: String,
	pub profiles: Vec<String>,
	pub imported_actions: usize,
	pub unsupported_actions: Vec<String>,
	pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
struct VsdManifest {
	#[serde(rename = "Actions", default)]
	actions: HashMap<String, VsdAction>,
	#[serde(rename = "DeviceSerialNumber", default)]
	device_serial_number: String,
	#[serde(rename = "DeviceUUID", default)]
	device_uuid: String,
	#[serde(rename = "Name", default)]
	name: String,
	#[serde(rename = "Pages", default)]
	pages: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct VsdAction {
	#[serde(rename = "Controller", default)]
	controller: String,
	#[serde(rename = "Name", default)]
	name: String,
	#[serde(rename = "Settings", default)]
	settings: Value,
	#[serde(rename = "State", default)]
	state: u16,
	#[serde(rename = "States", default)]
	states: Vec<VsdState>,
	#[serde(rename = "UUID", default)]
	uuid: String,
	#[serde(rename = "MultiActionData", default)]
	multi_action_data: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct VsdState {
	#[serde(rename = "Image", default)]
	image: String,
	#[serde(rename = "Title", default)]
	title: String,
}

#[derive(Clone)]
struct PageSpec {
	path: PathBuf,
	manifest: VsdManifest,
	profile_id: String,
}

#[derive(Clone, Copy)]
enum Mapping {
	MultiAction,
	ActionCarousel,
	ActionCycle,
	NativeOpenApps,
	NativeSuperHotkeys,
	NativeHotkeySwitch,
	NativeVolumeDown,
	NativeVolumeUp,
	NativeMute,
	NativeSiri,
	NativeSystemControl,
	NativePagePrevious,
	NativePageNext,
	NativePageGoto,
	NativePageIndicator,
	RunCommand,
	OpenUrl,
	SwitchProfile,
	DeviceBrightness,
	NativeVsdAction,
	Unsupported,
}

fn read_manifest(path: &Path) -> Result<VsdManifest, anyhow::Error> {
	let contents = fs::read_to_string(path)?;
	Ok(serde_json::from_str(&contents)?)
}

fn manifest_path(input: PathBuf) -> PathBuf {
	if input.is_dir() { input.join("manifest.json") } else { input }
}

fn safe_profile_name(name: &str) -> String {
	let mut name = name.trim().replace(['/', '\\'], "_");
	if name.is_empty() {
		name = "Imported".to_owned();
	}
	format!("{PROFILE_PREFIX}/{name}")
}

fn page_names(value: &Value) -> Vec<String> {
	value
		.get("Pages")
		.and_then(Value::as_array)
		.into_iter()
		.flatten()
		.filter_map(Value::as_str)
		.map(str::to_owned)
		.collect()
}

fn locate_page_manifest(root: &Path, page_name: &str) -> Option<PathBuf> {
	let candidates = [
		root.join(page_name).join("manifest.json"),
		root.join("profiles").join(page_name).join("manifest.json"),
		root.join("profiles").join(format!("{page_name}.sdProfile")).join("manifest.json"),
	];

	for candidate in candidates {
		if candidate.is_file() {
			return candidate.canonicalize().ok();
		}
	}
	None
}

fn collect_pages(root_manifest: &Path, root: &VsdManifest) -> Vec<PageSpec> {
	let Some(root_dir) = root_manifest.parent() else { return vec![] };
	let root_dir = root_dir.canonicalize().unwrap_or_else(|_| root_dir.to_owned());
	let mut paths = vec![root_manifest.canonicalize().unwrap_or_else(|_| root_manifest.to_owned())];
	for page_name in page_names(&root.pages) {
		if let Some(path) = locate_page_manifest(&root_dir, &page_name)
			&& path.starts_with(&root_dir)
			&& !paths.contains(&path)
		{
			paths.push(path);
		}
	}

	let mut used_names = HashSet::new();
	paths
		.into_iter()
		.filter_map(|path| {
			let manifest = read_manifest(&path).ok()?;
			let base_name = safe_profile_name(&manifest.name);
			let mut profile_id = base_name.clone();
			let mut suffix = 2;
			while !used_names.insert(profile_id.clone()) {
				profile_id = format!("{base_name} {suffix}");
				suffix += 1;
			}
			Some(PageSpec { path, manifest, profile_id })
		})
		.collect()
}

fn mapping(uuid: &str, _name: &str) -> Mapping {
	let uuid = uuid.to_ascii_lowercase();
	if uuid == "com.hotspot.streamdock.multiactions.routine" {
		return Mapping::MultiAction;
	}
	if uuid == "com.hotspot.streamdock.multiactions.lunbo" {
		return Mapping::ActionCarousel;
	}
	if uuid == "com.hotspot.streamdock.multiactions.toggle" {
		return Mapping::ActionCycle;
	}

	if uuid.contains("hotkeyswitch") {
		return Mapping::NativeHotkeySwitch;
	}
	if uuid.contains("super.hotkey") {
		return Mapping::NativeSuperHotkeys;
	}
	if uuid.contains("openapps") {
		return Mapping::NativeOpenApps;
	}
	if uuid.contains("touchbar.volumedown") {
		return Mapping::NativeVolumeDown;
	}
	if uuid.contains("touchbar.volumeup") {
		return Mapping::NativeVolumeUp;
	}
	if uuid.contains("touchbar.mute") {
		return Mapping::NativeMute;
	}
	if uuid.contains("touchbar.siri") {
		return Mapping::NativeSiri;
	}
	if uuid.contains(".dispatchcenter")
		|| uuid.contains(".screenshot")
		|| uuid.contains(".launchpad")
		|| uuid.contains(".desktopsaver")
		|| uuid.ends_with(".sleep")
		|| uuid.contains("increasescreenbrightness")
		|| uuid.contains("decreasescreenbrightness")
		|| uuid.contains(".previoustrack")
		|| uuid.contains(".playpause")
		|| uuid.contains(".nexttrack")
	{
		return Mapping::NativeSystemControl;
	}
	if uuid.contains("page.previous") {
		return Mapping::NativePagePrevious;
	}
	if uuid.contains("page.next") {
		return Mapping::NativePageNext;
	}
	if uuid.contains("page.goto") {
		return Mapping::NativePageGoto;
	}
	if uuid.contains("page.indicator") {
		return Mapping::NativePageIndicator;
	}
	// M18-applicable VSD actions execute in the application core; only composite
	// actions with unvalidated child serialization remain on the separate path.
	if crate::vsd_actions::definition(&uuid).is_some() && !crate::vsd_actions::is_composite_action(&uuid) {
		return Mapping::NativeVsdAction;
	}
	if uuid.ends_with(".open") || uuid.contains(".open.") {
		return Mapping::RunCommand;
	}
	if uuid.contains("website") {
		return Mapping::OpenUrl;
	}
	if uuid.contains("page.change") || uuid.contains("profile.rotate") || uuid.contains("profile.backtoparent") || uuid.contains("profile.openchild") {
		return Mapping::SwitchProfile;
	}
	if uuid.contains("device.brightness") {
		return Mapping::DeviceBrightness;
	}

	if uuid.contains("volumedown")
		|| uuid.contains("volumeup")
		|| uuid.contains("touchbar.volume")
		|| uuid.contains(".mute")
		|| uuid.contains(".siri")
		|| uuid.contains(".launchpad")
		|| uuid.contains(".dispatchcenter")
		|| uuid.contains(".screenshot")
		|| uuid.contains(".sleep")
		|| uuid.contains(".desktopsaver")
		|| uuid.contains(".playpause")
		|| uuid.contains(".previoustrack")
		|| uuid.contains(".nexttrack")
		|| uuid.contains("screen.brightness")
		|| uuid.contains("increasescreenbrightness")
		|| uuid.contains("decreasescreenbrightness")
		|| uuid.contains("system.volume")
		|| uuid.contains("system.multimedia")
		|| uuid.contains("device.devsleep")
	{
		return Mapping::RunCommand;
	}

	Mapping::Unsupported
}

fn fallback_action(uuid: &str, name: &str, plugin: &str) -> Action {
	let state = ActionState {
		image: "opendeck/multi-action.png".to_owned(),
		..Default::default()
	};
	Action {
		name: name.to_owned(),
		uuid: uuid.to_owned(),
		plugin: plugin.to_owned(),
		tooltip: name.to_owned(),
		icon: state.image.clone(),
		disable_automatic_states: false,
		visible_in_action_list: true,
		supported_in_multi_actions: true,
		property_inspector: String::new(),
		controllers: vec!["Keypad".to_owned()],
		encoder: None,
		states: vec![state],
	}
}

fn action_from_categories(categories: &HashMap<String, Category>, uuid: &str, name: &str, plugin: &str) -> Action {
	if let Some(action) = categories.values().flat_map(|category| category.actions.iter()).find(|action| action.uuid == uuid) {
		return action.clone();
	}
	fallback_action(uuid, name, plugin)
}

fn native_system_control(uuid: &str) -> (&'static str, &'static str) {
	let uuid = uuid.to_ascii_lowercase();
	if uuid.contains(".dispatchcenter") {
		(crate::m18_actions::DISPATCH_CENTER_UUID, "Dispatch Center")
	} else if uuid.contains(".screenshot") {
		(crate::m18_actions::SCREENSHOT_UUID, "Screenshot")
	} else if uuid.contains(".launchpad") {
		(crate::m18_actions::LAUNCHPAD_UUID, "Launchpad")
	} else if uuid.contains(".desktopsaver") {
		(crate::m18_actions::DESKTOP_SAVER_UUID, "Desktop Saver")
	} else if uuid.ends_with(".sleep") {
		(crate::m18_actions::SLEEP_UUID, "Sleep")
	} else if uuid.contains("increasescreenbrightness") {
		(crate::m18_actions::SCREEN_BRIGHTNESS_UP_UUID, "Increase screen brightness")
	} else if uuid.contains("decreasescreenbrightness") {
		(crate::m18_actions::SCREEN_BRIGHTNESS_DOWN_UUID, "Decrease screen brightness")
	} else if uuid.contains(".previoustrack") {
		(crate::m18_actions::PREVIOUS_TRACK_UUID, "Previous Track")
	} else if uuid.contains(".playpause") {
		(crate::m18_actions::PLAY_PAUSE_UUID, "Play/Pause")
	} else {
		(crate::m18_actions::NEXT_TRACK_UUID, "Next Track")
	}
}

fn numeric_setting(settings: &Value, key: &str) -> Option<usize> {
	settings.get(key).and_then(|value| value.as_u64().map(|value| value as usize).or_else(|| value.as_str()?.parse().ok()))
}

fn native_page_index(settings: &Value, fallback: usize) -> usize {
	numeric_setting(settings, "PageIndex").map(|page_number| page_number.saturating_sub(1)).unwrap_or(fallback)
}

fn show_page_number(settings: &Value) -> bool {
	settings
		.as_object()
		.and_then(|settings| {
			settings.iter().find_map(|(key, value)| {
				let normalized = key.to_ascii_lowercase().replace('_', "");
				(normalized == "showpagenumber").then(|| value.as_bool()).flatten()
			})
		})
		.unwrap_or(true)
}

fn setting_string(settings: &Value, keys: &[&str]) -> Option<String> {
	keys.iter().find_map(|key| settings.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn shell_quote(value: &str) -> String {
	format!("'{}'", value.replace('\'', "'\\''"))
}

fn app_command(settings: &Value) -> String {
	let application = application_target(settings);

	if application.is_empty() {
		"true".to_owned()
	} else {
		format!("open -a {}", shell_quote(&application))
	}
}

fn application_target(settings: &Value) -> String {
	settings
		.get("applications")
		.and_then(Value::as_array)
		.and_then(|applications| numeric_setting(settings, "appId").and_then(|index| applications.get(index)))
		.and_then(|application| application.get("path").and_then(Value::as_str).or_else(|| application.get("name").and_then(Value::as_str)))
		.map(str::to_owned)
		.or_else(|| setting_string(settings, &["path", "Path", "application", "app"]))
		.unwrap_or_default()
}

fn open_command(settings: &Value) -> Option<String> {
	let value = setting_string(settings, &["path", "Path", "url", "URL", "website", "link", "file", "folder"])?;
	if value.is_empty() { None } else { Some(format!("open {}", shell_quote(&value))) }
}

fn system_command(uuid: &str, settings: &Value) -> String {
	let uuid = uuid.to_ascii_lowercase();
	let action_index = settings.get("actionIdx").and_then(Value::as_i64).unwrap_or(0);
	match () {
		_ if uuid.contains("volumedown") => "osascript -e 'set volume output volume ((output volume of (get volume settings)) - 6)'".to_owned(),
		_ if uuid.contains("volumeup") => "osascript -e 'set volume output volume ((output volume of (get volume settings)) + 6)'".to_owned(),
		_ if uuid.contains("touchbar.volume") || uuid.contains("system.volume") => {
			let value = settings.get("value").and_then(Value::as_i64).unwrap_or(50).clamp(0, 100);
			match action_index {
				1 => "osascript -e 'set volume output volume ((output volume of (get volume settings)) - 6)'".to_owned(),
				2 => "osascript -e 'set volume output volume ((output volume of (get volume settings)) + 6)'".to_owned(),
				3 => "osascript -e 'set volume output muted (not (output muted of (get volume settings)))'".to_owned(),
				_ => format!("osascript -e 'set volume output volume {value}'"),
			}
		}
		_ if uuid.contains(".mute") => "osascript -e 'set volume output muted (not (output muted of (get volume settings)))'".to_owned(),
		_ if uuid.contains(".siri") => "open -a Siri".to_owned(),
		_ if uuid.contains(".launchpad") => "open -a Launchpad".to_owned(),
		_ if uuid.contains(".dispatchcenter") => "open 'x-apple.systempreferences:com.apple.ControlCenter-Settings.extension'".to_owned(),
		_ if uuid.contains(".screenshot") => "screencapture -i ~/Desktop/VSD-Screenshot.png".to_owned(),
		_ if uuid.contains(".sleep") => "pmset displaysleepnow".to_owned(),
		_ if uuid.contains(".desktopsaver") => "open -a 'Screen Saver'".to_owned(),
		_ if uuid.contains(".playpause") => "osascript -e 'tell application \"Music\" to playpause'".to_owned(),
		_ if uuid.contains(".previoustrack") => "osascript -e 'tell application \"Music\" to previous track'".to_owned(),
		_ if uuid.contains(".nexttrack") => "osascript -e 'tell application \"Music\" to next track'".to_owned(),
		_ if uuid.contains("increasescreenbrightness") => "osascript -e 'tell application \"System Events\" to key code 144'".to_owned(),
		_ if uuid.contains("decreasescreenbrightness") => "osascript -e 'tell application \"System Events\" to key code 145'".to_owned(),
		_ if uuid.contains("screen.brightness") => "true".to_owned(),
		_ if uuid.contains("system.multimedia") => match action_index {
			1 => "osascript -e 'tell application \"Music\" to playpause'".to_owned(),
			2 => "osascript -e 'tell application \"Music\" to previous track'".to_owned(),
			3 => "osascript -e 'tell application \"Music\" to next track'".to_owned(),
			_ => "true".to_owned(),
		},
		_ if uuid.contains("device.devsleep") => "pmset displaysleepnow".to_owned(),
		_ => "true".to_owned(),
	}
}

fn mac_key_label(code: u64) -> String {
	// Apple's HIToolbox Events.h virtual-key table. Unknown/layout-dependent
	// keys are intentionally shown as codes rather than guessed letters.
	match code {
		0x24 => "Return".to_owned(),
		0x30 => "Tab".to_owned(),
		0x31 => "Space".to_owned(),
		0x33 => "Delete".to_owned(),
		0x35 => "Escape".to_owned(),
		0x40 => "F17".to_owned(),
		0x4f => "F18".to_owned(),
		0x50 => "F19".to_owned(),
		0x5a => "F20".to_owned(),
		0x60 => "F5".to_owned(),
		0x61 => "F6".to_owned(),
		0x62 => "F7".to_owned(),
		0x63 => "F3".to_owned(),
		0x64 => "F8".to_owned(),
		0x65 => "F9".to_owned(),
		0x67 => "F11".to_owned(),
		0x69 => "F13".to_owned(),
		0x6a => "F16".to_owned(),
		0x6b => "F14".to_owned(),
		0x6d => "F10".to_owned(),
		0x6f => "F12".to_owned(),
		0x71 => "F15".to_owned(),
		0x76 => "F4".to_owned(),
		0x78 => "F2".to_owned(),
		0x7a => "F1".to_owned(),
		0x7b => "Left Arrow".to_owned(),
		0x7c => "Right Arrow".to_owned(),
		0x7d => "Down Arrow".to_owned(),
		0x7e => "Up Arrow".to_owned(),
		_ => format!("Mac key code {code}"),
	}
}

fn hotkey_settings(settings: &Value, hotkey_index: usize) -> Value {
	let hotkeys = settings.get("Hotkeys").and_then(Value::as_array);
	let Some(hotkey) = hotkeys.and_then(|hotkeys| hotkeys.get(hotkey_index)) else {
		return json!({});
	};

	// VSD Craft stores macOS virtual key codes, not Unicode/ASCII values. In
	// particular 79=F18, 107=F14, and 113=F15 on the user's M18 profile.
	// Enigo's `r(code)` sends that physical key; `k(Key,Direction)` wraps a
	// complete chord. KeyModifiers=65536 appears even for an unmodified key in
	// VSD Craft, so use the explicit modifier booleans instead of that mask.
	let mut modifiers: Vec<&str> = Vec::new();
	for (key, token) in [
		("KeyCmd", "Meta"),
		("RKeyCmd", "RCommand"),
		("KeyCtrl", "Control"),
		("RKeyCtrl", "RControl"),
		("KeyOption", "Alt"),
		("RKeyOption", "ROption"),
		("KeyShift", "Shift"),
		("RKeyShift", "RShift"),
	] {
		if hotkey.get(key).and_then(Value::as_bool).unwrap_or(false) {
			modifiers.push(token);
		}
	}

	let codes = hotkey
		.get("VKeyCodes")
		.and_then(Value::as_array)
		.map(|codes| codes.iter().filter_map(Value::as_u64).filter(|code| *code <= u16::MAX as u64).collect::<Vec<_>>())
		.filter(|codes| !codes.is_empty())
		.unwrap_or_else(|| hotkey.get("VKeyCode").and_then(Value::as_u64).filter(|code| *code <= u16::MAX as u64).into_iter().collect());
	if codes.is_empty() {
		return json!({ "down": "", "up": "" });
	}

	let mut tokens = Vec::new();
	let mut display = Vec::new();
	for modifier in &modifiers {
		tokens.push(format!("k({modifier},Press)"));
		display.push(
			match *modifier {
				"Meta" => "⌘",
				"RCommand" => "Right ⌘",
				"Control" => "⌃",
				"RControl" => "Right ⌃",
				"Alt" => "⌥",
				"ROption" => "Right ⌥",
				"Shift" => "⇧",
				"RShift" => "Right ⇧",
				_ => "",
			}
			.to_owned(),
		);
	}
	for code in codes {
		tokens.push(format!("r({code})"));
		display.push(mac_key_label(code));
	}
	for modifier in modifiers.iter().rev() {
		tokens.push(format!("k({modifier},Release)"));
	}
	json!({ "down": format!("[{}]", tokens.join(",")), "up": "", "display": display.join(" + ") })
}

fn super_hotkey_settings(settings: &Value) -> Value {
	let primary = hotkey_settings(settings, 0);
	let down = primary.get("down").and_then(Value::as_str).unwrap_or("").to_owned();
	let up = hotkey_settings(settings, 1).get("down").and_then(Value::as_str).unwrap_or("").to_owned();
	json!({
		"down": if down == "[]" { String::new() } else { down },
		"up": if up == "[]" { String::new() } else { up },
		"display": primary.get("display").and_then(Value::as_str).unwrap_or(""),
	})
}

fn hotkey_switch_slots(settings: &Value, visual_states: usize) -> Vec<Value> {
	let source_count = settings.get("Hotkeys").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
	let minimum = visual_states.max(1);
	let mut hotkeys = (0..source_count.max(minimum)).map(|index| hotkey_settings(settings, index)).collect::<Vec<_>>();
	// VSD Craft can serialize an unused trailing Hotkeys record after the last
	// visible switch state. Importing it creates a spurious blank state.
	while hotkeys.len() > minimum && hotkeys.last().and_then(|slot| slot.get("down")).and_then(Value::as_str).is_some_and(str::is_empty) {
		hotkeys.pop();
	}
	hotkeys
}

fn image_data_url(base_dir: &Path, raw: &str) -> Option<String> {
	if raw.is_empty() {
		return None;
	}
	let raw_path = Path::new(raw);
	let candidates = [
		raw_path.to_owned(),
		base_dir.join(raw_path),
		base_dir.join("Images").join(raw_path),
		base_dir.join("Images").join(raw_path.file_name()?),
	];
	let path = candidates.iter().find(|path| path.is_file())?;
	let data = fs::read(path).ok()?;
	let mime = match path.extension().and_then(|extension| extension.to_str()).unwrap_or("png").to_ascii_lowercase().as_str() {
		"jpg" | "jpeg" => "image/jpeg",
		"gif" => "image/gif",
		"webp" => "image/webp",
		"svg" => "image/svg+xml",
		_ => "image/png",
	};
	Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(data)))
}

fn apply_vsd_states(mut states: Vec<ActionState>, source: &[VsdState], base_dir: &Path) -> Vec<ActionState> {
	if states.is_empty() {
		states.push(ActionState::default());
	}
	if source.len() > states.len() {
		let template = states.last().cloned().unwrap_or_default();
		states.resize(source.len(), template);
	}
	for (index, source_state) in source.iter().enumerate() {
		let state = &mut states[index];
		if let Some(image) = image_data_url(base_dir, &source_state.image) {
			state.image = image;
		}
		if !source_state.title.is_empty() {
			state.text = source_state.title.clone();
			state.show = true;
		}
	}
	states
}

fn action_instance(action: Action, context: ActionContext, settings: Value, source_states: &[VsdState], base_dir: &Path, current_state: u16) -> ActionInstance {
	let states = apply_vsd_states(action.states.clone(), source_states, base_dir);
	let current_state = (current_state as usize).min(states.len().saturating_sub(1)) as u16;
	ActionInstance {
		action,
		context,
		states,
		current_state,
		settings,
		children: None,
	}
}

fn composite_child(raw: &Value) -> Option<VsdAction> {
	let uuid = setting_string(raw, &["UUID", "ActionUUID", "uuid", "actionUuid"]).or_else(|| setting_string(raw, &["ActionID", "actionId"]))?;
	let name = setting_string(raw, &["Name", "name"]).unwrap_or_else(|| {
		crate::vsd_actions::definition(&uuid)
			.map(|definition| definition.name.clone())
			.unwrap_or_else(|| "Unknown VSD Craft action".to_owned())
	});
	let settings = raw.get("Settings").or_else(|| raw.get("settings")).cloned().unwrap_or_else(|| json!({}));
	let state = raw.get("State").or_else(|| raw.get("state")).and_then(Value::as_u64).unwrap_or(0).min(u16::MAX as u64) as u16;
	let states = raw
		.get("States")
		.or_else(|| raw.get("states"))
		.cloned()
		.and_then(|value| serde_json::from_value(value).ok())
		.unwrap_or_default();
	let multi_action_data = raw.get("MultiActionData").or_else(|| raw.get("multiActionData")).and_then(Value::as_array).cloned().unwrap_or_default();
	Some(VsdAction {
		controller: "Keypad".to_owned(),
		name,
		settings,
		state,
		states,
		uuid,
		multi_action_data,
	})
}

fn composite_action_from_categories(categories: &HashMap<String, Category>, uuid: &str) -> Action {
	let mut action = action_from_categories(categories, uuid, "Multi Action", "");
	// Composite actions are implemented in the application core and must never
	// require a plugin process to exist.
	action.plugin.clear();
	action
}

fn key_position(key: &str) -> Option<u8> {
	let (x, y) = key.split_once(',')?;
	let x: u8 = x.trim().parse().ok()?;
	let y: u8 = y.trim().parse().ok()?;
	if y > 2 || x > 5 {
		return None;
	}
	Some(if x == 5 { 15 + y } else { x + y * 5 })
}

fn profile_for_index(pages: &[PageSpec], index: usize) -> &str {
	pages.get(index).map(|page| page.profile_id.as_str()).unwrap_or("VSD Craft/Imported")
}

fn switch_target(uuid: &str, settings: &Value, page_index: usize, pages: &[PageSpec]) -> String {
	let uuid = uuid.to_ascii_lowercase();
	if uuid.contains("page.goto") {
		let requested = native_page_index(settings, page_index);
		return profile_for_index(pages, requested.min(pages.len().saturating_sub(1))).to_owned();
	}
	if uuid.contains("page.previous") {
		return profile_for_index(pages, if page_index == 0 { pages.len().saturating_sub(1) } else { page_index - 1 }).to_owned();
	}
	if uuid.contains("profile.backtoparent") {
		return "Default".to_owned();
	}
	if uuid.contains("profile.openchild") || uuid.contains("profile.rotate") {
		if let Some(profile) = setting_string(settings, &["ProfileUUID", "profile"])
			&& let Some(page) = pages.iter().find(|page| page.path.to_string_lossy().contains(&profile))
		{
			return page.profile_id.clone();
		}
	}
	profile_for_index(pages, if pages.is_empty() { 0 } else { (page_index + 1) % pages.len() }).to_owned()
}

fn folder_target(settings: &Value, pages: &[PageSpec]) -> Option<String> {
	let target = setting_string(settings, &["ProfileUUID", "profile", "target"])?;
	pages
		.iter()
		.find(|page| page.profile_id == target || page.path.to_string_lossy().contains(&target))
		.map(|page| page.profile_id.clone())
}

fn mapped_instance(
	categories: &HashMap<String, Category>,
	action: &VsdAction,
	context: ActionContext,
	page_index: usize,
	pages: &[PageSpec],
	base_dir: &Path,
	device_id: &str,
	unsupported: &mut Vec<String>,
) -> ActionInstance {
	mapped_instance_at_depth(categories, action, context, page_index, pages, base_dir, device_id, unsupported, 0)
}

fn mapped_instance_at_depth(
	categories: &HashMap<String, Category>,
	action: &VsdAction,
	context: ActionContext,
	page_index: usize,
	pages: &[PageSpec],
	base_dir: &Path,
	device_id: &str,
	unsupported: &mut Vec<String>,
	depth: usize,
) -> ActionInstance {
	if depth > 0 && crate::vsd_actions::is_composite_action(&action.uuid) {
		if !unsupported.contains(&action.uuid) {
			unsupported.push(action.uuid.clone());
		}
		let mut placeholder = fallback_action(crate::m18_actions::UNSUPPORTED_VSD_UUID, "Nested VSD Craft flow needs a nested editor", "");
		placeholder.tooltip = "Nested VSD flows are preserved but inactive until nested action editing and addressing are supported".to_owned();
		placeholder.visible_in_action_list = false;
		return action_instance(
			placeholder,
			context,
			json!({ "sourceName": action.name, "sourceUuid": action.uuid, "sourceSettings": action.settings, "sourceCompositeChildren": action.multi_action_data }),
			&action.states,
			base_dir,
			action.state,
		);
	}
	if depth >= 16 {
		if !unsupported.contains(&action.uuid) {
			unsupported.push(action.uuid.clone());
		}
		let mut placeholder = fallback_action(crate::m18_actions::UNSUPPORTED_VSD_UUID, "Unsupported nested VSD Craft action", "");
		placeholder.tooltip = "Nested action depth exceeds the import safety limit".to_owned();
		placeholder.visible_in_action_list = false;
		return action_instance(
			placeholder,
			context,
			json!({ "sourceName": action.name, "sourceUuid": action.uuid, "sourceSettings": action.settings }),
			&action.states,
			base_dir,
			action.state,
		);
	}
	let kind = mapping(&action.uuid, &action.name);
	match kind {
		Mapping::MultiAction | Mapping::ActionCarousel | Mapping::ActionCycle => {
			let (parent_uuid, parent_name) = match kind {
				Mapping::MultiAction => ("opendeck.multiaction", "Multi Action"),
				Mapping::ActionCarousel => ("opendeck.carouselaction", "Action Carousel"),
				Mapping::ActionCycle => ("opendeck.toggleaction", "Action Cycle"),
				_ => unreachable!(),
			};
			let mut mapped = composite_action_from_categories(categories, parent_uuid);
			mapped.name = parent_name.to_owned();
			let mut parent = action_instance(
				mapped,
				context.clone(),
				json!({ "sourceUuid": action.uuid, "sourceSettings": action.settings, "delays": [] }),
				&action.states,
				base_dir,
				action.state,
			);
			let mut children = Vec::with_capacity(action.multi_action_data.len());
			for (index, raw_child) in action.multi_action_data.iter().enumerate() {
				let Some(child_action) = composite_child(raw_child) else {
					let unknown_uuid = setting_string(raw_child, &["UUID", "ActionUUID", "uuid", "actionUuid", "ActionID", "actionId"]).unwrap_or_else(|| "unknown.vsd-child".to_owned());
					if !unsupported.contains(&unknown_uuid) {
						unsupported.push(unknown_uuid.clone());
					}
					let mut placeholder = fallback_action(crate::m18_actions::UNSUPPORTED_VSD_UUID, "Malformed VSD Craft child", "");
					placeholder.tooltip = "This composite item has no recognizable action UUID and will not execute".to_owned();
					placeholder.visible_in_action_list = false;
					let mut child = action_instance(
						placeholder,
						ActionContext {
							index: (index + 1).min(u16::MAX as usize) as u16,
							..context.clone()
						},
						json!({ "sourceUuid": unknown_uuid, "sourceItem": raw_child }),
						&[],
						base_dir,
						0,
					);
					child.context.index = (index + 1).min(u16::MAX as usize) as u16;
					children.push(child);
					continue;
				};
				let child_context = ActionContext {
					index: (index + 1).min(u16::MAX as usize) as u16,
					..context.clone()
				};
				let mut child = mapped_instance_at_depth(categories, &child_action, child_context, page_index, pages, base_dir, device_id, unsupported, depth + 1);
				if !child_action.name.trim().is_empty() {
					child.action.name = child_action.name.clone();
				}
				if let Some(delays) = raw_child.get("MultiActionDelays").or_else(|| raw_child.get("multiActionDelays")) {
					if !child.settings.is_object() {
						child.settings = json!({ "value": child.settings });
					}
					child.settings["_vsdMultiActionDelays"] = delays.clone();
				}
				if let Some(image) = raw_child
					.get("MultiActionImage")
					.or_else(|| raw_child.get("multiActionImage"))
					.and_then(Value::as_str)
					.and_then(|image| image_data_url(base_dir, image))
					&& !image.is_empty()
				{
					if let Some(first_state) = child.states.first_mut() {
						first_state.image = image;
					}
				}
				children.push(child);
			}
			if parent_uuid != "opendeck.multiaction" {
				let template = parent.states.first().cloned().unwrap_or_default();
				parent.states.resize(children.len().max(1), template);
				for (index, child) in children.iter().enumerate() {
					if let Some(child_state) = child.states.get(child.current_state as usize)
						&& !child_state.image.is_empty()
					{
						parent.states[index].image = child_state.image.clone();
					}
				}
				let selected_child = if matches!(kind, Mapping::ActionCycle) {
					numeric_setting(&action.settings, "Index").unwrap_or(action.state as usize)
				} else {
					action.state as usize
				};
				parent.current_state = selected_child.min(children.len().saturating_sub(1)) as u16;
			}
			parent.children = Some(children);
			parent
		}
		Mapping::NativeOpenApps => {
			let mapped = action_from_categories(categories, crate::m18_actions::OPEN_APPS_UUID, "OpenApps", "");
			let mut instance = action_instance(mapped, context, json!({ "appPath": application_target(&action.settings) }), &action.states, base_dir, action.state);
			crate::m18_actions::refresh_open_app_icon(&mut instance);
			instance
		}
		Mapping::NativeSuperHotkeys => {
			let mapped = action_from_categories(categories, crate::m18_actions::SUPER_HOTKEYS_UUID, "Super Hotkeys", "");
			action_instance(mapped, context, super_hotkey_settings(&action.settings), &action.states, base_dir, action.state)
		}
		Mapping::NativeHotkeySwitch => {
			let hotkeys = hotkey_switch_slots(&action.settings, action.states.len());
			let count = hotkeys.len();
			let mapped = action_from_categories(categories, crate::m18_actions::HOTKEY_SWITCH_UUID, "HotkeySwitch", "");
			let mut instance = action_instance(
				mapped,
				context,
				json!({
					"hotkeys": hotkeys,
					"index": numeric_setting(&action.settings, "Index").unwrap_or(0).min(count - 1),
				}),
				&action.states,
				base_dir,
				action.state,
			);
			let template = instance.states.first().cloned().unwrap_or_default();
			instance.states.truncate(count);
			instance.states.resize(count, template);
			instance.current_state = numeric_setting(&action.settings, "Index").unwrap_or(0).min(count - 1) as u16;
			instance
		}
		Mapping::NativeVolumeDown | Mapping::NativeVolumeUp | Mapping::NativeMute | Mapping::NativeSiri => {
			let (uuid, name) = match kind {
				Mapping::NativeVolumeDown => (crate::m18_actions::VOLUME_DOWN_UUID, "Volume down"),
				Mapping::NativeVolumeUp => (crate::m18_actions::VOLUME_UP_UUID, "Volume up"),
				Mapping::NativeMute => (crate::m18_actions::MUTE_UUID, "Mute"),
				Mapping::NativeSiri => (crate::m18_actions::SIRI_UUID, "Siri"),
				_ => unreachable!(),
			};
			let mapped = action_from_categories(categories, uuid, name, "");
			action_instance(mapped, context, json!({}), &action.states, base_dir, action.state)
		}
		Mapping::NativeSystemControl => {
			let (uuid, name) = native_system_control(&action.uuid);
			let mapped = action_from_categories(categories, uuid, name, "");
			action_instance(mapped, context, json!({}), &action.states, base_dir, action.state)
		}
		Mapping::NativePagePrevious | Mapping::NativePageNext | Mapping::NativePageGoto | Mapping::NativePageIndicator => {
			let (uuid, name) = match kind {
				Mapping::NativePagePrevious => (crate::m18_actions::PAGE_PREVIOUS_UUID, "Previous page"),
				Mapping::NativePageNext => (crate::m18_actions::PAGE_NEXT_UUID, "Next page"),
				Mapping::NativePageGoto => (crate::m18_actions::PAGE_GOTO_UUID, "Go to page"),
				Mapping::NativePageIndicator => (crate::m18_actions::PAGE_INDICATOR_UUID, "Page Indicator"),
				_ => unreachable!(),
			};
			let settings = if matches!(kind, Mapping::NativePageGoto) {
				let index = native_page_index(&action.settings, page_index);
				json!({
					"page": profile_for_index(pages, index),
					"pageIndex": index,
					"showPageNumber": show_page_number(&action.settings),
				})
			} else {
				json!({})
			};
			let mapped = action_from_categories(categories, uuid, name, "");
			action_instance(mapped, context, settings, &action.states, base_dir, action.state)
		}
		Mapping::RunCommand => {
			let command = if action.uuid.to_ascii_lowercase().contains("openapps") {
				app_command(&action.settings)
			} else if action.uuid.to_ascii_lowercase().contains(".open") {
				open_command(&action.settings).unwrap_or_else(|| "true".to_owned())
			} else {
				system_command(&action.uuid, &action.settings)
			};
			let mapped = action_from_categories(categories, "com.amansprojects.starterpack.runcommand", "Run Command", STARTER_PLUGIN);
			action_instance(mapped, context, json!({ "down": command }), &action.states, base_dir, action.state)
		}
		Mapping::OpenUrl => {
			let url = setting_string(&action.settings, &["url", "URL", "website", "link", "path"]).unwrap_or_default();
			let mapped = action_from_categories(categories, "com.amansprojects.starterpack.openurl", "Open URL", STARTER_PLUGIN);
			action_instance(mapped, context, json!({ "down": url }), &action.states, base_dir, action.state)
		}
		Mapping::SwitchProfile => {
			let target = switch_target(&action.uuid, &action.settings, page_index, pages);
			let mapped = action_from_categories(categories, "com.amansprojects.starterpack.switchprofile", "Switch Profile", STARTER_PLUGIN);
			action_instance(mapped, context, json!({ "device": device_id, "profile": target }), &action.states, base_dir, action.state)
		}
		Mapping::DeviceBrightness => {
			let mapped = action_from_categories(categories, "com.amansprojects.starterpack.devicebrightness", "Device Brightness", STARTER_PLUGIN);
			let action_index = action.settings.get("actionIdx").and_then(Value::as_i64).unwrap_or(0);
			let (brightness_action, value) = match action_index {
				1 => ("decrease", 6),
				2 => ("increase", 6),
				_ => ("set", action.settings.get("value").and_then(Value::as_i64).unwrap_or(50).clamp(0, 100)),
			};
			action_instance(mapped, context, json!({ "action": brightness_action, "value": value }), &action.states, base_dir, action.state)
		}
		Mapping::NativeVsdAction => {
			let Some(definition) = crate::vsd_actions::definition(&action.uuid) else {
				unreachable!("NativeVsdAction is selected only for catalogue entries")
			};
			let mapped = action_from_categories(categories, &definition.uuid, &definition.name, "");
			let settings = if action.uuid.eq_ignore_ascii_case("com.hotspot.streamdock.profile.openchild") {
				json!({ "profile": folder_target(&action.settings, pages).unwrap_or_default() })
			} else if action.uuid.eq_ignore_ascii_case("com.hotspot.streamdock.profile.backtoparent") {
				json!({})
			} else if crate::vsd_actions::is_hotkey(&action.uuid) {
				hotkey_settings(&action.settings, 0)
			} else {
				action.settings.clone()
			};
			action_instance(mapped, context, settings, &action.states, base_dir, action.state)
		}
		Mapping::Unsupported => {
			if !unsupported.contains(&action.uuid) {
				unsupported.push(action.uuid.clone());
			}
			let mut mapped = fallback_action(crate::m18_actions::UNSUPPORTED_VSD_UUID, "Unsupported VSD Craft action", "");
			mapped.tooltip = "Preserved from VSD Craft; behavior is not implemented and this key will not execute".to_owned();
			mapped.visible_in_action_list = false;
			action_instance(
				mapped,
				context,
				json!({
					"sourceName": action.name,
					"sourceUuid": action.uuid,
					"sourceSettings": action.settings,
				}),
				&action.states,
				base_dir,
				action.state,
			)
		}
	}
}

fn profile_from_page(categories: &HashMap<String, Category>, page: &PageSpec, page_index: usize, pages: &[PageSpec], device_id: &str, unsupported: &mut Vec<String>) -> (Profile, usize) {
	let base_dir = page.path.parent().unwrap_or_else(|| Path::new("."));
	let mut keys = vec![None; 20];
	let mut imported_actions = 0;
	for (key, action) in &page.manifest.actions {
		if action.controller.to_ascii_lowercase() != "keypad" {
			continue;
		}
		let Some(position) = key_position(key) else { continue };
		let context = ActionContext {
			device: device_id.to_owned(),
			profile: page.profile_id.clone(),
			controller: "Keypad".to_owned(),
			position,
			index: 0,
		};
		keys[position as usize] = Some(mapped_instance(categories, action, context, page_index, pages, base_dir, device_id, unsupported));
		imported_actions += 1;
	}
	(
		Profile {
			id: page.profile_id.clone(),
			keys,
			sliders: vec![],
			infobars: vec![],
			stale: false,
		},
		imported_actions,
	)
}

#[command]
pub async fn import_vsd_profile(app: AppHandle) -> Result<ImportSummary, Error> {
	let Some(FilePath::Path(selected)) = app.dialog().file().add_filter("VSD Craft profile", &["json"]).blocking_pick_file() else {
		return Err(anyhow::anyhow!("No VSD Craft profile was selected").into());
	};

	let selected = manifest_path(selected);
	if !selected.is_file() {
		return Err(anyhow::anyhow!("The selected path does not contain manifest.json").into());
	}
	let root = read_manifest(&selected)?;
	if !root.device_uuid.is_empty() && root.device_uuid != "VSDM18" {
		log::warn!("Importing a VSD profile whose device UUID is {}", root.device_uuid);
	}
	let pages = collect_pages(&selected, &root);
	if pages.is_empty() {
		return Err(anyhow::anyhow!("The selected VSD Craft profile has no importable pages").into());
	}
	let serial = pages
		.iter()
		.find_map(|page| (!page.manifest.device_serial_number.is_empty()).then(|| page.manifest.device_serial_number.clone()))
		.unwrap_or_else(|| "VSDM18".to_owned());
	let device_id = format!("{M18_DEVICE_NAMESPACE}-{serial}");

	let categories = CATEGORIES.read().await.clone();
	let mut unsupported = vec![];
	let mut imported_profiles = vec![];
	let mut imported_actions = 0;
	let mut profiles = vec![];
	for (page_index, page) in pages.iter().enumerate() {
		let (profile, count) = profile_from_page(&categories, page, page_index, &pages, &device_id, &mut unsupported);
		imported_actions += count;
		imported_profiles.push(page.profile_id.clone());
		profiles.push(profile);
	}

	let device = DeviceInfo {
		id: device_id.clone(),
		plugin: String::new(),
		name: "VSD Inside M18".to_owned(),
		rows: 4,
		columns: 5,
		encoders: 0,
		touchpoints: 0,
		infobars: 0,
		r#type: 0,
	};
	// `save_profiles` only needs the profile store and device store; the device
	// can be disconnected while importing, so it is deliberately not inserted
	// into the live device registry here.
	let selected_profile = profiles.first().map(|profile| profile.id.clone());
	let mut locks = acquire_locks_mut().await;
	for profile in profiles {
		let store = locks.profile_stores.get_profile_store_mut(&device, &profile.id).await?;
		store.value = profile;
		store.save()?;
	}
	if let Some(selected_profile) = selected_profile {
		locks.device_stores.set_selected_profile(&device.id, selected_profile)?;
	}
	crate::m18_pages::replace(
		&device_id,
		pages
			.iter()
			.enumerate()
			.map(|(index, page)| crate::m18_pages::M18Page {
				id: page.profile_id.clone(),
				name: (index + 1).to_string(),
				profile: page.profile_id.clone(),
			})
			.collect(),
	)?;

	Ok(ImportSummary {
		device: device_id,
		profiles: imported_profiles,
		imported_actions,
		unsupported_actions: unsupported,
		source: selected.to_string_lossy().into_owned(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn maps_vsd_coordinates_to_all_eighteen_m18_buttons() {
		for y in 0..3 {
			for x in 0..5 {
				assert_eq!(key_position(&format!("{x},{y}")), Some(x + y * 5));
			}
			assert_eq!(key_position(&format!("5,{y}")), Some(15 + y));
		}
		assert_eq!(key_position("6,0"), None);
	}

	#[test]
	fn imports_a_plain_vsd_hotkey_as_a_native_action_without_an_action_plugin() {
		let settings = json!({
			"Hotkeys": [{ "VKeyCode": 79, "VKeyCodes": [79], "KeyCmd": false }]
		});
		assert_eq!(hotkey_settings(&settings, 0)["down"], "[r(79)]");
		assert_eq!(hotkey_settings(&settings, 0)["display"], "F18");
		assert_eq!(ron::from_str::<Vec<enigo::agent::Token>>("[r(79)]").unwrap().len(), 1);

		let mut categories = HashMap::new();
		crate::vsd_actions::insert_catalog(&mut categories);
		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Hotkey".to_owned(),
			settings,
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.system.hotkey".to_owned(),
			multi_action_data: vec![],
		};
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Profile A".to_owned(),
			controller: "Keypad".to_owned(),
			position: 0,
			index: 0,
		};
		let instance = mapped_instance(&categories, &action, context, 0, &[], Path::new("."), "18-test", &mut vec![]);
		assert_eq!(instance.action.uuid, action.uuid);
		assert!(instance.action.plugin.is_empty());
		assert_eq!(instance.settings["down"], "[r(79)]");
	}

	#[test]
	fn preserves_f14_f15_and_modified_chords_from_vsd_profile() {
		let settings = json!({ "Hotkeys": [
			{ "VKeyCode": 107, "VKeyCodes": [107], "KeyModifiers": 65536 },
			{ "VKeyCode": 113, "VKeyCodes": [113], "KeyModifiers": 65536 },
			{ "VKeyCode": 79, "VKeyCodes": [79], "KeyCmd": true, "KeyShift": true }
		] });
		assert_eq!(hotkey_settings(&settings, 0)["down"], "[r(107)]");
		assert_eq!(hotkey_settings(&settings, 0)["display"], "F14");
		assert_eq!(hotkey_settings(&settings, 1)["down"], "[r(113)]");
		assert_eq!(hotkey_settings(&settings, 1)["display"], "F15");
		let chord = hotkey_settings(&settings, 2);
		assert_eq!(chord["down"], "[k(Meta,Press),k(Shift,Press),r(79),k(Shift,Release),k(Meta,Release)]");
		assert_eq!(chord["display"], "⌘ + ⇧ + F18");
		assert_eq!(ron::from_str::<Vec<enigo::agent::Token>>(chord["down"].as_str().unwrap()).unwrap().len(), 5);
	}

	#[test]
	fn ignores_unused_trailing_hotkey_switch_record() {
		let settings = json!({ "Hotkeys": [
			{ "VKeyCodes": [107] },
			{ "VKeyCodes": [113] },
			{ "VKeyCode": -1, "VKeyCodes": [] }
		] });
		let slots = hotkey_switch_slots(&settings, 2);
		assert_eq!(slots.len(), 2);
		assert_eq!(slots[0]["down"], "[r(107)]");
		assert_eq!(slots[1]["down"], "[r(113)]");
		assert_eq!(hotkey_switch_slots(&settings, 3).len(), 3);
	}

	#[test]
	fn maps_current_m18_vsd_action_families() {
		assert!(matches!(mapping("com.hotspot.streamdock.system.hotkeySwitch", "HotkeySwitch"), Mapping::NativeHotkeySwitch));
		assert!(matches!(mapping("com.hotspot.streamdock.device.brightness", "Brightness"), Mapping::NativeVsdAction));
		assert!(matches!(mapping("com.hotspot.streamdock.quickcontrol.microphone", "Microphone"), Mapping::NativeVsdAction));
		assert!(matches!(mapping("com.hotspot.streamdock.system.openApps", "OpenApps"), Mapping::NativeOpenApps));
		assert!(matches!(mapping("com.hotspot.streamdock.page.next", "Next page"), Mapping::NativePageNext));
		assert!(matches!(mapping("com.hotspot.streamdock.page.indicator", "Page Indicator"), Mapping::NativePageIndicator));
		assert!(matches!(mapping("com.hotspot.streamdock.touchbar.siri", "Siri"), Mapping::NativeSiri));
		assert!(matches!(mapping("com.hotspot.streamdock.touchbar.dispatchcenter", "Dispatch Center"), Mapping::NativeSystemControl));
		assert!(matches!(mapping("com.hotspot.streamdock.touchbar.screenshot", "Screenshot"), Mapping::NativeSystemControl));
		assert!(matches!(
			mapping("com.hotspot.streamdock.touchbar.increasescreenbrightness", "Increase screen brightness"),
			Mapping::NativeSystemControl
		));
		assert!(matches!(mapping("com.hotspot.streamdock.touchbar.playpause", "Play/Pause"), Mapping::NativeSystemControl));
	}

	#[test]
	fn imports_vsd_page_targets_as_one_based_numbers_and_preserves_number_visibility() {
		assert_eq!(native_page_index(&json!({ "PageIndex": 1 }), 1), 0);
		assert_eq!(native_page_index(&json!({ "PageIndex": 2 }), 0), 1);
		assert_eq!(native_page_index(&json!({}), 1), 1);
		assert!(show_page_number(&json!({})));
		assert!(!show_page_number(&json!({ "ShowPageNumber": false })));
		assert!(!show_page_number(&json!({ "show_page_number": false })));

		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Go to Page".to_owned(),
			settings: json!({ "PageIndex": 1 }),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.page.goto".to_owned(),
			multi_action_data: vec![],
		};
		let pages = vec![
			PageSpec {
				path: PathBuf::from("page-1"),
				manifest: VsdManifest {
					actions: HashMap::new(),
					device_serial_number: String::new(),
					device_uuid: String::new(),
					name: "Page 1".to_owned(),
					pages: Value::Null,
				},
				profile_id: "Page 1".to_owned(),
			},
			PageSpec {
				path: PathBuf::from("page-2"),
				manifest: VsdManifest {
					actions: HashMap::new(),
					device_serial_number: String::new(),
					device_uuid: String::new(),
					name: "Page 2".to_owned(),
					pages: Value::Null,
				},
				profile_id: "Page 2".to_owned(),
			},
		];
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Page 1".to_owned(),
			controller: "Keypad".to_owned(),
			position: 15,
			index: 0,
		};
		let mut unsupported = vec![];
		let instance = mapped_instance(&HashMap::new(), &action, context, 0, &pages, Path::new("."), "18-test", &mut unsupported);

		assert_eq!(instance.action.uuid, crate::m18_actions::PAGE_GOTO_UUID);
		assert_eq!(instance.settings["pageIndex"], 0);
		assert_eq!(instance.settings["page"], "Page 1");
		assert_eq!(instance.settings["showPageNumber"], true);
		assert!(unsupported.is_empty());
	}

	#[test]
	fn imports_page_indicator_as_a_native_m18_action() {
		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Page Indicator".to_owned(),
			settings: json!({}),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.page.indicator".to_owned(),
			multi_action_data: vec![],
		};
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Profile 2".to_owned(),
			controller: "Keypad".to_owned(),
			position: 0,
			index: 0,
		};
		let mut unsupported = vec![];
		let instance = mapped_instance(&HashMap::new(), &action, context, 1, &[], Path::new("."), "18-test", &mut unsupported);

		assert_eq!(instance.action.uuid, crate::m18_actions::PAGE_INDICATOR_UUID);
		assert_eq!(instance.action.name, "Page Indicator");
		assert!(crate::m18_actions::is_native_action(&instance.action.uuid));
		assert!(unsupported.is_empty());
	}

	#[test]
	fn imports_vsd_composite_actions_as_native_children_and_preserves_child_delays() {
		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Multi Action".to_owned(),
			settings: json!({ "layout": "sequence" }),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.multiactions.routine".to_owned(),
			multi_action_data: vec![
				json!({
					"ActionID": "app-child-instance-1",
					"UUID": "com.hotspot.streamdock.system.hotkey",
					"Name": "Hotkey",
					"Settings": { "Hotkeys": [{ "VKeyCodes": [79] }] },
					"MultiActionDelays": { "Delay1": 25, "Delay2": "40" }
				}),
				json!({
					"ActionID": "delay-child",
					"UUID": "com.hotspot.streamdock.multiactions.delay",
					"Name": "Delay",
					"Settings": { "delay": "750" }
				}),
				json!({
					"ActionID": "nested-flow",
					"UUID": "com.hotspot.streamdock.multiactions.routine",
					"Name": "Nested flow",
					"Settings": {},
					"MultiActionData": [{ "ActionID": "inner", "UUID": "com.hotspot.streamdock.system.hotkey", "Settings": {} }]
				}),
			],
		};
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "test".to_owned(),
			controller: "Keypad".to_owned(),
			position: 0,
			index: 0,
		};
		let mut unsupported = vec![];
		let instance = mapped_instance(&HashMap::new(), &action, context, 0, &[], Path::new("."), "18-test", &mut unsupported);

		assert_eq!(instance.action.uuid, "opendeck.multiaction");
		assert!(instance.action.plugin.is_empty());
		assert_eq!(instance.settings["sourceUuid"], action.uuid);
		let children = instance.children.as_ref().unwrap();
		assert_eq!(children.len(), 3);
		assert_eq!(children[0].action.uuid, "com.hotspot.streamdock.system.hotkey");
		assert_eq!(children[0].settings["down"], "[r(79)]");
		assert_eq!(children[0].settings["_vsdMultiActionDelays"]["Delay1"], 25);
		assert_eq!(children[0].settings["_vsdMultiActionDelays"]["Delay2"], "40");
		assert_eq!(children[1].action.uuid, "com.hotspot.streamdock.multiactions.delay");
		assert_eq!(children[0].context.index, 1);
		assert_eq!(children[1].context.index, 2);
		assert_eq!(children[2].action.uuid, crate::m18_actions::UNSUPPORTED_VSD_UUID);
		assert_eq!(children[2].settings["sourceCompositeChildren"].as_array().unwrap().len(), 1);
		assert_eq!(children[2].context.index, 3);
		assert_eq!(unsupported, vec!["com.hotspot.streamdock.multiactions.routine"]);
	}

	#[test]
	fn deserializes_the_vendor_multi_action_parent_array() {
		let parsed: VsdAction = serde_json::from_value(json!({
			"Controller": "Keypad",
			"Name": "Action Cycle",
			"UUID": "com.hotspot.streamdock.multiactions.toggle",
			"Settings": { "Index": 1 },
			"MultiActionData": [{ "ActionID": "id-1", "UUID": "com.hotspot.streamdock.system.hotkey", "Settings": {} }]
		}))
		.unwrap();
		assert_eq!(parsed.multi_action_data.len(), 1);
		assert_eq!(parsed.multi_action_data[0]["UUID"], "com.hotspot.streamdock.system.hotkey");
		assert_eq!(numeric_setting(&parsed.settings, "Index"), Some(1));
		let nested = composite_child(&json!({
			"UUID": "com.hotspot.streamdock.multiactions.routine",
			"MultiActionData": [{ "UUID": "com.hotspot.streamdock.system.hotkey" }]
		}))
		.unwrap();
		assert_eq!(nested.multi_action_data.len(), 1);
	}

	#[test]
	fn imports_carousel_and_cycle_as_distinct_native_parents_and_respects_cycle_index() {
		for (source_uuid, expected_uuid, index) in [
			("com.hotspot.streamdock.multiactions.LunBo", "opendeck.carouselaction", 0),
			("com.hotspot.streamdock.multiactions.toggle", "opendeck.toggleaction", 1),
		] {
			let action = VsdAction {
				controller: "Keypad".to_owned(),
				name: "Flow".to_owned(),
				settings: json!({ "Index": index }),
				state: 0,
				states: vec![],
				uuid: source_uuid.to_owned(),
				multi_action_data: vec![
					json!({ "ActionID": "one", "UUID": "com.hotspot.streamdock.system.hotkey", "Name": "First", "Settings": { "Hotkeys": [{ "VKeyCodes": [79] }] } }),
					json!({ "ActionID": "two", "UUID": "com.hotspot.streamdock.system.hotkey", "Name": "Second", "Settings": { "Hotkeys": [{ "VKeyCodes": [107] }] } }),
				],
			};
			let context = ActionContext {
				device: "18-test".to_owned(),
				profile: "test".to_owned(),
				controller: "Keypad".to_owned(),
				position: 0,
				index: 0,
			};
			let instance = mapped_instance(&HashMap::new(), &action, context, 0, &[], Path::new("."), "18-test", &mut vec![]);
			assert_eq!(instance.action.uuid, expected_uuid);
			assert_eq!(instance.current_state, index);
			assert_eq!(instance.children.as_ref().unwrap()[0].action.name, "First");
			assert_eq!(instance.children.as_ref().unwrap()[1].action.name, "Second");
		}
	}

	#[test]
	fn imports_catalogued_single_actions_as_native_core_actions_with_settings() {
		let mut categories = HashMap::new();
		crate::vsd_actions::insert_catalog(&mut categories);
		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "UDP".to_owned(),
			settings: json!({ "Host": "192.0.2.4", "Port": 9000, "Message": "toggle" }),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.network.udp".to_owned(),
			multi_action_data: vec![],
		};
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Profile A".to_owned(),
			controller: "Keypad".to_owned(),
			position: 17,
			index: 0,
		};
		let instance = mapped_instance(&categories, &action, context, 0, &[], Path::new("/tmp"), "device", &mut vec![]);

		assert_eq!(instance.action.uuid, action.uuid);
		assert!(instance.action.plugin.is_empty());
		assert_eq!(instance.settings, action.settings);
		assert!(crate::m18_actions::is_native_action(&instance.action.uuid));

		let brightness = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Brightness".to_owned(),
			settings: json!({ "actionIdx": 1 }),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.device.brightness".to_owned(),
			multi_action_data: vec![],
		};
		let brightness_context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Profile A".to_owned(),
			controller: "Keypad".to_owned(),
			position: 0,
			index: 0,
		};
		let instance = mapped_instance(&categories, &brightness, brightness_context, 0, &[], Path::new("/tmp"), "device", &mut vec![]);
		assert_eq!(instance.action.uuid, brightness.uuid);
		assert!(instance.action.plugin.is_empty());
		assert_eq!(instance.settings, brightness.settings);
	}

	#[test]
	fn preserves_vsd_website_path_and_maps_common_system_controls() {
		let website = json!({ "path": "https://example.com" });
		assert_eq!(open_command(&website).as_deref(), Some("open 'https://example.com'"));
		assert!(matches!(mapping("com.hotspot.streamdock.profile.rotate", "Scene Shift"), Mapping::NativeVsdAction));
		assert!(matches!(mapping("com.hotspot.streamdock.profile.openchild", "Create Folder"), Mapping::NativeVsdAction));
		assert!(matches!(mapping("com.hotspot.streamdock.profile.backtoparent", "Go back"), Mapping::NativeVsdAction));
		assert!(matches!(mapping("com.hotspot.streamdock.system.multimedia", "Multimedia"), Mapping::NativeVsdAction));
	}

	#[test]
	fn imports_folder_targets_as_native_m18_page_references() {
		let page = PageSpec {
			path: PathBuf::from("/fixtures/child/manifest.json"),
			manifest: serde_json::from_value(json!({})).unwrap(),
			profile_id: "VSD Craft/Child".to_owned(),
		};
		let pages = vec![page];
		assert_eq!(folder_target(&json!({ "ProfileUUID": "/fixtures/child" }), &pages).as_deref(), Some("VSD Craft/Child"));

		let mut categories = HashMap::new();
		crate::vsd_actions::insert_catalog(&mut categories);
		let action = VsdAction {
			controller: "Keypad".to_owned(),
			name: "Create Folder".to_owned(),
			settings: json!({ "ProfileUUID": "/fixtures/child" }),
			state: 0,
			states: vec![],
			uuid: "com.hotspot.streamdock.profile.openchild".to_owned(),
			multi_action_data: vec![],
		};
		let context = ActionContext {
			device: "18-test".to_owned(),
			profile: "Profile A".to_owned(),
			controller: "Keypad".to_owned(),
			position: 0,
			index: 0,
		};
		let instance = mapped_instance(&categories, &action, context, 0, &pages, Path::new("/fixtures"), "18-test", &mut vec![]);
		assert_eq!(instance.action.uuid, action.uuid);
		assert!(instance.action.plugin.is_empty());
		assert_eq!(instance.settings["profile"], "VSD Craft/Child");
	}
}
