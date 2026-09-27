pub mod info_param;
pub mod manifest;
mod webserver;

use crate::APP_HANDLE;
use crate::built_info::TARGET;
use crate::shared::{CATEGORIES, Category, config_dir, convert_icon, is_flatpak, log_dir};

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{LazyLock, mpsc};
use std::{fs, path};

use tauri::{AppHandle, Manager};

use futures::StreamExt;
use tokio::net::{TcpListener, TcpStream};

use anyhow::anyhow;
use log::{error, warn};
use tokio::sync::Mutex;

pub enum PluginChildType {
	Wine,
	Native,
	Node,
}

enum PluginInstance {
	Webview,
	Wine(Child),
	Native(Child),
	Node(Child),
}

static INSTANCES: LazyLock<Mutex<HashMap<String, PluginInstance>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Plugins, property inspectors, and the editor all run on this machine, so
/// the plugin servers listen on the loopback interface only. Binding to every
/// interface exposed plugin control and profile files to the local network.
pub const LOOPBACK: &str = "127.0.0.1";

pub static PORT_BASE: LazyLock<u16> = LazyLock::new(|| {
	let mut base = 57116;
	loop {
		let websocket_result = std::net::TcpListener::bind((LOOPBACK, base));
		let webserver_result = std::net::TcpListener::bind((LOOPBACK, base + 2));
		if websocket_result.is_ok() && webserver_result.is_ok() {
			log::debug!("Using ports {} and {}", base, base + 2);
			break;
		}
		base += 1;
	}
	base
});

/// Browsers attach an `Origin` header to every WebSocket handshake and
/// cross-origin request. Plugin processes send none; the editor, property
/// inspectors, and HTML plugins are served from local origins. Anything else
/// is a web page the user happens to have open, which must never be able to
/// impersonate a plugin or read profile files.
pub fn is_trusted_origin(origin: &str) -> bool {
	let origin = origin.trim().to_ascii_lowercase();
	if origin.starts_with("tauri://") {
		return true;
	}
	let Some(rest) = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://")) else {
		return false;
	};
	let host = rest.split('/').next().unwrap_or_default();
	let host = if let Some(bracketed) = host.strip_prefix('[') {
		bracketed.split(']').next().unwrap_or_default()
	} else {
		host.split(':').next().unwrap_or_default()
	};
	matches!(host, "localhost" | "127.0.0.1" | "::1" | "tauri.localhost")
}

/// Attach a kernel-enforced "die when parent dies" signal to a plugin child process.
#[cfg(target_os = "linux")]
fn attach_parent_death_signal(command: &mut Command) {
	use std::os::unix::process::CommandExt;
	// SAFETY: `libc::prctl` is async-signal-safe.
	unsafe {
		command.pre_exec(move || {
			if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong) != 0 {
				return Err(std::io::Error::last_os_error());
			}
			Ok(())
		});
	}
}

pub type SpawnRequest = Box<dyn FnOnce() -> Result<(String, PluginChildType, Command), anyhow::Error> + Send>;

/// Initialise a plugin from a given directory.
pub async fn initialise_plugin(path: path::PathBuf, spawner_tx: mpsc::Sender<SpawnRequest>) -> anyhow::Result<()> {
	let plugin_uuid = path.file_name().unwrap().to_str().unwrap().to_owned();
	if plugin_uuid == crate::m18::LEGACY_HARDWARE_PLUGIN_ID {
		return Err(anyhow!("the VSD M18 hardware driver is built into OpenDeck VSD M18"));
	}
	let plugin_uuid_2 = plugin_uuid.clone();

	let mut manifest = manifest::read_manifest(&path)?;

	if let Some(icon) = manifest.category_icon {
		let category_icon_path = path.join(icon);
		manifest.category_icon = Some(convert_icon(category_icon_path.to_string_lossy().to_string()));
	}

	for action in &mut manifest.actions {
		plugin_uuid.clone_into(&mut action.plugin);

		let action_icon_path = path.join(action.icon.clone());
		action.icon = convert_icon(action_icon_path.to_str().unwrap().to_owned());

		if !action.property_inspector.is_empty() {
			action.property_inspector = path.join(&action.property_inspector).to_string_lossy().to_string();
		} else if let Some(ref property_inspector) = manifest.property_inspector_path {
			action.property_inspector = path.join(property_inspector).to_string_lossy().to_string();
		}

		if let Some(encoder) = &mut action.encoder {
			if !encoder.icon.is_empty() {
				let encoder_icon_path = path.join(encoder.icon.clone());
				encoder.icon = convert_icon(encoder_icon_path.to_str().unwrap().to_owned());
			}

			if !encoder.background.is_empty() {
				let encoder_background_path = path.join(encoder.background.clone());
				encoder.background = convert_icon(encoder_background_path.to_str().unwrap().to_owned());
			}
		}

		for state in &mut action.states {
			if state.image == "actionDefaultImage" {
				state.image.clone_from(&action.icon);
			} else {
				let state_icon = path.join(state.image.clone());
				state.image = convert_icon(state_icon.to_str().unwrap().to_owned());
			}

			match state.family.clone().to_lowercase().trim() {
				"arial" => "Liberation Sans",
				"arial black" => "Archivo Black",
				"comic sans ms" => "Comic Neue",
				"courier" | "Courier New" => "Courier Prime",
				"georgia" => "Tinos",
				"impact" => "Anton",
				"microsoft sans serif" | "Times New Roman" => "Liberation Serif",
				"tahoma" | "Verdana" => "Open Sans",
				"trebuchet ms" => "Fira Sans",
				_ => continue,
			}
			.clone_into(&mut state.family);
		}
	}

	{
		let mut categories = CATEGORIES.write().await;
		if let Some(category) = categories.get_mut(&manifest.category) {
			for action in manifest.actions {
				if let Some(index) = category.actions.iter().position(|v| v.uuid == action.uuid) {
					category.actions.remove(index);
				}
				category.actions.push(action);
			}
		} else {
			let mut category: Category = Category {
				icon: manifest.category_icon,
				actions: vec![],
			};
			for action in manifest.actions {
				category.actions.push(action);
			}
			if !category.actions.is_empty() {
				categories.insert(manifest.category, category);
			}
		}
	}

	#[cfg(target_os = "windows")]
	let platform = "windows";
	#[cfg(target_os = "macos")]
	let platform = "mac";
	#[cfg(target_os = "linux")]
	let platform = "linux";

	let mut code_path = manifest.code_path;
	let mut use_wine = false;
	let mut supported = false;

	// Determine the method used to run the plugin based on its supported operating systems and the current operating system.
	for os in manifest.os {
		if os.platform == platform {
			#[cfg(target_os = "windows")]
			if manifest.code_path_windows.is_some() {
				code_path = manifest.code_path_windows.clone();
			}
			#[cfg(target_os = "macos")]
			if manifest.code_path_macos.is_some() {
				code_path = manifest.code_path_macos;
			}
			#[cfg(target_os = "linux")]
			if manifest.code_path_linux.is_some() {
				code_path = manifest.code_path_linux;
			}
			code_path = manifest.code_paths.and_then(|p| p.get(TARGET).cloned()).or(code_path);

			use_wine = false;

			supported = true;
			break;
		} else if os.platform == "windows" {
			use_wine = true;
			supported = true;
		}
	}

	if code_path.is_none() && use_wine {
		code_path = manifest.code_path_windows;
	}

	if !supported || code_path.is_none() {
		return Err(anyhow!("unsupported on platform {}", platform));
	}

	let code_path = code_path.unwrap();
	let args = [
		"-port".to_owned(),
		PORT_BASE.to_string(),
		"-pluginUUID".to_owned(),
		plugin_uuid.clone(),
		"-registerEvent".to_owned(),
		"registerPlugin".to_owned(),
		"-info".to_owned(),
	];

	if code_path.to_lowercase().ends_with(".html") || code_path.to_lowercase().ends_with(".htm") || code_path.to_lowercase().ends_with(".xhtml") {
		let url = format!("http://localhost:{}/", *PORT_BASE + 2) + path.join(code_path).to_str().unwrap();
		let window = tauri::WebviewWindowBuilder::new(APP_HANDLE.get().unwrap(), plugin_uuid.replace('.', "_"), tauri::WebviewUrl::External(url.parse()?))
			.title(manifest.name)
			.visible(false)
			.build()?;

		if fs::exists(path.join("debug")).unwrap_or(false) {
			let _ = window.show();
			window.open_devtools();
		}

		let info = info_param::make_info(plugin_uuid.to_owned(), manifest.version, false).await;
		window.eval(format!(
			r#"const opendeckInit = () => {{
				try {{
					if (document.readyState !== "complete") throw new Error("not ready");
					if (typeof connectOpenActionSocket === "function") connectOpenActionSocket({port}, "{uuid}", "{event}", `{info}`);
					else connectElgatoStreamDeckSocket({port}, "{uuid}", "{event}", `{info}`);
				}} catch (e) {{
					setTimeout(opendeckInit, 10);
				}}
			}};
			opendeckInit();
			"#,
			port = *PORT_BASE,
			uuid = plugin_uuid,
			event = "registerPlugin",
			info = serde_json::to_string(&info)?
		))?;

		INSTANCES.lock().await.insert(plugin_uuid, PluginInstance::Webview);
	} else if code_path.to_lowercase().ends_with(".js") || code_path.to_lowercase().ends_with(".mjs") || code_path.to_lowercase().ends_with(".cjs") {
		// Check for Node.js installation and version in one go.
		let command = if is_flatpak() { "flatpak-spawn" } else { "node" };
		let extra_args = if is_flatpak() { vec!["--host", "node"] } else { vec![] };
		let version_output = Command::new(command).args(&extra_args).arg("--version").output();
		if version_output.is_err() || String::from_utf8(version_output.unwrap().stdout).unwrap().trim() < "v20.0.0" {
			return Err(anyhow!("Node.js version 20.0.0 or higher is required"));
		}

		let info = info_param::make_info(plugin_uuid.to_owned(), manifest.version, true).await;
		let log_file = fs::File::create(log_dir().join("plugins").join(format!("{plugin_uuid}.log")))?;

		spawner_tx
			.send(Box::new(move || {
				let mut command = Command::new(command);
				command
					.current_dir(path)
					.args(extra_args)
					.arg(code_path)
					.args(args)
					.arg(serde_json::to_string(&info)?)
					.stdout(Stdio::from(log_file.try_clone()?))
					.stderr(Stdio::from(log_file));
				#[cfg(target_os = "linux")]
				attach_parent_death_signal(&mut command);
				#[cfg(target_os = "windows")]
				{
					use std::os::windows::process::CommandExt;
					command.creation_flags(0x08000000);
				}
				Ok((plugin_uuid, PluginChildType::Node, command))
			}))
			.map_err(|e| anyhow!(e.to_string()))?;
	} else if use_wine {
		let command = if is_flatpak() { "flatpak-spawn" } else { "wine" };
		let extra_args = if is_flatpak() { vec!["--host", "wine"] } else { vec![] };
		let result = Command::new(command)
			.args(&extra_args)
			.arg("--version")
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.and_then(|mut child| child.wait())
			.map(|status| status.success());
		if !matches!(result, Ok(true)) {
			return Err(anyhow!("failed to detect an installation of Wine"));
		}

		let info = info_param::make_info(plugin_uuid.to_owned(), manifest.version, true).await;
		let log_file = fs::File::create(log_dir().join("plugins").join(format!("{plugin_uuid}.log")))?;

		spawner_tx
			.send(Box::new(move || {
				let mut command = Command::new(command);
				command
					.current_dir(&path)
					.args(extra_args)
					.arg(code_path)
					.args(args)
					.arg(serde_json::to_string(&info)?)
					.stdout(Stdio::from(log_file.try_clone()?))
					.stderr(Stdio::from(log_file));
				if crate::store::get_settings().value.separatewine {
					command.env("WINEPREFIX", path.join("wineprefix").to_string_lossy().to_string());
				} else {
					let _ = fs::remove_dir_all(path.join("wineprefix"));
				}
				#[cfg(target_os = "linux")]
				attach_parent_death_signal(&mut command);
				Ok((plugin_uuid, PluginChildType::Wine, command))
			}))
			.map_err(|e| anyhow!(e.to_string()))?;
	} else {
		let info = info_param::make_info(plugin_uuid.to_owned(), manifest.version, false).await;
		let log_file = fs::File::create(log_dir().join("plugins").join(format!("{plugin_uuid}.log")))?;

		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			fs::set_permissions(path.join(&code_path), fs::Permissions::from_mode(0o755))?;
		}

		spawner_tx
			.send(Box::new(move || {
				let mut command = Command::new(path.join(code_path));
				command
					.current_dir(path)
					.args(args)
					.arg(serde_json::to_string(&info)?)
					.stdout(Stdio::from(log_file.try_clone()?))
					.stderr(Stdio::from(log_file));
				#[cfg(target_os = "linux")]
				attach_parent_death_signal(&mut command);
				#[cfg(target_os = "windows")]
				{
					use std::os::windows::process::CommandExt;
					command.creation_flags(0x08000000);
				}
				Ok((plugin_uuid, PluginChildType::Native, command))
			}))
			.map_err(|e| anyhow!(e.to_string()))?;
	}

	if let Some(applications) = manifest.applications_to_monitor
		&& let Some(applications) = applications.get(platform)
	{
		crate::application_watcher::start_monitoring(&plugin_uuid_2, applications).await;
	}

	Ok(())
}

pub async fn deactivate_plugin(app: &AppHandle, uuid: &str) -> Result<(), anyhow::Error> {
	crate::application_watcher::stop_monitoring(uuid).await;

	if let Some(instance) = INSTANCES.lock().await.remove(uuid) {
		match instance {
			PluginInstance::Webview => {
				if let Some(window) = app.get_webview_window(&uuid.replace('.', "_")) {
					window.close()?;
					tokio::time::sleep(std::time::Duration::from_millis(10)).await;
				}
			}
			PluginInstance::Node(mut child) | PluginInstance::Wine(mut child) | PluginInstance::Native(mut child) => {
				child.kill()?;
				child.wait()?;
			}
		}
		Ok(())
	} else {
		Err(anyhow!("instance of plugin {} not found", uuid))
	}
}

#[cfg(windows)]
pub async fn deactivate_plugins() {
	let uuids = {
		let instances = INSTANCES.lock().await;
		instances.keys().cloned().collect::<Vec<_>>()
	};

	let app = APP_HANDLE.get().unwrap();
	for uuid in uuids {
		let _ = deactivate_plugin(app, &uuid).await;
	}
}

/// Initialise plugins from the plugins directory.
pub fn initialise_plugins() {
	tokio::spawn(init_websocket_server());
	webserver::init_webserver(config_dir());

	let plugin_dir = config_dir().join("plugins");
	let _ = fs::create_dir_all(&plugin_dir);
	let _ = fs::create_dir_all(log_dir().join("plugins"));

	if let Ok(Ok(entries)) = APP_HANDLE.get().unwrap().path().resolve("plugins", tauri::path::BaseDirectory::Resource).map(fs::read_dir) {
		for entry in entries.flatten() {
			if let Err(error) = (|| -> Result<(), anyhow::Error> {
				let builtin_version = semver::Version::parse(&serde_json::from_slice::<manifest::PluginManifest>(&fs::read(entry.path().join("manifest.json"))?)?.version)?;
				let existing_path = plugin_dir.join(entry.file_name());
				if (|| -> Result<(), anyhow::Error> {
					let existing_version = semver::Version::parse(&serde_json::from_slice::<manifest::PluginManifest>(&fs::read(existing_path.join("manifest.json"))?)?.version)?;
					if existing_version < builtin_version {
						Err(anyhow::anyhow!("builtin version is newer than existing version"))
					} else {
						Ok(())
					}
				})()
				.is_err()
				{
					if existing_path.exists() {
						fs::rename(&existing_path, existing_path.with_extension("old"))?;
					}
					if crate::shared::copy_dir(entry.path(), &existing_path).is_err() && existing_path.with_extension("old").exists() {
						fs::rename(existing_path.with_extension("old"), &existing_path)?;
					}
					let _ = fs::remove_dir_all(existing_path.with_extension("old"));
				}
				Ok(())
			})() {
				error!("Failed to upgrade builtin plugin {}: {}", entry.file_name().to_string_lossy(), error);
			}
		}
	}

	let entries = match fs::read_dir(&plugin_dir) {
		Ok(p) => p,
		Err(error) => {
			error!("Failed to read plugins directory at {}: {}", plugin_dir.display(), error);
			panic!()
		}
	};

	let (tx, rx) = mpsc::channel::<SpawnRequest>();
	APP_HANDLE.get().unwrap().manage(tx.clone());

	// Use a dedicated spawner thread so that plugin processes don't die due to PR_SET_PDEATHSIG when the parent Tokio worker exits
	std::thread::spawn(|| {
		for f in rx {
			match f() {
				Ok((plugin_uuid, child_type, mut command)) => match command.spawn() {
					Ok(child) => {
						INSTANCES.blocking_lock().insert(
							plugin_uuid,
							match child_type {
								PluginChildType::Wine => PluginInstance::Wine(child),
								PluginChildType::Native => PluginInstance::Native(child),
								PluginChildType::Node => PluginInstance::Node(child),
							},
						);
					}
					Err(error) => warn!("Failed to initialise plugin {}: {}", plugin_uuid, error),
				},
				Err(error) => warn!("Failed to initialise plugin: {}", error),
			}
		}
	});

	// Iterate through all directory entries in the plugins folder and initialise them as plugins if appropriate
	for entry in entries {
		if let Ok(entry) = entry {
			if entry.file_name().to_string_lossy() == crate::m18::LEGACY_HARDWARE_PLUGIN_ID {
				warn!("Ignoring legacy M18 hardware plugin; hardware support is built in");
				continue;
			}
			let path = match entry.metadata().unwrap().is_symlink() {
				true => entry.path().parent().unwrap_or_else(|| path::Path::new(".")).join(fs::read_link(entry.path()).unwrap()),
				false => entry.path(),
			};
			let metadata = fs::metadata(&path).unwrap();
			if metadata.is_dir() {
				let spawner_tx = tx.clone();
				tokio::spawn(async move {
					if let Err(error) = initialise_plugin(path.clone(), spawner_tx).await {
						warn!("Failed to initialise plugin at {}: {:#}", path.display(), error);
					}
				});
			}
		} else if let Err(error) = entry {
			warn!("Failed to read entry of plugins directory: {}", error)
		}
	}

	// On macOS, hidden WKWebView windows suspend JavaScript after ~7s.
	// Periodically eval a no-op to keep them alive.
	#[cfg(target_os = "macos")]
	tokio::spawn(async {
		use tauri::Manager;
		let app = APP_HANDLE.get().unwrap();
		loop {
			tokio::time::sleep(std::time::Duration::from_secs(3)).await;
			let instances = INSTANCES.lock().await;
			for (uuid, _) in instances.iter().filter(|(_, instance)| matches!(instance, PluginInstance::Webview)) {
				if let Some(window) = app.get_webview_window(&uuid.replace('.', "_")) {
					let _ = window.eval("void(0);");
				}
			}
		}
	});
}

/// Start the WebSocket server that plugins communicate with.
async fn init_websocket_server() {
	let listener = match TcpListener::bind((LOOPBACK, *PORT_BASE)).await {
		Ok(listener) => listener,
		Err(error) => {
			error!("Failed to bind plugin WebSocket server to socket: {}", error);
			return;
		}
	};

	#[cfg(windows)]
	{
		use std::os::windows::io::AsRawSocket;
		use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};

		unsafe { SetHandleInformation(listener.as_raw_socket() as _, HANDLE_FLAG_INHERIT, 0) };
	}

	loop {
		match listener.accept().await {
			// Each connection is handled on its own task: a client that stalls
			// during its handshake must not block every other plugin from connecting.
			Ok((stream, _)) => {
				tokio::spawn(accept_connection(stream));
			}
			Err(error) => {
				// Transient failures (for example, running out of file descriptors)
				// must not permanently stop the plugin server.
				warn!("Failed to accept a plugin connection: {error}");
				tokio::time::sleep(std::time::Duration::from_millis(250)).await;
			}
		}
	}
}

const WEBSOCKET_REGISTRATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Handle incoming data from a WebSocket connection.
async fn accept_connection(stream: TcpStream) {
	use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

	let check_origin = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
		match request.headers().get("origin").map(|origin| origin.to_str().unwrap_or_default()) {
			Some(origin) if !is_trusted_origin(origin) => {
				warn!("Rejected a plugin WebSocket connection from untrusted origin {origin}");
				let mut rejection = ErrorResponse::new(Some("untrusted origin".to_owned()));
				*rejection.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN;
				Err(rejection)
			}
			_ => Ok(response),
		}
	};

	let mut socket = match tokio::time::timeout(WEBSOCKET_REGISTRATION_TIMEOUT, tokio_tungstenite::accept_hdr_async(stream, check_origin)).await {
		Ok(Ok(socket)) => socket,
		Ok(Err(error)) => {
			warn!("Failed to complete WebSocket handshake: {}", error);
			return;
		}
		Err(_) => {
			warn!("A plugin connection did not complete its WebSocket handshake in time");
			return;
		}
	};

	let register_event = match tokio::time::timeout(WEBSOCKET_REGISTRATION_TIMEOUT, socket.next()).await {
		Ok(Some(Ok(message))) => message,
		// The client disconnected or failed before registering; nothing to do.
		Ok(Some(Err(_)) | None) => return,
		Err(_) => {
			warn!("A plugin connection did not register in time");
			return;
		}
	};
	let Ok(text) = register_event.to_text() else { return };
	match serde_json::from_str(text) {
		Ok(event) => crate::events::register_plugin(event, socket).await,
		Err(_) => {
			let _ = crate::events::inbound::process_incoming_message(Ok(register_event), "", false).await;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::is_trusted_origin;

	#[test]
	fn only_local_origins_may_talk_to_the_plugin_servers() {
		for origin in [
			"tauri://localhost",
			"http://tauri.localhost",
			"https://tauri.localhost",
			"http://localhost:57118",
			"http://127.0.0.1:57118",
			"http://[::1]:57118",
			"http://localhost:5173",
		] {
			assert!(is_trusted_origin(origin), "{origin} should be trusted");
		}
		for origin in ["https://example.com", "http://localhost.example.com", "http://127.0.0.1.evil.test", "null", "file://", ""] {
			assert!(!is_trusted_origin(origin), "{origin} should be rejected");
		}
	}
}
