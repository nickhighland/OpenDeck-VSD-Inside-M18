use super::Error;

use crate::built_info;
use crate::shared::{PRODUCT_NAME, config_dir};

use std::fs::{File, read_dir};
use std::io::Write;
use std::path::{Path, PathBuf};

use path_slash::PathExt;
use tauri::{AppHandle, Manager, command};
#[cfg(not(debug_assertions))]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, FilePath};
use zip::{ZipWriter, write::FileOptions};

#[command]
pub async fn get_settings() -> crate::store::Settings {
	crate::store::get_settings().value
}

#[command]
pub async fn set_settings(_app: AppHandle, settings: crate::store::Settings) -> Result<(), Error> {
	let mut store = crate::store::SETTINGS_MUT.lock().await;
	if store.value == settings {
		// The editor re-sends unchanged settings (for example when it starts);
		// skip the disk write and the USB traffic.
		crate::store::remember_settings(&settings);
		return Ok(());
	}
	let previous = store.value.clone();

	#[cfg(not(debug_assertions))]
	if previous.autolaunch != settings.autolaunch {
		let _ = match settings.autolaunch {
			true => _app.autolaunch().enable(),
			false => _app.autolaunch().disable(),
		};
	}

	// Applying a setting to hardware must never stop it from being saved: a
	// device that is reconnecting at this moment picks the value up on connect.
	if previous.brightness != settings.brightness
		&& let Err(error) = crate::events::outbound::devices::set_brightness(settings.brightness).await
	{
		log::warn!("Failed to apply M18 brightness: {error:#}");
	}
	// Saved key images are drawn at the old rotation and icon size; the
	// editor draws them again.
	if previous.rotation != settings.rotation || previous.app_icon_scale != settings.app_icon_scale {
		crate::key_images::forget_all().await;
	}
	if previous.led_brightness != settings.led_brightness
		&& let Err(error) = crate::events::outbound::devices::set_led_brightness(settings.led_brightness).await
	{
		log::warn!("Failed to apply M18 LED brightness: {error:#}");
	}
	if previous.led_color != settings.led_color
		&& let Err(error) = crate::events::outbound::devices::show_settings_leds().await
	{
		log::warn!("Failed to apply M18 LED color: {error:#}");
	}
	if previous.sleep_timeout_minutes != settings.sleep_timeout_minutes
		&& let Err(error) = crate::device_sleep::update_sleep_timeout_minutes(settings.sleep_timeout_minutes).await
	{
		log::warn!("Failed to apply the M18 sleep timeout: {error:#}");
	}
	if previous.sleep_when_computer_locked != settings.sleep_when_computer_locked
		&& let Err(error) = crate::device_sleep::update_sleep_when_computer_locked(settings.sleep_when_computer_locked).await
	{
		log::warn!("Failed to apply the lock-screen sleep setting: {error:#}");
	}

	store.value = settings;
	store.save()?;
	crate::store::remember_settings(&store.value);
	Ok(())
}

#[command]
pub fn open_config_directory() -> Result<(), Error> {
	if let Err(error) = open::that_detached(config_dir()) {
		return Err(anyhow::Error::from(error).into());
	}
	Ok(())
}

#[command]
pub fn open_log_directory() -> Result<(), Error> {
	if let Err(error) = open::that_detached(crate::shared::log_dir()) {
		return Err(anyhow::Error::from(error).into());
	}
	Ok(())
}

#[command]
pub fn get_build_info() -> String {
	format!(
		r#"
		<details>
			<summary> {} v{} ({}) on {} </summary>
			{}
		</details>
		"#,
		PRODUCT_NAME,
		built_info::PKG_VERSION,
		built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("commit hash unknown"),
		built_info::TARGET,
		built_info::DIRECT_DEPENDENCIES_STR
	)
}

fn add_dir_to_zip<W: Write + std::io::Seek>(zip: &mut ZipWriter<W>, base_dir: &Path, current_dir: &Path, options: FileOptions<()>, skip_paths: &[PathBuf]) -> std::io::Result<()> {
	for entry in read_dir(current_dir)? {
		let entry = entry?;
		let path = entry.path();
		if skip_paths.contains(&path) {
			continue;
		}
		let relative = path.strip_prefix(base_dir).unwrap();

		if path.is_dir() {
			zip.add_directory(relative.to_slash_lossy(), options)?;
			add_dir_to_zip(zip, base_dir, &path, options, skip_paths)?;
		} else {
			zip.start_file(relative.to_slash_lossy(), options)?;
			let mut file = File::open(&path)?;
			std::io::copy(&mut file, zip)?;
		}
	}
	Ok(())
}

#[command]
pub async fn backup_config_directory(app: AppHandle) -> Result<bool, Error> {
	let filename = format!(
		"{}_config_{}_{}_{}.zip",
		PRODUCT_NAME,
		std::env::consts::OS,
		std::env::consts::ARCH.replace("_", "-"),
		chrono::Local::now().format("%Y%m%d")
	);

	let path = app
		.dialog()
		.file()
		.set_file_name(filename)
		.add_filter(format!("{} config backup", PRODUCT_NAME), &["zip"])
		.blocking_save_file();

	let Some(FilePath::Path(path)) = path else {
		return Ok(false);
	};
	let _ = std::fs::remove_file(&path);

	let temp_path = path.with_extension("zip.part");
	let file = File::create(&temp_path)?;

	let mut skip_paths = vec![temp_path.clone()];
	let config_dir = config_dir();

	if let Some(builtin_plugins) = app
		.path()
		.resolve("plugins", tauri::path::BaseDirectory::Resource)
		.ok()
		.and_then(|p| read_dir(p).ok())
		.map(|e| e.into_iter().flatten().map(|x| config_dir.join("plugins").join(x.file_name())).collect::<Vec<_>>())
	{
		skip_paths.extend(builtin_plugins);
	};

	let mut zip = ZipWriter::new(file);
	let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
	add_dir_to_zip(&mut zip, &config_dir, &config_dir, options, &skip_paths)?;
	zip.finish().map_err(anyhow::Error::from)?;

	std::fs::rename(temp_path, path)?;

	Ok(true)
}

#[command]
pub async fn restore_config_directory(app: AppHandle) -> Result<(), Error> {
	let path = app.dialog().file().add_filter(format!("{} config backup", PRODUCT_NAME), &["zip"]).blocking_pick_file();

	let Some(FilePath::Path(path)) = path else {
		return Ok(());
	};

	let config_dir = config_dir();
	let temp_dir = config_dir.with_extension("temp");
	let backup_dir = config_dir.with_extension("bak");
	let _ = std::fs::remove_dir_all(&temp_dir);
	let _ = std::fs::remove_dir_all(&backup_dir);

	crate::zip_extract::extract(File::open(path)?, &temp_dir).map_err(anyhow::Error::from)?;
	// Refuse archives that are not configuration backups instead of replacing
	// the whole configuration with arbitrary files.
	if !temp_dir.join("settings.json").is_file() && !temp_dir.join("profiles").is_dir() {
		let _ = std::fs::remove_dir_all(&temp_dir);
		return Err(anyhow::anyhow!("This ZIP file is not a {PRODUCT_NAME} configuration backup; nothing was changed.").into());
	}

	// Write pending edits into the configuration being replaced, then stop all
	// profile writes so nothing in memory is flushed over the restored files
	// before the restart.
	if let Err(error) = crate::store::profiles::flush_stale_profiles().await {
		log::warn!("Failed to flush profiles before restoring a backup: {error:#}");
	}
	crate::store::profiles::freeze_profile_writes();

	#[cfg(windows)]
	crate::plugins::deactivate_plugins().await;
	std::fs::rename(&config_dir, &backup_dir)?;
	if let Err(error) = std::fs::rename(&temp_dir, &config_dir) {
		// Put the previous configuration back rather than leaving none at all.
		let _ = std::fs::rename(&backup_dir, &config_dir);
		return Err(anyhow::Error::from(error).into());
	}
	let _ = std::fs::remove_dir_all(backup_dir);
	crate::key_images::forget_all().await;

	app.restart();
}
