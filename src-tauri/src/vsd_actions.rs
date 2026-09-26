//! M18-native catalog and runtime for VSD Craft actions.
//!
//! This is deliberately application-core functionality: VSD/M18 actions do
//! not spawn or depend on action-plugin processes. The catalog metadata is
//! captured from VSD Craft's installed action manifests; device-specific
//! encoder/K1 Pro entries are excluded.

use crate::shared::{Action, ActionState, Category};
use enigo::agent::Token;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::process::Command;
use std::sync::LazyLock;

#[derive(Clone, Debug, Deserialize)]
pub struct VsdActionDefinition {
	pub name: String,
	pub uuid: String,
	pub tooltip: String,
	pub category: String,
	#[serde(rename = "stateCount")]
	pub state_count: usize,
	#[serde(rename = "supportedInMultiActions")]
	pub supported_in_multi_actions: bool,
}

static DEFINITIONS: LazyLock<Vec<VsdActionDefinition>> = LazyLock::new(|| serde_json::from_str(include_str!("vsd_action_catalog.json")).expect("bundled VSD action catalog must be valid JSON"));

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

fn category_icon(category: &str) -> &'static str {
	match category {
		"Action Flows" => "opendeck/multi-action.png",
		"Pages & Profiles" => "opendeck/page-navigation.svg",
		"Shortcuts & Input" => "opendeck/apps-hotkeys.svg",
		"System & Apps" => "opendeck/system-controls.svg",
		"Media & VSD Extras" | "Time & Online" => "opendeck/system-controls.svg",
		"Network & Integrations" => "opendeck/device-controls.svg",
		_ => "opendeck/device-controls.svg",
	}
}

pub fn insert_catalog(categories: &mut HashMap<String, Category>) {
	for definition in definitions() {
		let category_name = format!("M18 · {}", definition.category);
		let icon = category_icon(&definition.category);
		let category = categories.entry(category_name).or_insert_with(|| Category {
			icon: Some(icon.to_owned()),
			actions: vec![],
		});
		let state_count = definition.state_count.max(1);
		let states = (0..state_count)
			.map(|index| ActionState {
				image: "opendeck/multi-action.png".to_owned(),
				name: if state_count > 1 { format!("{} {}", definition.name, index + 1) } else { definition.name.clone() },
				..Default::default()
			})
			.collect();
		category.actions.push(Action {
			name: definition.name.clone(),
			uuid: definition.uuid.clone(),
			plugin: String::new(),
			tooltip: definition.tooltip.clone(),
			icon: icon.to_owned(),
			disable_automatic_states: false,
			visible_in_action_list: true,
			supported_in_multi_actions: definition.supported_in_multi_actions,
			property_inspector: String::new(),
			controllers: vec!["Keypad".to_owned()],
			encoder: None,
			states,
		});
	}
}

pub fn default_settings(uuid: &str) -> Value {
	match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.system.openapps" => json!({ "appPath": "" }),
		"com.hotspot.streamdock.system.open" | "com.hotspot.streamdock.system.website" => json!({ "path": "" }),
		"com.hotspot.streamdock.system.hotkey" | "com.hotspot.streamdock.system.super.hotkey" => json!({ "down": "", "up": "", "display": "" }),
		"com.hotspot.streamdock.system.hotkeyswitch" => json!({ "hotkeys": [{ "down": "", "up": "" }, { "down": "", "up": "" }], "index": 0 }),
		"com.hotspot.streamdock.system.text" | "com.hotspot.streamdock.plain.text" => json!({ "text": "" }),
		"com.hotspot.streamdock.system.password" => json!({ "password": "" }),
		"com.hotspot.streamdock.network.udp" => json!({ "host": "127.0.0.1", "port": 5000, "message": "" }),
		"com.hotspot.streamdock.soundboard.playaudio" => json!({ "path": "", "mode": "Play/Stop", "volume": 100, "outputDevice": "default", "fadeType": "none", "fadeDuration": 0 }),
		"com.hotspot.streamdock.youtube.chatmessage" => json!({ "videoId": "", "message": "" }),
		"com.hotspot.streamdock.youtube.viewers" => json!({ "videoId": "" }),
		"com.hotspot.streamdock.weather.action1" => json!({ "location": "", "units": "fahrenheit" }),
		"com.mirabox.streamdock.time.action2" | "com.mirabox.streamdock.time.action3" => json!({ "durationSeconds": 60, "label": "" }),
		"com.mirabox.streamdock.time.action1" => json!({ "timeZone": "local", "format": "%H:%M" }),
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

fn preset_input(uuid: &str) -> Option<&'static str> {
	match uuid.to_ascii_lowercase().as_str() {
		"com.hotspot.streamdock.hotkey.browser.back" => Some("[k(meta,uni('['))]"),
		"com.hotspot.streamdock.hotkey.browser.forward" => Some("[k(meta,uni(']'))]"),
		"com.hotspot.streamdock.hotkey.browser.refresh" => Some("[k(meta,uni('r'))]"),
		"com.hotspot.streamdock.hotkey.browser.collect" => Some("[k(meta,uni('d'))]"),
		"com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop" | "com.hotspot.streamdock.touchbar.showdesktop" => Some("[k(uni('F11'))]"),
		"com.hotspot.streamdock.hotkey.quicktool.searchbar" => Some("[k(meta,uni(' '))]"),
		"com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition" => Some("[k(uni('F5'))]"),
		"com.hotspot.streamdock.hotkey.quickcontrol.switchscreen" => Some("[k(ctrl,uni('F1'))]"),
		"com.hotspot.streamdock.touchbar.dndmode" => Some("[k(meta,ctrl,uni('d'))]"),
		"com.hotspot.streamdock.touchbar.dictation" => Some("[k(uni('F5'))]"),
		"com.hotspot.streamdock.touchbar.inputmethod" => Some("[k(ctrl,uni(' '))]"),
		"com.hotspot.streamdock.touchbar.notificationcenter" | "com.hotspot.streamdock.quicktool.notification" => Some("[k(ctrl,uni('F8'))]"),
		"com.hotspot.streamdock.touchbar.screenlock" => Some("[k(meta,ctrl,uni('q'))]"),
		"com.hotspot.streamdock.hotkey.pr.copy" => Some("[k(meta,uni('c'))]"),
		"com.hotspot.streamdock.hotkey.pr.paste" => Some("[k(meta,uni('v'))]"),
		"com.hotspot.streamdock.hotkey.pr.cut" => Some("[k(meta,uni('x'))]"),
		"com.hotspot.streamdock.hotkey.pr.play" => Some("[k(uni(' '))]"),
		"com.hotspot.streamdock.hotkey.pr.delete" => Some("[k(uni('Delete'))]"),
		"com.hotspot.streamdock.hotkey.pr.undo" => Some("[k(meta,uni('z'))]"),
		"com.hotspot.streamdock.hotkey.pr.addedit" => Some("[k(meta,uni('k'))]"),
		"com.hotspot.streamdock.hotkey.pr.effectspanel" => Some("[k(shift,uni('7'))]"),
		"com.hotspot.streamdock.hotkey.pr.togglefullscreen" => Some("[k(ctrl,uni('`'))]"),
		"com.hotspot.streamdock.hotkey.pr.razor" => Some("[k(uni('c'))]"),
		"com.hotspot.streamdock.hotkey.pr.pentool" => Some("[k(uni('p'))]"),
		"com.hotspot.streamdock.hotkey.pr.rectangletool" => Some("[k(uni('q'))]"),
		"com.hotspot.streamdock.hotkey.pr.rippleeditingtools" => Some("[k(uni('b'))]"),
		"com.hotspot.streamdock.hotkey.pr.addmarker" => Some("[k(uni('m'))]"),
		"com.hotspot.streamdock.pr.action5" => Some("[k(uni('\\'))]"),
		_ => None,
	}
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
		"com.hotspot.streamdock.quicktool.taskmanager" => Some("Activity Monitor"),
		"com.hotspot.streamdock.quicktool.homepage" => Some("Safari"),
		"com.hotspot.streamdock.quicktool.emoticons" | "com.hotspot.streamdock.emoticons.lib" => Some("Character Viewer"),
		"com.hotspot.streamdock.touchbar.launchpad" => Some("Launchpad"),
		_ => None,
	}
}

async fn system_action(uuid: &str, settings: &Value) -> Result<bool, anyhow::Error> {
	let uuid = uuid.to_ascii_lowercase();
	let action_index = number_setting(settings, &["actionIdx", "ActionIndex", "index"]).unwrap_or(0);
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
		"com.hotspot.streamdock.touchbar.dispatchcenter" => Some(("/usr/bin/open", "x-apple.systempreferences:com.apple.ControlCenter-Settings.extension")),
		"com.hotspot.streamdock.touchbar.screenshot" => Some(("/usr/sbin/screencapture", "-i -c")),
		"com.hotspot.streamdock.touchbar.decreasescreenbrightness" => Some(("/usr/bin/osascript", "tell application \"System Events\" to key code 145")),
		"com.hotspot.streamdock.touchbar.increasescreenbrightness" => Some(("/usr/bin/osascript", "tell application \"System Events\" to key code 144")),
		"com.hotspot.streamdock.system.multimedia" => match action_index {
			1 => Some(("/usr/bin/osascript", "tell application \"Music\" to playpause")),
			2 => Some(("/usr/bin/osascript", "tell application \"Music\" to previous track")),
			3 => Some(("/usr/bin/osascript", "tell application \"Music\" to next track")),
			_ => None,
		},
		"com.hotspot.streamdock.quickcontrol.microphone" => Some(("/usr/bin/open", "x-apple.systempreferences:com.apple.Sound-Settings.extension")),
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
		if let Some(input) = configured_input(instance).or_else(|| preset_input(&uuid).map(str::to_owned)) {
			crate::m18_actions::execute_input(Some(input)).await?;
		}
		return Ok(false);
	}
	if let Some(app) = app_for_uuid(&uuid) {
		open_target(app).await?;
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.system.openapps" {
		if let Some(app) = text_setting(&instance.settings, &["appPath", "path", "application", "app"]) {
			open_target(app).await?;
		}
		return Ok(false);
	}
	if matches!(uuid.as_str(), "com.hotspot.streamdock.system.open" | "com.hotspot.streamdock.system.website") {
		if let Some(target) = text_setting(&instance.settings, &["path", "Path", "url", "URL", "website", "link", "file", "folder"]) {
			open_target(target).await?;
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
		let target = text_setting(&instance.settings, &["parentProfile", "parent", "ProfileUUID"]).unwrap_or("Default");
		crate::m18_pages::switch_to(&instance.context.device, Some(target), None, 0).await?;
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.profile.openchild" || uuid == "com.hotspot.streamdock.profile.rotate" {
		if let Some(target) = text_setting(&instance.settings, &["profile", "ProfileUUID", "target"]) {
			crate::m18_pages::switch_to(&instance.context.device, Some(target), None, 0).await?;
		} else {
			crate::m18_pages::switch_to(&instance.context.device, None, None, 1).await?;
		}
		return Ok(false);
	}
	if uuid.starts_with("com.mirabox.streamdock.screensaver.") {
		crate::screensaver::start_device(&instance.context.device).await;
		return Ok(false);
	}
	if uuid == "com.mirabox.streamdock.emoji.emoji" || uuid == "com.mirabox.streamdock.emoji.emoji_send" {
		if let Some(emoji) = text_setting(&instance.settings, &["emoji", "Emoji", "text", "Text", "value"]) {
			send_text(emoji).await?;
		} else {
			open_target("Character Viewer").await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.quicktool.notification" || uuid == "com.hotspot.streamdock.touchbar.notificationcenter" {
		#[cfg(target_os = "macos")]
		{
			// macOS has no public notification-center activation API; expose the
			// system surface rather than silently pretending a key event worked.
			open_target("x-apple.systempreferences:com.apple.Notifications-Settings.extension").await?;
		}
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.soundboard.stopaudioplay" {
		return Ok(false);
	}
	if uuid == "com.hotspot.streamdock.multiactions.delay" {
		return Ok(false);
	}
	if uuid.contains("quickcontrol") || uuid.contains("touchbar") || uuid.contains("system.multimedia") {
		return Ok(false);
	}
	if uuid.contains("dateTime")
		|| uuid.contains("calendar")
		|| uuid.contains("time.action")
		|| uuid.contains("Weather")
		|| uuid.contains("weather")
		|| uuid.contains("youtube")
		|| uuid.contains("soundboard")
		|| uuid.contains("memo")
		|| uuid.contains("watertank")
		|| uuid.contains("pigmenteffects")
		|| uuid.contains("eatgoldcoins")
		|| uuid.contains("musicalrhythma")
		|| uuid.contains("emoticons")
		|| uuid.contains("pictureEmoticons")
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
	fn catalog_contains_every_supported_keypad_action_without_action_plugins() {
		assert_eq!(definitions().len(), 103);
		assert!(definition("com.hotspot.streamdock.touchbar.siri").is_some());
		assert!(definition("com.hotspot.streamdock.multiactions.ActionWheel").is_none());
		assert!(definition("com.hotspot.streamdock.device.k1proLED+").is_none());
		assert!(is_composite_action("com.hotspot.streamdock.multiactions.routine"));
		assert!(is_composite_action("com.hotspot.streamdock.multiactions.LunBo"));
	}

	#[test]
	fn catalog_actions_are_native_keypad_actions() {
		let mut categories = HashMap::new();
		insert_catalog(&mut categories);
		for definition in definitions() {
			let category = format!("M18 · {}", definition.category);
			let action = categories[&category].actions.iter().find(|action| action.uuid == definition.uuid).unwrap();
			assert!(action.plugin.is_empty());
			assert_eq!(action.controllers, ["Keypad"]);
		}
	}
}
