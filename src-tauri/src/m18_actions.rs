//! Native actions for the M18 workflow.
//!
//! These actions live in the application core. They do not require a second
//! action-plugin process, which is important for the actions used by the
//! user's everyday M18 profile.

use crate::shared::ActionInstance;

use base64::Engine;
use enigo::{
	Enigo, Settings,
	agent::{Agent, Token},
};
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
pub const PAGE_PREVIOUS_UUID: &str = "opendeck.m18.page-previous";
pub const PAGE_NEXT_UUID: &str = "opendeck.m18.page-next";
pub const PAGE_GOTO_UUID: &str = "opendeck.m18.page-goto";

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
			| PAGE_PREVIOUS_UUID
			| PAGE_NEXT_UUID
			| PAGE_GOTO_UUID
	)
}

pub fn is_switch_action(uuid: &str) -> bool {
	matches!(uuid, HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID)
}

pub fn default_settings(uuid: &str) -> Value {
	match uuid {
		OPEN_APPS_UUID => serde_json::json!({ "appPath": "" }),
		SUPER_HOTKEYS_UUID => serde_json::json!({ "down": "", "up": "" }),
		HOTKEY_SWITCH_UUID | SUPER_HOTKEY_SWITCH_UUID => serde_json::json!({ "hotkeys": [{ "down": "", "up": "" }, { "down": "", "up": "" }], "index": 0 }),
		PAGE_GOTO_UUID => serde_json::json!({ "page": "", "pageIndex": 0 }),
		_ => Value::Object(serde_json::Map::new()),
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

pub async fn render(instance: &ActionInstance) -> Result<(), anyhow::Error> {
	let image = if instance.action.uuid == OPEN_APPS_UUID {
		app_icon_data_url(&instance.settings).or_else(|| instance.states.get(instance.current_state as usize).and_then(|state| image_data_url(&state.image)))
	} else {
		instance.states.get(instance.current_state as usize).and_then(|state| image_data_url(&state.image))
	};
	crate::events::outbound::devices::update_image((&instance.context).into(), image).await
}

async fn execute_input(input: Option<String>) -> Result<(), anyhow::Error> {
	let Some(input) = input.filter(|value| !value.trim().is_empty()) else {
		return Ok(());
	};

	tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
		let store = ENIGO.get_or_init(|| Mutex::new(None));
		let mut guard = store.lock().map_err(|_| anyhow::anyhow!("M18 input engine lock was poisoned"))?;
		if guard.is_none() {
			guard.replace(Enigo::new(&Settings::default())?);
		}
		let enigo = guard.as_mut().expect("input engine was initialised");
		let tokens: Vec<Token> = ron::from_str(&input)?;
		for token in tokens {
			enigo.execute(&token)?;
		}
		Ok(())
	})
	.await??;
	Ok(())
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
		VOLUME_DOWN_UUID | VOLUME_UP_UUID | MUTE_UUID | SIRI_UUID => {
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
		_ => Ok(false),
	}
}
