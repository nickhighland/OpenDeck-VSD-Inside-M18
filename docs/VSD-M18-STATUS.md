# OpenDeck VSD M18

## What this fork is

OpenDeck VSD M18 is an M18-only fork of OpenDeck. It keeps OpenDeck's native profile, action-plugin, multi-action, toggle-action, device-switching, and background operation model, while implementing M18 hardware support directly in the Rust application and including a VSD Craft profile importer. Generic Stream Deck hardware discovery and layout presentation are not part of this fork.

The name is deliberately explicit: **OpenDeck VSD M18**.

## M18 hardware support

M18 hardware support is built into the application. There is no M18 hardware plugin to install or manage.

- 15 LCD keys are supported.
- The three physical bottom keys are supported as positions 16, 17, and 18.
- JPEG button images are sent at 64×64 pixels with the M18 rotation/mirroring protocol.
- Key presses, release events, device brightness, reconnects, and device sleep/wake are handled by the built-in Rust driver.
- LED color palettes are exposed through the built-in **M18 LED Colors** action and applied directly by the core driver.
- The editor shows three rows of five LCD keys and a separate row of three physical bottom buttons. The bottom buttons are assignable but never receive image writes.
- OpenDeck's 4×5 position numbering remains internal only, so existing actions, plugins, and imported profiles can address positions 0–17 without exposing positions 18–19.

## VSD Craft importer

The importer is available from **Settings → Import VSD Craft**. It reads a VSD Craft `manifest.json`, follows its nested page profiles, and writes native OpenDeck profiles for the M18 serial number.

Translated VSD actions include:

- ordinary hotkeys and native Super Hotkeys;
- native HotkeySwitch states;
- native OpenApps and Open actions; OpenApps resolves and displays the selected macOS app icon;
- Website actions, including VSD Craft's `path` setting;
- native previous, next, and goto page actions using the M18 page set, plus change page actions;
- Scene Shift, Open Folder, and parent-profile navigation;
- device brightness and device sleep;
- volume, mute, media transport, Siri, Launchpad, Mission Control (VSD Craft's “Dispatch Center”), screenshots, sleep, and the macOS screen-saver action;
- imported button titles, state images, and current state selection.

Unknown action UUIDs are reported in the import summary and retained in place with their title and image. This makes migration recoverable instead of silently losing buttons.

## Feature-parity boundary

OpenDeck's existing features remain the foundation and are not replaced by the importer. The M18 protocol, computer-state display sleep, and common M18 actions are implemented. Full one-for-one parity with every VSD Craft plugin is a larger porting project: VSD Craft also ships dynamic plugins for weather, calendars, world time, timers, countdowns, memo storage, emoticons, paint effects, and animated visualizers. Those require native OpenDeck action implementations rather than a simple profile translation.

The detailed read-only audit of VSD Craft's 123 action UUIDs and the fork's feature parity is in [docs/VSD-CRAFT-PARITY.md](VSD-CRAFT-PARITY.md). It contains no exported personal profiles.

The right technical path is incremental: port each remaining VSD action UUID to a native OpenDeck action, add a fixture profile and runtime test, then remove that UUID from the importer's unsupported list. The hardware layer does not need to be redesigned for those ports.

## M18-only boundary

The fork intentionally starts with a fresh application configuration namespace and does not copy old OpenDeck profile files. It does not initialize Elgato hardware or expose the generic Elgato plugin catalogue. OpenDeck's plugin protocol is retained for action plugins and extensions. Hardware discovery, HID input, LCD output, LED output, and M18 reconnect handling belong to the application core.

## Build and verification

From the repository root:

```sh
deno task check
cargo test --manifest-path src-tauri/Cargo.toml
deno task tauri build
```

The release build produced an Apple Silicon macOS app and DMG. The bundle contains no M18 hardware plugin binary; the driver is part of the application binary. The app is ad-hoc/unsigned for distribution purposes, so macOS may require **right-click → Open** on first launch; notarization requires the user's Apple Developer credentials.

For a crash-restarting menu-bar installation, copy the app to `/Applications/OpenDeck VSD M18.app`, copy `macos/com.opendeck.vsd-m18.plist` to `~/Library/LaunchAgents/`, and load it with:

```sh
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.opendeck.vsd-m18.plist
```

The supplied plist intentionally uses `KeepAlive=true`, so quitting the app also causes it to reopen. Remove the job with `launchctl bootout` if you need a normal quit while testing.

## Upstream sources

- OpenDeck: https://github.com/nekename/OpenDeck
- M18 protocol reference: https://github.com/ibanks42/opendeck-m18
