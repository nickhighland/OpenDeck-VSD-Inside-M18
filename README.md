# OpenDeck VSD M18

OpenDeck VSD M18 is an M18-only desktop controller built from [OpenDeck](https://github.com/nekename/OpenDeck). It keeps OpenDeck's action, plugin, profile, multi-action, toggle-action, application-switching, and background-operation model, but the user-facing hardware layout is dedicated to the VSD Inside M18.

## M18 layout

The editor shows the physical device directly:

```text
[ LCD 1 ][ LCD 2 ][ LCD 3 ][ LCD 4 ][ LCD 5 ]
[ LCD 6 ][ LCD 7 ][ LCD 8 ][ LCD 9 ][ LCD 10]
[LCD 11 ][LCD 12 ][LCD 13 ][LCD 14 ][LCD 15]
             [Bottom 1] [Bottom 2] [Bottom 3]
```

The bottom buttons are real assignable controls, but they have no LCD. Their assignments can use action titles and behavior without causing image writes to nonexistent displays. The fork does not expose the two unused positions that a generic OpenDeck 4×5 profile can contain. Internal position numbering is retained only so the OpenDeck action/plugin protocol and imported profiles remain compatible.

The VSD Inside M18 hardware driver is built into the application. It is not installed, listed, spawned, or managed as an OpenDeck plugin. The core driver talks directly to VSD Inside M18 (`5548:1000`) for key press/release events, brightness, reconnects, and sleep/wake handling. OpenDeck's remaining plugin system is reserved for actions and user extensions.

The M18 screen follows the computer's state: it stays on while the computer is in use, turns off after a configurable period without keyboard, mouse, or trackpad input (and, optionally, while the computer is locked), and turns back on as soon as the computer is used again. Pressing an M18 key while the screen is off only wakes it and does not run the key's action. There is no device screensaver: the M18's keys are windows onto a single LCD panel, which is not prone to OLED-style burn-in, so turning the backlight off is the effective way to protect it.

**Settings › M18 Display** sets the color and brightness of the M18's 24 RGB LEDs, which go dark while the M18 sleeps. The built-in **M18 LED Colors** action sets individual LED colors for the page it is on; it does not depend on a hardware plugin process.

A **Run Command** key, and any Hotkey Switch entry set to **Command**, runs a shell command in the background. Unlike keystrokes, which macOS sends to the lock screen, commands also work while the Mac is locked; for example, `m1ddc set input 17` switches a monitor to HDMI 1. Homebrew's folders are on the command's `PATH`.

Native M18 actions are grouped into Apps & Hotkeys, Device Controls, Page Navigation, and System Controls. Search covers action names, localized labels, tooltips, identifiers, plugin IDs, and category names.

## VSD Craft migration

Use **Settings → Import VSD Craft** and select a VSD Craft `manifest.json`. The importer follows nested page profiles and creates native M18 profiles with titles, images, hotkeys, toggle hotkeys, applications, URLs, page navigation, brightness, media/system controls, and supported VSD actions.

Unsupported action UUIDs are reported and retained with their original settings, title, and image as clearly labeled inactive placeholders; they will not execute until implemented. Dynamic VSD Craft widgets such as weather, calendars, timers, memo storage, and animated visualizers still need individual native action ports for full one-for-one parity.

See [docs/VSD-M18-STATUS.md](docs/VSD-M18-STATUS.md) for the compatibility boundary and verification notes.

## macOS background operation

The app defaults to background/menu-bar operation. The supplied [LaunchAgent template](macos/com.opendeck.vsd-m18.plist) can keep the app running and restart it after an abnormal exit. It intentionally uses `KeepAlive=true`, so a deliberate quit also reopens it until the job is unloaded.

```sh
cp "releases/OpenDeck VSD M18.app" "/Applications/OpenDeck VSD M18.app"
cp macos/com.opendeck.vsd-m18.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.opendeck.vsd-m18.plist
```

Releases are signed with a Developer ID certificate and notarized when the repository's signing secrets are set; see [docs/MACOS-SIGNING.md](docs/MACOS-SIGNING.md). Ad-hoc signed builds may need right-clicking the app and choosing **Open** on first launch.

Hotkeys, typed text, and media keys need the **Accessibility** permission (System Settings › Privacy & Security › Accessibility). macOS ties it to the app's signature: after installing an ad-hoc signed build, or the first Developer ID build, remove OpenDeck VSD M18 from the Accessibility list with **−** and add it again, even if it still appears switched on. The app explains this when macOS refuses a key, and **Settings › General** shows whether keystrokes are allowed.

## Build and verification

From the repository root:

```sh
deno task check
cargo test --manifest-path src-tauri/Cargo.toml
deno task tauri build
```

The repository includes the built-in M18 HID driver and the VSD Craft importer. The fork does not scan for or initialize Elgato Stream Deck hardware, and it does not migrate old generic OpenDeck/Stream Deck layout files into its M18 configuration directory.

## GitHub Actions on Unraid

The `Unraid verification` workflow uses a dedicated repository-level Unraid runner for trusted pushes to `main` and manual runs on `main`. It runs Rust formatting/tests and frontend checks/builds. It intentionally has no `pull_request` trigger: this is a public repository, and untrusted pull-request code must not run on a persistent runner with access to the Unraid host. Release packages target Apple silicon Macs and Windows x64, using GitHub-hosted runners. See [docs/UNRAID-CI.md](docs/UNRAID-CI.md) for runner safety and [docs/SETUP-RECOVERY.md](docs/SETUP-RECOVERY.md) for setup and recovery instructions.

## License and upstream sources

The project remains GPL-3.0-or-later, following OpenDeck and the M18 protocol implementation used by this fork.

- OpenDeck: https://github.com/nekename/OpenDeck
- M18 protocol reference: https://github.com/ibanks42/opendeck-m18
