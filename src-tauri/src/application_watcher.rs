use crate::store::{NotProfile, Store};

use std::collections::HashMap;
use std::sync::LazyLock;

use active_win_pos_rs::get_active_window;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::Emitter;
use tokio::sync::RwLock;

pub type ApplicationProfiles = HashMap<String, HashMap<String, String>>;
impl NotProfile for ApplicationProfiles {}

pub static APPLICATIONS: RwLock<Vec<String>> = RwLock::const_new(Vec::new());
pub static APPLICATION_PROFILES: LazyLock<RwLock<Store<ApplicationProfiles>>> = LazyLock::new(|| RwLock::new(Store::new("applications", &crate::shared::config_dir(), HashMap::new()).unwrap()));

pub static APPLICATION_PROCESSES: LazyLock<RwLock<HashMap<String, Vec<u32>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));
pub static APPLICATION_PLUGINS: LazyLock<RwLock<HashMap<String, Vec<String>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Clone, serde::Serialize)]
pub struct SwitchProfileEvent {
	device: String,
	profile: String,
}

pub fn init_application_watcher() {
	tokio::spawn(async move {
		let mut previous = String::new();
		let app_handle = crate::APP_HANDLE.get().unwrap();
		loop {
			let app_name = if let Ok(win) = get_active_window() {
				let mut applications = APPLICATIONS.write().await;
				if !applications.contains(&win.app_name) && !win.app_name.to_lowercase().starts_with(&crate::shared::PRODUCT_NAME.to_lowercase()) && !win.app_name.trim().is_empty() {
					applications.push(win.app_name.clone());
					let _ = app_handle.emit("applications", applications.clone());
				}
				win.app_name
			} else {
				String::new()
			};

			if app_name != previous {
				// Collect the switches first: no registry or settings guard may be
				// held while a page switch awaits the profile locks.
				let switches = {
					let application_profiles = &APPLICATION_PROFILES.read().await.value;
					let application = application_profiles.get(&app_name);
					let default = application_profiles.get("opendeck_default");
					let devices = crate::shared::DEVICES.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
					devices
						.into_iter()
						.filter_map(|device| {
							let profile = application.and_then(|d| d.get(&device)).or(default.and_then(|d| d.get(&device)))?.clone();
							Some((device, profile))
						})
						.collect::<Vec<_>>()
				};
				for (device, profile) in switches {
					if crate::store::profiles::DEVICE_STORES.write().await.get_selected_profile(&device).ok().as_ref() == Some(&profile) {
						continue;
					}
					if crate::m18::is_m18(&device) {
						// Switch M18 pages in the core, so automatic switching works
						// whether or not the editor window is open.
						if let Err(error) = crate::m18_pages::switch_to_profile(&device, &profile).await {
							log::warn!("Failed to switch {device} to the page for {app_name}: {error:#}");
						}
						continue;
					}
					let _ = app_handle.emit("switch_profile", SwitchProfileEvent { device, profile });
				}
				previous = app_name;
			}

			tokio::time::sleep(std::time::Duration::from_millis(250)).await;
		}
	});

	tokio::spawn(async move {
		let mut system = System::new_with_specifics(RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().without_tasks()));

		loop {
			if !APPLICATION_PLUGINS.read().await.is_empty() {
				system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().without_tasks());

				for (application, processes) in APPLICATION_PROCESSES.write().await.iter_mut() {
					let mut alive_processes = Vec::with_capacity(processes.len());
					for pid in processes.iter() {
						if system.process(Pid::from_u32(*pid)).is_some() {
							alive_processes.push(*pid);
						} else {
							for plugin in APPLICATION_PLUGINS.read().await.get(application).into_iter().flatten() {
								let _ = crate::events::outbound::applications::application_did_terminate(plugin, application.clone()).await;
							}
						}
					}
					*processes = alive_processes;
				}
			}

			let application_plugins = APPLICATION_PLUGINS.read().await;
			for (application, plugins) in application_plugins.iter() {
				for process in system.processes_by_exact_name(application.as_ref()) {
					let pid = process.pid().as_u32();
					let mut application_processes = APPLICATION_PROCESSES.write().await;
					let pids = application_processes.entry(application.clone()).or_default();
					if !pids.contains(&pid) {
						pids.push(pid);
						for plugin in plugins {
							let _ = crate::events::outbound::applications::application_did_launch(plugin, application.clone()).await;
						}
					}
				}
			}
			drop(application_plugins);

			tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		}
	});
}

/// Remove automatic-switching rules that point at a deleted profile.
pub async fn forget_profile(device: &str, profile: &str) {
	let mut store = APPLICATION_PROFILES.write().await;
	let mut changed = false;
	for devices in store.value.values_mut() {
		if devices.get(device).is_some_and(|mapped| mapped == profile) {
			devices.remove(device);
			changed = true;
		}
	}
	if changed {
		store.value.retain(|_, devices| !devices.is_empty());
		if let Err(error) = store.save() {
			log::warn!("Failed to save application switching rules: {error:#}");
		}
	}
}

pub async fn start_monitoring(plugin: &str, applications: &Vec<String>) {
	let mut application_plugins = APPLICATION_PLUGINS.write().await;

	for application in applications {
		application_plugins.entry(application.to_owned()).or_default().push(plugin.to_owned());

		let application_processes = APPLICATION_PROCESSES.read().await;
		if let Some(pids) = application_processes.get(application) {
			for _ in pids {
				let _ = crate::events::outbound::applications::application_did_launch(plugin, application.to_owned()).await;
			}
		}
	}
}

pub async fn stop_monitoring(plugin: &str) {
	let mut application_plugins = APPLICATION_PLUGINS.write().await;
	for plugins in application_plugins.values_mut() {
		plugins.retain(|p| p != plugin);
	}
	application_plugins.retain(|_, p| !p.is_empty());
}
