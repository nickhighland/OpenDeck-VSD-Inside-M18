//! M18-native catalog and runtime for VSD Craft actions.
//!
//! This is deliberately application-core functionality: VSD/M18 actions do
//! not spawn or depend on action-plugin processes. The catalog metadata is
//! captured from VSD Craft's installed action manifests; device-specific
//! encoder/K1 Pro entries are excluded.

use enigo::agent::Token;
use serde::Deserialize;
use serde_json::{Value, json};
use std::process::Command;
use std::sync::LazyLock;
use tauri::Emitter;

#[derive(Clone, Debug, Deserialize)]
pub struct VsdActionDefinition {
	pub name: String,
	pub uuid: String,
	// Vendor metadata, kept with the inventory; the library has its own names.
	#[allow(dead_code)]
	pub tooltip: String,
	#[allow(dead_code)]
	pub category: String,
	#[serde(rename = "stateCount")]
	pub state_count: usize,
	#[serde(rename = "supportedInMultiActions")]
	pub supported_in_multi_actions: bool,
}

static DEFINITIONS: LazyLock<Vec<VsdActionDefinition>> = LazyLock::new(|| serde_json::from_str(include_str!("vsd_action_catalog.json")).expect("bundled VSD action catalog must be valid JSON"));

#[cfg_attr(not(test), allow(dead_code))]
pub fn definitions() -> &'static [VsdActionDefinition] {
	&DEFINITIONS
}

pub fn definition(uuid: &str) -> Option<&'static VsdActionDefinition> {
	DEFINITIONS.iter().find(|definition| definition.uuid.eq_ignore_ascii_case(uuid))
}

pub fn is_vsd_action(uuid: &str) -> bool {
	definition(uuid).is_some()
}

pub fn is_composite_action(uuid: &str) -> bool {
	matches!(
		uuid.to_ascii_lowercase().as_str(),
		"com.hotspot.streamdock.multiactions.routine" | "com.hotspot.streamdock.multiactions.lunbo" | "com.hotspot.streamdock.multiactions.toggle"
	)
}

pub fn is_hotkey_switch(uuid: &str) -> bool {
	uuid.eq_ignore_ascii_case("com.hotspot.streamdock.system.hotkeySwitch")
}

pub fn is_hotkey(uuid: &str) -> bool {
	uuid.eq_ignore_ascii_case("com.hotspot.streamdock.system.hotkey")
}

pub fn default_settings(uuid: &str) -> Value {
	match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.device.brightness" => json!({ "actionIdx": 0 }),
		"com.hotspot.streamdock.multiactions.delay" => json!({ "delay": 500 }),
		"com.hotspot.streamdock.system.multimedia" => json!({ "actionIdx": 1 }),
		"com.hotspot.streamdock.mouse.event" => json!({ "eventType": "click", "button": "left", "x": 0, "y": 0, "axis": "vertical", "amount": 3, "coordinate": "absolute", "modifiers": [] }),
		"com.hotspot.streamdock.system.openapps" => json!({ "appPath": "" }),
		"com.hotspot.streamdock.system.open" | "com.hotspot.streamdock.system.website" => json!({ "path": "" }),
		"com.hotspot.streamdock.system.close" => json!({ "appPath": "" }),
		"com.hotspot.streamdock.system.hotkey" | "com.hotspot.streamdock.system.super.hotkey" => json!({ "down": "", "up": "", "display": "" }),
		"com.hotspot.streamdock.system.hotkeyswitch" => json!({ "hotkeys": [{ "down": "", "up": "" }, { "down": "", "up": "" }], "index": 0 }),
		"com.hotspot.streamdock.system.text" | "com.hotspot.streamdock.plain.text" => json!({ "text": "" }),
		"com.hotspot.streamdock.system.password" => json!({ "password": "" }),
		"com.hotspot.streamdock.network.udp" => json!({ "host": "127.0.0.1", "port": 5000, "message": "" }),
		"com.hotspot.streamdock.soundboard.playaudio" => json!({ "path": "", "mode": "Play/Stop", "volume": 100, "outputDevice": "default", "fadeType": "none", "fadeDuration": 0 }),
		"com.hotspot.streamdock.youtube.chatmessage" => json!({ "videoId": "", "message": "" }),
		"com.hotspot.streamdock.youtube.viewers" => json!({ "videoId": "" }),
		"com.hotspot.streamdock.weather.action1" => json!({ "inputCity": "", "searchList": [], "cityId": "", "tempList": "0", "title": "", "radio": "0", "radio2": "0", "theme": "Modern" }),
		"com.hotspot.streamdock.memo.action1" | "com.hotspot.streamdock.memo.action2" => json!({ "title": "", "content": "", "time": "", "color": "#ffffff" }),
		"com.mirabox.streamdock.time.action1" => json!({ "amplify": true, "zone": "system", "theme": "theme1", "isBackgroundHidden": false }),
		"com.mirabox.streamdock.time.action2" => json!({ "select": "1", "deep": 0 }),
		"com.mirabox.streamdock.time.action3" => json!({ "select": "1", "surplus": 60000, "timing": "60000", "inputTime": "0", "musicUrl": "", "color": "rgb(0,255,0)" }),
		"com.mirabox.streamdock.datetime.action1" => json!({ "format": "%Y-%m-%d %H:%M" }),
		"com.mirabox.streamdock.calendar.action1" => json!({ "daysAhead": 0 }),
		_ => Value::Object(serde_json::Map::new()),
	}
}

fn text_setting<'a>(settings: &'a Value, keys: &[&str]) -> Option<&'a str> {
	keys.iter().find_map(|key| settings.get(*key).and_then(Value::as_str).filter(|value| !value.trim().is_empty()))
}

fn number_setting(settings: &Value, keys: &[&str]) -> Option<i64> {
	keys.iter().find_map(|key| settings.get(*key).and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok())))
}

fn brightness_adjustment(action_index: i64) -> Option<i8> {
	match action_index {
		0 => Some(6),
		1 => Some(-6),
		_ => None,
	}
}

fn multimedia_input(action_index: i64) -> Option<&'static str> {
	match action_index {
		0 => Some("[k(MediaPrevTrack)]"),
		1 => Some("[k(MediaPlayPause)]"),
		2 => Some("[k(MediaNextTrack)]"),
		4 => Some("[k(VolumeMute)]"),
		5 => Some("[k(VolumeUp)]"),
		6 => Some("[k(VolumeDown)]"),
		_ => None,
	}
}

fn mouse_event_input(settings: &Value) -> Option<String> {
	let event = text_setting(settings, &["eventType", "event", "type", "operation"])
		.unwrap_or("click")
		.to_ascii_lowercase()
		.replace([' ', '_', '-'], "");
	let button = match text_setting(settings, &["button", "mouseButton"]).unwrap_or("left").to_ascii_lowercase().as_str() {
		"right" | "mouserightbutton" => "Right",
		"middle" | "mousmiddlebutton" => "Middle",
		"side1" | "back" | "mousebutton1" => "Back",
		"side2" | "forward" | "mousebutton2" => "Forward",
		_ => "Left",
	};
	let x = number_setting(settings, &["x", "X", "xAxis", "XAxis"]).unwrap_or(0).clamp(-32768, 32767);
	let y = number_setting(settings, &["y", "Y", "yAxis", "YAxis"]).unwrap_or(0).clamp(-32768, 32767);
	let axis = if text_setting(settings, &["axis", "scrollAxis", "wheelType"]).is_some_and(|value| value.to_ascii_lowercase().starts_with('h')) {
		"Horizontal"
	} else {
		"Vertical"
	};
	let coordinate = if text_setting(settings, &["coordinate", "coordinateMode"]).is_some_and(|value| value.to_ascii_lowercase().starts_with('r')) {
		"Rel"
	} else {
		"Abs"
	};
	let amount = number_setting(settings, &["amount", "scrollAmount", "wheelAmount"]).unwrap_or(3).clamp(-1000, 1000);
	let mut modifiers = Vec::new();
	for (keys, name) in [
		(&["KeyCmd", "KeyCommand", "command"][..], "Meta"),
		(&["KeyCtrl", "KeyControl", "control"][..], "Control"),
		(&["KeyOption", "KeyAlt", "alt"][..], "Alt"),
		(&["KeyShift", "shift"][..], "Shift"),
	] {
		if keys.iter().any(|key| settings.get(*key).and_then(Value::as_bool).unwrap_or(false)) {
			modifiers.push(name);
		}
	}
	if let Some(extra) = settings.get("modifiers").and_then(Value::as_array) {
		for modifier in extra.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase) {
			let token = match modifier.as_str() {
				"cmd" | "command" | "meta" | "win" => Some("Meta"),
				"ctrl" | "control" => Some("Control"),
				"alt" | "option" => Some("Alt"),
				"shift" => Some("Shift"),
				_ => None,
			};
			if let Some(token) = token
				&& !modifiers.contains(&token)
			{
				modifiers.push(token);
			}
		}
	}

	let mut tokens = modifiers.iter().map(|modifier| format!("k({modifier},Press)")).collect::<Vec<_>>();
	match event.as_str() {
		"doubleclick" | "mousedoubleclick" => {
			for _ in 0..2 {
				tokens.push(format!("b({button},Press)"));
				tokens.push(format!("b({button},Release)"));
			}
		}
		"move" | "mousemove" => tokens.push(format!("m({x},{y},{coordinate})")),
		"scroll" | "wheelscroll" | "mousewheelscroll" => tokens.push(format!("s({amount},{axis})")),
		"drag" | "draganddrop" | "mousedrag" => {
			tokens.push(format!("b({button},Press)"));
			tokens.push(format!("m({x},{y},{coordinate})"));
			tokens.push(format!("b({button},Release)"));
		}
		_ => tokens.push(format!("b({button},Click)")),
	}
	tokens.extend(modifiers.iter().rev().map(|modifier| format!("k({modifier},Release)")));
	let ron = format!("[{}]", tokens.join(","));
	// Fail closed if malformed saved settings somehow produced invalid input.
	let parsed: Vec<Token> = ron::from_str(&ron).ok()?;
	Some(ron::to_string(&parsed).ok()?)
}

async fn run(program: &str, args: Vec<String>) -> Result<(), anyhow::Error> {
	let program = program.to_owned();
	tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
		let status = Command::new(&program).args(args).status()?;
		if status.success() { Ok(()) } else { Err(anyhow::anyhow!("{program} exited with {status}")) }
	})
	.await??;
	Ok(())
}

async fn open_target(target: &str) -> Result<(), anyhow::Error> {
	if target.trim().is_empty() {
		return Ok(());
	}
	#[cfg(target_os = "macos")]
	{
		run("/usr/bin/open", vec![target.to_owned()]).await
	}
	#[cfg(target_os = "linux")]
	{
		run("xdg-open", vec![target.to_owned()]).await
	}
	#[cfg(target_os = "windows")]
	{
		run("cmd", vec!["/C".to_owned(), "start".to_owned(), "".to_owned(), target.to_owned()]).await
	}
}

/// Open an application given as a path, a bundle identifier, or a display
/// name. `open Calculator` treats a bare name as a file path and fails, so
/// names must go through `open -a`.
async fn open_application(target: &str) -> Result<(), anyhow::Error> {
	let target = target.trim();
	if target.is_empty() {
		return Ok(());
	}
	#[cfg(target_os = "macos")]
	{
		if target.contains('/') || target.contains("://") {
			run("/usr/bin/open", vec![target.to_owned()]).await
		} else if looks_like_bundle_identifier(target) {
			run("/usr/bin/open", vec!["-b".to_owned(), target.to_owned()]).await
		} else {
			run("/usr/bin/open", vec!["-a".to_owned(), target.trim_end_matches(".app").to_owned()]).await
		}
	}
	#[cfg(not(target_os = "macos"))]
	{
		open_target(target).await
	}
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn looks_like_bundle_identifier(value: &str) -> bool {
	let segments = value.split('.').collect::<Vec<_>>();
	segments.len() >= 3
		&& !value.to_ascii_lowercase().ends_with(".app")
		&& segments
			.iter()
			.all(|segment| !segment.is_empty() && segment.chars().all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_'))
}

fn configured_input(instance: &crate::shared::ActionInstance) -> Option<String> {
	if let Some(input) = text_setting(&instance.settings, &["down", "Down", "input", "Input"]) {
		return Some(input.to_owned());
	}
	if is_hotkey_switch(&instance.action.uuid) {
		return instance
			.settings
			.get("hotkeys")
			.and_then(Value::as_array)
			.and_then(|hotkeys| hotkeys.get(instance.current_state as usize))
			.and_then(|hotkey| hotkey.get("down"))
			.and_then(Value::as_str)
			.map(str::to_owned);
	}
	None
}

fn shortcut(modifiers: &[enigo::Key], key: enigo::Key) -> String {
	let mut tokens = modifiers.iter().copied().map(|key| Token::Key(key, enigo::Direction::Press)).collect::<Vec<_>>();
	tokens.push(Token::Key(key, enigo::Direction::Click));
	tokens.extend(modifiers.iter().rev().copied().map(|key| Token::Key(key, enigo::Direction::Release)));
	ron::to_string(&tokens).expect("Enigo key tokens must serialize")
}

fn preset_input(uuid: &str) -> Option<String> {
	use enigo::Key::{Control, Delete, F1, F5, F11, Meta, Shift, Space, Unicode};
	let (modifiers, key): (&[enigo::Key], enigo::Key) = match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.hotkey.browser.back" => (&[Meta], Unicode('[')),
		"com.hotspot.streamdock.hotkey.browser.forward" => (&[Meta], Unicode(']')),
		"com.hotspot.streamdock.hotkey.browser.refresh" => (&[Meta], Unicode('r')),
		"com.hotspot.streamdock.hotkey.browser.collect" => (&[Meta], Unicode('d')),
		"com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop" | "com.hotspot.streamdock.touchbar.showdesktop" => (&[], F11),
		"com.hotspot.streamdock.hotkey.quicktool.searchbar" => (&[Meta], Space),
		"com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition" | "com.hotspot.streamdock.touchbar.dictation" => (&[], F5),
		"com.hotspot.streamdock.hotkey.quickcontrol.switchscreen" => (&[Control], F1),
		"com.hotspot.streamdock.touchbar.inputmethod" => (&[Control], Space),
		"com.hotspot.streamdock.touchbar.screenlock" => (&[Meta, Control], Unicode('q')),
		"com.hotspot.streamdock.hotkey.pr.copy" => (&[Meta], Unicode('c')),
		"com.hotspot.streamdock.hotkey.pr.paste" => (&[Meta], Unicode('v')),
		"com.hotspot.streamdock.hotkey.pr.cut" => (&[Meta], Unicode('x')),
		"com.hotspot.streamdock.hotkey.pr.play" => (&[], Space),
		"com.hotspot.streamdock.hotkey.pr.delete" => (&[], Delete),
		"com.hotspot.streamdock.hotkey.pr.undo" => (&[Meta], Unicode('z')),
		"com.hotspot.streamdock.hotkey.pr.addedit" => (&[Meta], Unicode('k')),
		"com.hotspot.streamdock.hotkey.pr.effectspanel" => (&[Shift], Unicode('7')),
		"com.hotspot.streamdock.hotkey.pr.togglefullscreen" => (&[Control], Unicode('`')),
		"com.hotspot.streamdock.hotkey.pr.razor" => (&[], Unicode('c')),
		"com.hotspot.streamdock.hotkey.pr.pentool" => (&[], Unicode('p')),
		"com.hotspot.streamdock.hotkey.pr.rectangletool" => (&[], Unicode('q')),
		"com.hotspot.streamdock.hotkey.pr.rippleeditingtools" => (&[], Unicode('b')),
		"com.hotspot.streamdock.hotkey.pr.addmarker" => (&[], Unicode('m')),
		"com.hotspot.streamdock.pr.action5" => (&[], Unicode('\\')),
		_ => return None,
	};
	Some(shortcut(modifiers, key))
}

async fn send_text(text: &str) -> Result<(), anyhow::Error> {
	let encoded = ron::to_string(&vec![Token::Text(text.to_owned())])?;
	crate::m18_actions::execute_input(Some(encoded)).await
}

fn app_for_uuid(uuid: &str) -> Option<&'static str> {
	match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.quicktool.calculator" => Some("Calculator"),
		"com.hotspot.streamdock.quicktool.controlpanel" => Some("System Settings"),
		"com.hotspot.streamdock.quicktool.mail" => Some("Mail"),
		"com.hotspot.streamdock.quicktool.music" => Some("Music"),
		// The catalogued UUID carries a `.hotkey.` segment; both spellings exist.
		"com.hotspot.streamdock.quicktool.taskmanager" | "com.hotspot.streamdock.hotkey.quicktool.taskmanager" => Some("Activity Monitor"),
		"com.hotspot.streamdock.quicktool.homepage" => Some("Safari"),
		"com.hotspot.streamdock.touchbar.launchpad" => Some("Launchpad"),
		_ => None,
	}
}

/// Actions that open macOS's Emoji & Symbols viewer. It is not an app that
/// `open -a` can launch; its standard shortcut is ⌃⌘Space.
fn opens_emoji_viewer(uuid: &str) -> bool {
	matches!(
		uuid.to_ascii_lowercase().as_str(),
		"com.hotspot.streamdock.quicktool.emoticons" | "com.hotspot.streamdock.hotkey.quicktool.emoticons" | "com.hotspot.streamdock.emoticons.lib"
	)
}

fn emoji_viewer_input() -> String {
	shortcut(&[enigo::Key::Control, enigo::Key::Meta], enigo::Key::Space)
}

/// Touch Bar media keys, sent as real media keys so they control whichever
/// player is active (Music, Spotify, a browser), as the hardware keys do.
fn media_key_input(uuid: &str) -> Option<&'static str> {
	match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.touchbar.playpause" => Some("[k(MediaPlayPause)]"),
		"com.hotspot.streamdock.touchbar.nexttrack" => Some("[k(MediaNextTrack)]"),
		"com.hotspot.streamdock.touchbar.previoustrack" => Some("[k(MediaPrevTrack)]"),
		#[cfg(target_os = "macos")]
		"com.hotspot.streamdock.touchbar.fastforward" => Some("[k(MediaFast)]"),
		#[cfg(target_os = "macos")]
		"com.hotspot.streamdock.touchbar.fastrewind" => Some("[k(MediaRewind)]"),
		_ => None,
	}
}

async fn system_action(uuid: &str, settings: &Value) -> Result<bool, anyhow::Error> {
	let uuid = uuid.to_ascii_lowercase();
	let action_index = number_setting(settings, &["actionIdx", "ActionIndex", "index"]).unwrap_or(0);
	if uuid == "com.hotspot.streamdock.system.multimedia" {
		if let Some(input) = multimedia_input(action_index) {
			crate::m18_actions::execute_input(Some(input.to_owned())).await?;
			return Ok(true);
		}
		if action_index == 3 {
			#[cfg(target_os = "macos")]
			run(
				"/usr/bin/osascript",
				vec![
					"-e".to_owned(),
					"tell application \"System Events\" to set musicIsRunning to exists (process \"Music\")\nif musicIsRunning then tell application \"Music\" to stop".to_owned(),
				],
			)
			.await?;
			#[cfg(not(target_os = "macos"))]
			crate::m18_actions::execute_input(Some("[k(MediaStop)]".to_owned())).await?;
			return Ok(true);
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.quickcontrol.microphone" {
		#[cfg(target_os = "macos")]
		crate::macos_audio::toggle_default_input_mute()?;
		#[cfg(target_os = "linux")]
		crate::m18_actions::execute_input(Some("[k(MicMute)]".to_owned())).await?;
		#[cfg(target_os = "windows")]
		open_target("ms-settings:sound").await?;
		return Ok(true);
	}
	let args = match uuid.as_str() {
		"com.hotspot.streamdock.hotkey.quickcontrol.volumedown" | "com.hotspot.streamdock.touchbar.volumedown" => {
			Some(("/usr/bin/osascript", "set volume output volume ((output volume of (get volume settings)) - 6)"))
		}
		"com.hotspot.streamdock.hotkey.quickcontrol.volumeup" | "com.hotspot.streamdock.touchbar.volumeup" => {
			Some(("/usr/bin/osascript", "set volume output volume ((output volume of (get volume settings)) + 6)"))
		}
		"com.hotspot.streamdock.hotkey.quickcontrol.mute" | "com.hotspot.streamdock.touchbar.mute" => {
			Some(("/usr/bin/osascript", "set volume output muted (not (output muted of (get volume settings)) )"))
		}
		"com.hotspot.streamdock.quickcontrol.sleep" | "com.hotspot.streamdock.touchbar.sleep" | "com.hotspot.streamdock.device.devsleep" => Some(("/usr/bin/pmset", "displaysleepnow")),
		"com.hotspot.streamdock.touchbar.desktopsaver" => Some(("/usr/bin/open", "-a ScreenSaverEngine")),
		"com.hotspot.streamdock.touchbar.siri" => Some(("/usr/bin/open", "-a Siri")),
		// "Dispatch Center" is VSD Craft's translation of 调度中心: Mission Control.
		"com.hotspot.streamdock.touchbar.dispatchcenter" => Some(("/usr/bin/open", "-a Mission Control")),
		"com.hotspot.streamdock.touchbar.screenshot" => Some(("/usr/sbin/screencapture", "-i -c")),
		"com.hotspot.streamdock.touchbar.decreasescreenbrightness" => Some(("/usr/bin/osascript", "tell application \"System Events\" to key code 145")),
		"com.hotspot.streamdock.touchbar.increasescreenbrightness" => Some(("/usr/bin/osascript", "tell application \"System Events\" to key code 144")),
		"com.hotspot.streamdock.touchbar.notificationcenter" | "com.hotspot.streamdock.quicktool.notification" => None,
		"com.hotspot.streamdock.touchbar.dndmode" => Some(("/usr/bin/open", "x-apple.systempreferences:com.apple.Focus-Settings.extension")),
		_ => None,
	};
	if let Some((program, argument)) = args {
		let arguments = match program {
			"/usr/sbin/screencapture" => vec!["-i".to_owned(), "-c".to_owned()],
			"/usr/bin/open" if argument.starts_with("-a ") => vec!["-a".to_owned(), argument[3..].to_owned()],
			"/usr/bin/open" => vec![argument.to_owned()],
			"/usr/bin/pmset" => vec![argument.to_owned()],
			_ => vec!["-e".to_owned(), argument.to_owned()],
		};
		run(program, arguments).await?;
		return Ok(true);
	}
	Ok(false)
}

pub async fn key_down(instance: &crate::shared::ActionInstance) -> Result<(), anyhow::Error> {
	let uuid = instance.action.uuid.as_str();
	if is_hotkey_switch(uuid) {
		let Some(input) = configured_input(instance) else { return Ok(()) };
		crate::m18_actions::execute_input(Some(input)).await?;
	} else if uuid.eq_ignore_ascii_case("com.hotspot.streamdock.system.super.hotkey") {
		if let Some(input) = text_setting(&instance.settings, &["down", "Down"]) {
			crate::m18_actions::execute_input(Some(input.to_owned())).await?;
		}
	}
	Ok(())
}

pub async fn key_up(instance: &crate::shared::ActionInstance) -> Result<bool, anyhow::Error> {
	let uuid = instance.action.uuid.to_ascii_lowercase();
	// Say so on the key instead of silently doing nothing.
	if crate::action_library::is_coming_soon(&uuid) {
		return Err(anyhow::anyhow!("{} is not available yet", instance.action.name));
	}
	if is_hotkey_switch(&uuid) {
		if let Some(input) = instance
			.settings
			.get("hotkeys")
			.and_then(Value::as_array)
			.and_then(|hotkeys| hotkeys.get(instance.current_state as usize))
			.and_then(|hotkey| hotkey.get("up"))
			.and_then(Value::as_str)
			.filter(|input| !input.trim().is_empty())
		{
			crate::m18_actions::execute_input(Some(input.to_owned())).await?;
		}
		return Ok(true);
	}
	if uuid == "com.hotspot.streamdock.system.super.hotkey" {
		if let Some(input) = text_setting(&instance.settings, &["up", "Up"]) {
			crate::m18_actions::execute_input(Some(input.to_owned())).await?;
		}
		return Ok(false);
	}
	if is_hotkey(&uuid) || uuid.contains(".hotkey.") || uuid.contains("hotkey.pr.") {
		if let Some(input) = configured_input(instance).or_else(|| preset_input(&uuid)) {
			crate::m18_actions::execute_input(Some(input)).await?;
			return Ok(false);
		}
		// An ordinary Hotkey with nothing recorded does nothing. Preset UUIDs that
		// merely carry a `.hotkey.` segment (volume, mute, Task Manager, emoji,
		// notifications) have no key sequence and fall through to their handlers.
		if is_hotkey(&uuid) {
			return Ok(false);
		}
	}
	if opens_emoji_viewer(&uuid) {
		crate::m18_actions::execute_input(Some(emoji_viewer_input())).await?;
		return Ok(false);
	}
	if let Some(input) = media_key_input(&uuid) {
		crate::m18_actions::execute_input(Some(input.to_owned())).await?;
		return Ok(false);
	}
	if let Some(app) = app_for_uuid(&uuid) {
		open_application(app).await?;
		return Ok(false);
	}
	if !is_hotkey(&uuid)
		&& !uuid.contains(".hotkey.")
		&& !uuid.contains("hotkey.pr.")
		&& let Some(input) = preset_input(&uuid)
	{
		crate::m18_actions::execute_input(Some(input.to_owned())).await?;
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.system.openapps" {
		if let Some(app) = text_setting(&instance.settings, &["appPath", "path", "application", "app"]) {
			open_application(app).await?;
		}
		return Ok(false);
	}
	if matches!(uuid.as_str(), "com.hotspot.streamdock.system.open" | "com.hotspot.streamdock.system.website") {
		if let Some(target) = text_setting(&instance.settings, &["path", "Path", "url", "URL", "website", "link", "file", "folder"]) {
			open_target(target).await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.mouse.event" {
		if let Some(input) = mouse_event_input(&instance.settings) {
			crate::m18_actions::execute_input(Some(input)).await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.device.brightness" {
		let action_index = number_setting(&instance.settings, &["actionIdx", "ActionIndex"]).unwrap_or(0);
		let Some(adjustment) = brightness_adjustment(action_index) else {
			log::warn!("VSD M18 brightness action has an unknown action index: {action_index}");
			return Ok(false);
		};
		crate::m18::adjust_brightness(&instance.context.device, adjustment).await?;
		if let Some(app) = crate::APP_HANDLE.get() {
			app.emit(
				"device_brightness",
				json!({ "action": if adjustment > 0 { "increase" } else { "decrease" }, "value": adjustment.unsigned_abs() }),
			)?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.system.close" {
		if let Some(app) = text_setting(&instance.settings, &["appPath", "path", "application", "app"]) {
			#[cfg(target_os = "macos")]
			run("/usr/bin/osascript", vec!["-e".to_owned(), format!("tell application {:?} to quit", app)]).await?;
		}
		return Ok(false);
	}
	if matches!(uuid.as_str(), "com.hotspot.streamdock.system.text" | "com.hotspot.streamdock.plain.text") {
		if let Some(text) = text_setting(&instance.settings, &["text", "Text", "value", "Value"]) {
			send_text(text).await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.system.password" {
		if let Some(password) = text_setting(&instance.settings, &["password", "Password", "text", "Text"]) {
			send_text(password).await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.network.udp" {
		let host = text_setting(&instance.settings, &["host", "Host", "ip", "IP", "address", "Address"]).unwrap_or("127.0.0.1");
		let port = number_setting(&instance.settings, &["port", "Port"]).unwrap_or(5000).clamp(1, u16::MAX as i64);
		let message = text_setting(&instance.settings, &["message", "Message", "data", "Data"]).unwrap_or("").as_bytes().to_vec();
		let target = format!("{host}:{port}");
		tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
			let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
			socket.send_to(&message, target)?;
			Ok(())
		})
		.await??;
		return Ok(false);
	}
	if system_action(&uuid, &instance.settings).await? {
		return Ok(false);
	}
	if uuid.starts_with("com.hotspot.streamdock.page.") {
		match uuid.as_str() {
			"com.hotspot.streamdock.page.previous" => crate::m18_pages::switch_to(&instance.context.device, None, None, -1).await?,
			"com.hotspot.streamdock.page.next" => crate::m18_pages::switch_to(&instance.context.device, None, None, 1).await?,
			"com.hotspot.streamdock.page.goto" => {
				let index = number_setting(&instance.settings, &["PageIndex", "pageIndex", "page"]).unwrap_or(0).saturating_sub(1).max(0) as usize;
				crate::m18_pages::switch_to(&instance.context.device, None, Some(index), 0).await?;
			}
			_ => {}
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.profile.backtoparent" {
		crate::m18_pages::go_back(&instance.context.device).await?;
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.profile.openchild" {
		if let Some(target) = text_setting(&instance.settings, &["profile", "ProfileUUID", "target"]) {
			crate::m18_pages::open_folder(&instance.context.device, target).await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.profile.rotate" {
		if let Some(target) = text_setting(&instance.settings, &["profile", "ProfileUUID", "target"]) {
			crate::m18_pages::switch_to(&instance.context.device, Some(target), None, 0).await?;
		} else {
			crate::m18_pages::switch_to(&instance.context.device, None, None, 1).await?;
		}
		return Ok(false);
	}
	if uuid == "com.mirabox.streamdock.emoji.emoji" || uuid == "com.mirabox.streamdock.emoji.emoji_send" {
		if let Some(emoji) = text_setting(&instance.settings, &["emoji", "Emoji", "text", "Text", "value"]) {
			send_text(emoji).await?;
		} else {
			crate::m18_actions::execute_input(Some(emoji_viewer_input())).await?;
		}
		return Ok(false);
	}
	if matches!(
		uuid.as_str(),
		"com.hotspot.streamdock.quicktool.notification" | "com.hotspot.streamdock.hotkey.quicktool.notification" | "com.hotspot.streamdock.touchbar.notificationcenter"
	) {
		#[cfg(target_os = "macos")]
		{
			// macOS has no public notification-center activation API; expose the
			// system surface rather than silently pretending a key event worked.
			open_target("x-apple.systempreferences:com.apple.Notifications-Settings.extension").await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.soundboard.playaudio" {
		return crate::soundboard::play(instance).await;
	}
	if uuid == "com.hotspot.streamdock.soundboard.stopaudioplay" {
		crate::soundboard::stop_all()?;
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.multiactions.delay" {
		return Ok(false);
	}
	if uuid.contains("quickcontrol") || uuid.contains("touchbar") {
		return Ok(false);
	}
	// `uuid` is lower-cased above, so every pattern here must be lower-case.
	if uuid.contains("datetime")
		|| uuid.contains("calendar")
		|| uuid.contains("time.action")
		|| uuid.contains("weather")
		|| uuid.contains("youtube")
		|| uuid.contains("soundboard")
		|| uuid.contains("memo")
		|| uuid.contains("watertank")
		|| uuid.contains("pigmenteffects")
		|| uuid.contains("eatgoldcoins")
		|| uuid.contains("musicalrhythma")
		|| uuid.contains("emoticons")
	{
		return Ok(false);
	}
	if let Some(definition) = definition(&uuid) {
		log::warn!(
			"VSD M18 action '{}' ({}) needs a configuration or live data source; no action was sent",
			definition.name,
			definition.uuid
		);
	}
	Ok(false)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn brightness_actions_use_device_relative_steps() {
		assert_eq!(brightness_adjustment(0), Some(6));
		assert_eq!(brightness_adjustment(1), Some(-6));
		assert_eq!(brightness_adjustment(2), None);
	}

	#[test]
	fn multimedia_actions_cover_all_seven_vsd_operations() {
		assert_eq!(multimedia_input(0), Some("[k(MediaPrevTrack)]"));
		assert_eq!(multimedia_input(1), Some("[k(MediaPlayPause)]"));
		assert_eq!(multimedia_input(2), Some("[k(MediaNextTrack)]"));
		assert_eq!(multimedia_input(3), None); // Stop is handled per platform.
		assert_eq!(multimedia_input(4), Some("[k(VolumeMute)]"));
		assert_eq!(multimedia_input(5), Some("[k(VolumeUp)]"));
		assert_eq!(multimedia_input(6), Some("[k(VolumeDown)]"));
		assert_eq!(multimedia_input(7), None);
	}

	#[test]
	fn local_shortcut_catalog_actions_generate_valid_enigo_tokens() {
		for uuid in [
			"com.hotspot.streamdock.hotkey.browser.back",
			"com.hotspot.streamdock.hotkey.browser.collect",
			"com.hotspot.streamdock.hotkey.browser.forward",
			"com.hotspot.streamdock.hotkey.browser.refresh",
			"com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop",
			"com.hotspot.streamdock.hotkey.quicktool.searchbar",
			"com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition",
			"com.hotspot.streamdock.hotkey.quickcontrol.switchscreen",
			"com.hotspot.streamdock.touchbar.dictation",
			"com.hotspot.streamdock.touchbar.inputmethod",
			"com.hotspot.streamdock.touchbar.screenlock",
			"com.hotspot.streamdock.touchbar.showdesktop",
		] {
			let input = preset_input(uuid).unwrap_or_else(|| panic!("missing key sequence for {uuid}"));
			assert!(!ron::from_str::<Vec<Token>>(&input).unwrap().is_empty(), "invalid Enigo token sequence for {uuid}");
		}
	}

	#[test]
	fn catalog_contains_every_supported_keypad_action_without_action_plugins() {
		assert_eq!(definitions().len(), 109);
		assert!(definition("com.hotspot.streamdock.touchbar.siri").is_some());
		assert!(definition("com.hotspot.streamdock.device.brightness").is_some());
		assert!(definition("com.hotspot.streamdock.profile.rotate").is_some());
		assert!(definition("com.hotspot.streamdock.mouse.event").is_some());
		assert!(definition("com.hotspot.streamdock.system.hotkey").is_some());
		assert!(definition("com.hotspot.streamdock.system.super.hotkey").is_some());
		assert!(definition("com.hotspot.streamdock.system.multimedia").is_some());
		assert!(definition("com.hotspot.streamdock.quickcontrol.microphone").is_some());
		assert!(definition("com.hotspot.streamdock.plain.text").is_none());
		assert!(definition("com.hotspot.streamdock.multiactions.ActionWheel").is_none());
		assert!(definition("com.hotspot.streamdock.device.k1proLED+").is_none());
		assert!(is_composite_action("com.hotspot.streamdock.multiactions.routine"));
		assert!(is_composite_action("com.hotspot.streamdock.multiactions.LunBo"));
	}

	#[test]
	fn defaults_match_the_vsd_time_action_settings_schema() {
		assert_eq!(
			default_settings("com.mirabox.streamdock.time.action1"),
			json!({
				"amplify": true,
				"zone": "system",
				"theme": "theme1",
				"isBackgroundHidden": false
			})
		);
		assert_eq!(default_settings("com.mirabox.streamdock.time.action2"), json!({ "select": "1", "deep": 0 }));
		assert_eq!(
			default_settings("com.mirabox.streamdock.time.action3"),
			json!({
				"select": "1",
				"surplus": 60000,
				"timing": "60000",
				"inputTime": "0",
				"musicUrl": "",
				"color": "rgb(0,255,0)"
			})
		);
	}

	#[test]
	fn mouse_action_tokens_cover_click_move_scroll_drag_and_modifiers() {
		let click = mouse_event_input(&json!({ "eventType": "click", "button": "right" })).unwrap();
		assert_eq!(ron::from_str::<Vec<Token>>(&click).unwrap(), vec![Token::Button(enigo::Button::Right, enigo::Direction::Click)]);

		let double = mouse_event_input(&json!({ "eventType": "doubleClick", "button": "left" })).unwrap();
		assert_eq!(ron::from_str::<Vec<Token>>(&double).unwrap().len(), 4);

		let movement = mouse_event_input(&json!({ "eventType": "move", "x": -42, "y": 18, "coordinate": "relative" })).unwrap();
		assert_eq!(ron::from_str::<Vec<Token>>(&movement).unwrap(), vec![Token::MoveMouse(-42, 18, enigo::Coordinate::Rel)]);

		let scroll = mouse_event_input(&json!({ "eventType": "scroll", "amount": -5, "axis": "horizontal" })).unwrap();
		assert_eq!(ron::from_str::<Vec<Token>>(&scroll).unwrap(), vec![Token::Scroll(-5, enigo::Axis::Horizontal)]);

		let drag = mouse_event_input(&json!({ "eventType": "drag", "x": 30, "y": -20, "button": "middle", "coordinate": "relative", "modifiers": ["shift"] })).unwrap();
		let drag_tokens = ron::from_str::<Vec<Token>>(&drag).unwrap();
		assert_eq!(drag_tokens.len(), 5);
		assert!(matches!(drag_tokens.first(), Some(Token::Key(enigo::Key::Shift, enigo::Direction::Press))));
		assert!(matches!(drag_tokens.get(2), Some(Token::MoveMouse(30, -20, enigo::Coordinate::Rel))));
		assert!(matches!(drag_tokens.last(), Some(Token::Key(enigo::Key::Shift, enigo::Direction::Release))));
	}
}
