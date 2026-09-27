//! Checks for new releases of this app on GitHub.

use crate::shared::PRODUCT_NAME;

use std::time::Duration;

use serde_json::Value;

/// Releases of this M18 fork, not upstream OpenDeck: upstream builds do not
/// contain the built-in M18 driver.
pub const RELEASES_REPOSITORY: &str = "nickhighland/OpenDeck-VSD-Inside-M18";

#[derive(Clone, serde::Serialize)]
pub struct Release {
	/// The running version.
	pub current: String,
	/// The latest published release.
	pub latest: String,
	/// Whether `latest` is newer than `current`.
	pub newer: bool,
	/// The release's page, where its downloads are.
	pub url: String,
	pub notes: String,
}

/// Release tags look like "tv2.15.0" (see publish.yml); "v2.15.0" is accepted too.
fn tag_version(tag: &str) -> Result<semver::Version, semver::Error> {
	semver::Version::parse(tag.trim_start_matches("tv").trim_start_matches('v'))
}

pub async fn latest_release() -> Result<Release, anyhow::Error> {
	let release = reqwest::Client::new()
		.get(format!("https://api.github.com/repos/{RELEASES_REPOSITORY}/releases/latest"))
		.header("Accept", "application/vnd.github+json")
		.header("User-Agent", "OpenDeck-VSD-M18")
		.timeout(Duration::from_secs(20))
		.send()
		.await?
		.error_for_status()?
		.json::<Value>()
		.await?;
	let tag = release.get("tag_name").and_then(Value::as_str).ok_or_else(|| anyhow::anyhow!("the latest release has no tag"))?;
	let latest = tag_version(tag)?;
	let current = semver::Version::parse(crate::built_info::PKG_VERSION)?;
	Ok(Release {
		newer: current < latest,
		current: current.to_string(),
		latest: latest.to_string(),
		url: release
			.get("html_url")
			.and_then(Value::as_str)
			.map_or_else(|| format!("https://github.com/{RELEASES_REPOSITORY}/releases/latest"), str::to_owned),
		notes: release.get("body").and_then(Value::as_str).unwrap_or_default().trim().to_owned(),
	})
}

/// At startup: when a newer release exists, offer to open its download page.
pub async fn notify_if_newer() -> Result<(), anyhow::Error> {
	let release = latest_release().await?;
	if !release.newer {
		return Ok(());
	}
	let app = crate::APP_HANDLE.get().ok_or_else(|| anyhow::anyhow!("the application is not initialised"))?;
	use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
	let url = release.url.clone();
	app.dialog()
		.message(format!("{PRODUCT_NAME} {} is available. You have {}.\n\n{}", release.latest, release.current, release.notes))
		.title(format!("{PRODUCT_NAME} update available"))
		.buttons(MessageDialogButtons::OkCancelCustom("Download".to_owned(), "Later".to_owned()))
		.show(move |download| {
			if download && let Err(error) = open::that_detached(&url) {
				log::warn!("Failed to open the release page: {error}");
			}
		});
	Ok(())
}

/// Settings' "Check now" button.
#[tauri::command]
pub async fn check_for_updates() -> Result<Release, String> {
	latest_release().await.map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
mod tests {
	#[test]
	fn release_tags_of_this_repository_parse() {
		assert_eq!(super::tag_version("tv2.15.3").unwrap(), semver::Version::new(2, 15, 3));
		assert_eq!(super::tag_version("v2.16.0").unwrap(), semver::Version::new(2, 16, 0));
		assert!(super::tag_version("latest").is_err());
	}
}
