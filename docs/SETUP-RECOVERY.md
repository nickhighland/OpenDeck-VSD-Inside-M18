# Setup, backup, and recovery

## Back up and migrate profiles

Before importing VSD Craft data or restoring an older configuration, open **Settings → Backup config** and save the ZIP somewhere outside the application's config folder. A backup contains profiles, settings, custom artwork, and user-installed plugin data; bundled plugins are supplied by the application and are not copied into the archive.

To migrate from VSD Craft, choose **Settings → Import VSD Craft** and select the source `manifest.json`. The importer follows nested page profiles and keeps unsupported actions in place with their title and image where possible. Review the import summary for unsupported action UUIDs. Importing creates profiles; it does not overwrite the VSD Craft source. Back up the fork first so you can return to its prior state.

To restore, use **Settings → Restore config** and select a ZIP made by the application's backup command. Restore replaces the current configuration and restarts the app. Make a fresh backup first: restoration replaces the app's existing settings and profiles.

The exact config and log directories can be opened from **Settings → Open config** and **Settings → Open logs**. If the app cannot open, typical config locations are:

- macOS: `~/Library/Application Support/com.nickhighland.opendeck-vsd-m18`
- Linux: `~/.config/com.nickhighland.opendeck-vsd-m18`

Do not copy profile JSON alone when profiles use custom images; preserve the complete config directory or use the ZIP backup/restore controls.

## macOS background launch and crash recovery

The supplied LaunchAgent starts the app hidden at login and uses `KeepAlive=true`, so launchd restarts it after a crash **and** after a deliberate quit. Install the app at `/Applications/OpenDeck VSD M18.app`, then run:

```sh
mkdir -p ~/Library/LaunchAgents
cp macos/com.opendeck.vsd-m18.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.opendeck.vsd-m18.plist
launchctl print gui/$(id -u)/com.opendeck.vsd-m18
```

If the app is installed elsewhere, update the absolute executable path in the plist first. To stop it and prevent relaunch while troubleshooting:

```sh
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.opendeck.vsd-m18.plist
```

The LaunchAgent writes stdout/stderr to `/tmp/opendeck-vsd-m18.log` and `/tmp/opendeck-vsd-m18-error.log`; application logs are also available through **Settings → Open logs**. Only one application should own the M18 at a time: quit VSD Craft before starting this fork, and unload this LaunchAgent before switching back.

## Linux desktop device access

Debian and RPM packages install the M18 udev rule automatically. For another package format or manual installation, copy `src-tauri/bundle/40-opendeck-m18.rules` to `/etc/udev/rules.d/40-opendeck-m18.rules`, then reload rules and reconnect the device:

```sh
sudo install -m 0644 src-tauri/bundle/40-opendeck-m18.rules /etc/udev/rules.d/40-opendeck-m18.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
```

If the device is still missing, confirm that the OS sees the USB device, reconnect it, and inspect the application log. Do not run the desktop app as root to work around a missing udev permission rule.

## Unraid Actions runner recovery

The repository's Unraid runner is CI infrastructure, not a place to run the M18 desktop app. In **Repository → Settings → Actions → Runners**, confirm the repository-level runner is online and has the `unraid` label. The verification workflow is limited to trusted pushes to `main` and manual runs on `main`; pull requests intentionally do not use the persistent Unraid runner.

If a workflow is queued, check that the runner is online and that its labels match `self-hosted`, `linux`, `x64`, and `unraid`. If the container cannot reconnect, inspect its Unraid logs and confirm its persistent runner-files mount is intact. A newly configured runner needs a fresh, short-lived repository registration token; do not put that token, a personal access token, Docker socket mount, or container environment dump in Git. Preserve the persistent runner data when restarting the container so it can reuse its registration.
