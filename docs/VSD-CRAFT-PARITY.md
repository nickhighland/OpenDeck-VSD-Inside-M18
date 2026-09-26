# VSD Craft feature and parity audit

Audit date: 2026-09-25; live migration update: 2026-09-26
VSD Craft version: `3.10.205.0918`
Connected device: VSD Inside M18 (serial omitted)
Fork: OpenDeck VSD M18

## Executive result

The M18 hardware replacement is viable, but the fork does **not** yet have one-for-one VSD Craft feature parity.

The audit found:

- VSD Craft ships 123 distinct action UUIDs across its bundled and default action packages.
- The fork currently exposes the OpenDeck action catalog plus 12 M18-native actions in the application core: OpenApps, Super Hotkeys, HotkeySwitch, Super Hotkey Switch, volume down/up, mute, Siri, native previous/next/goto page controls, and M18 LED Colors. Super Hotkey and Super Hotkey Switch still use software input; this is **not** VSD Craft Super Hotkey parity.
- The VSD importer recognizes 62 VSD action UUIDs and translates them to generic input, command, URL, profile-switch, toggle, or brightness actions. That is migration coverage, not full behavioral parity.
- 61 VSD action UUIDs have no importer translation.
- The importer supports nested M18 pages and maps supported actions to native M18/OpenDeck actions. Private profile exports and selected-app lists are intentionally excluded from this public audit.

The strongest parity area is the hardware layer: the fork drives the M18 directly from the application core, including its 15 LCD keys, three physical bottom buttons, LED output, reconnect handling, and sleep/wake. The computer-idle screensaver and wake-only first press are implemented but still need physical-device verification, including full-display coverage. The largest parity gap is the action catalog and its property inspectors.

## Evidence and method

The audit used three independent sources:

1. Live inspection of the installed VSD Craft window while an M18 was connected. This covered device selection, pages, scene controls, the settings window, action categories, search, and a representative action property inspector.
2. Read-only inspection of VSD Craft's installed manifests, bundled plugin directories, profile manifests, images, videos, logs, preferences, and `DataCache.db`.
3. Read-only inspection of the fork source, especially `src-tauri/src/m18.rs`, `src-tauri/src/vsd_import.rs`, the profile manager, the action catalog, and the screensaver components.

Actions were not executed when doing so would launch applications, send keystrokes, change system settings, or modify the user's profile. For those actions, the installed manifest and property-inspector definitions were used as the feature evidence.

## Application-level feature matrix

| VSD Craft feature | What was verified | Fork status |
|---|---|---|
| Menu-bar/background operation | The app bundle sets `LSUIElement=true`; the live Settings window has checked `Power on` and `Minimize` options. | **Partial.** The fork has autolaunch/background settings, but not the same VSD Craft menu-bar/settings surface. |
| Device discovery | Live device selector showed a VSD Inside M18; the device dropdown showed the connected M18. `Automatically Detect and Prompt` is present in VSD Craft settings. | **Matched for M18.** The fork has direct M18 discovery and registration in the core. Its hardware support is not an installable hardware plugin. |
| M18 LCD layout | Live editor shows a 3 × 5 grid of 15 LCD keys. | **Matched.** The fork renders the same 15 LCD positions. |
| Three bottom physical buttons | Live editor shows three separate circular buttons below the LCD grid; the profile contains positions `5,0`, `5,1`, and `5,2`. | **Matched.** The fork maps the bottom buttons to logical positions 15–17, accepts actions on them, and does not attempt LCD image writes for them. |
| Device image output | VSD Craft displays per-key PNG/GIF/JPG artwork and custom titles. | **Partial.** The fork renders still images and converts the rendered result to M18 JPEG output. Animated images are not continuously animated on the device; MP4/MBG per-key media is not supported. |
| OpenApps artwork | VSD Craft's OpenApps action supplies an icon for the selected application. | **Matched for app selections.** Native OpenApps resolves the selected app path, bundle identifier, or name and uses the macOS application icon in the editor and on the M18. |
| Device brightness | VSD Craft exposes `Brightness` and a `devsleep` action. | **Partial.** The fork has global brightness settings and a `Device Brightness` action. Sleep is a core setting/behavior rather than an exact VSD action. |
| M18 LEDs | VSD Craft's M18 profile does not use an explicit LED action; its bundle also contains K1 Pro LED actions for another device family. | **Matched/enhanced for M18.** The fork has a built-in `M18 LED Colors` action and core LED output. |
| Reconnect/keepalive | VSD Craft maintains a connected device session, but the user reports frequent application crashes. | **Matched at the hardware boundary.** The fork's HID session, keepalive, reconnect, and sleep/wake path are in Rust core code, outside a hardware plugin process. Stability still needs long-running real-device testing. |
| Scenes | VSD Craft has `Create Scene`, remove, rename, backup, import, export, and a default-scene checkbox. | **Partial.** The fork has profiles, folders, create/delete/rename/duplicate, backup/restore, and application-profile selection, but not the same scene editor or scene import/export format. |
| Pages | VSD Craft visibly has page buttons `1`, `2`, and `+`; the live device switched between two page layouts. | **Matched for the M18 workflow.** The importer creates a native M18 page set, the editor exposes its page buttons, and previous/next/goto actions switch pages without a hardware plugin. |
| App-based automatic switching | VSD Craft Scenes settings contain three application selectors and a “following app foreground, auto-switch to scene” option. | **Partial/matched conceptually.** The fork has an application watcher and per-application profile mappings. The VSD three-slot scene UI is not replicated. |
| Action catalog | VSD Craft has category groups, drag-to-key actions, a search field, and a large bundled catalog. | **Partial.** The fork has the OpenDeck action catalog and drag/drop model, plus 12 M18-native actions. Its action-list search is local to the fork catalog. |
| Action property inspector | VSD Craft's selected OpenApps key showed delete, title entry, title styling, app selection, app-list reload, and icon customization. | **Partial.** The fork has the OpenDeck property-inspector model and inspectors for its current actions, but not the VSD Craft-specific title/icon/app editor for every VSD action. |
| Multi-action composition | VSD Craft includes Multi Action, Action Carousel, Action Cycle, Delay, and knob variants. | **Partial.** The fork has built-in Multi Action and Toggle Action. Carousel, delay, and knob action-group variants are missing. |
| VSD Craft profile import | VSD profiles contain nested page profiles, per-position actions, state images/titles, and device metadata. | **Matched for supported migration data.** The importer follows nested pages, maps supported actions, preserves state artwork/titles, resolves OpenApps icons, and reports unsupported UUIDs for future profiles. Private profile contents are not included here. |
| Per-key GIF/video artwork | The installed VSD Craft profile library contains PNG, GIF, MBG, JPG, and MP4 assets. | **Partial.** GIFs can be decoded for the UI's image path, but the M18 receives a rendered JPEG frame rather than an animated stream. MP4/MBG is not a supported per-key image format. |
| VSD Craft screensaver actions | The bundle includes `Screensaver 1` and `Screensaver 2` action plugins. | **Partial, not hardware-verified.** The fork has video/photo input, macOS-wide idle detection, LED-preserving output, and first-press wake suppression. Full 480×272 background streaming is implemented, but coverage, alignment, playback cadence, and wake behavior still need connected-device observation. It is not a VSD-style key action. |
| Persistent boot logo | VSD Craft Settings → Device offers `Replace the boot logo (Resolution 480*272)`. | **Missing.** The fork's temporary screensaver background is not a boot-logo upload. The separate persistent upload protocol must be established and tested before exposing this option. |
| Startup/update/settings | VSD Craft Settings includes version, update check, language, power-on, minimize, reset-current-device, application-folder, reset-all-devices, and auto-detect options. | **Partial.** The fork has language, autolaunch, update checks, brightness, sleep, rotation, background, backup/restore, developer/statistics, config/log folders, and VSD import. Its reset/minimize/power-on controls are not one-for-one. |
| Store/account/notifications | The main VSD Craft window has store, account, and notification controls, plus Community/Reddit/Discord/mail links. | **Missing/partial.** The fork retains OpenDeck plugin management but does not reproduce the VSD Craft store/account/community surface. |
| Device-specific scope | VSD Craft bundles actions for M18 and many other MiraBox/VSD devices. | **Intentional divergence.** The fork is M18-only and does not need to retain the generic Stream Deck layout/device catalogue. |

## Profile migration coverage

The importer follows nested M18 pages and maps supported VSD action UUIDs to native actions or generic OpenDeck equivalents. The public repository intentionally omits private profile exports, selected-app lists, local profile paths, and device serial numbers. Validate migrations with sanitized fixtures and a backup of the user's own profile; do not commit personal exports.

### Boot-logo feasibility finding

The installed VSD Craft library has a separate `SDDevice::sendLogoSizeCommand(int,unsigned char)` routine. Read-only ARM64 disassembly shows a `CRT...LOG` size-negotiation packet, distinct from the temporary `BGPIC` display-background command used by the screensaver. This supports a persistent boot-logo upload path, but the image format, transfer chunks, completion acknowledgement, size limits, and recovery rules remain unknown. No boot-logo command was sent during this audit.

### Super Hotkey feasibility finding

[MiraBox distinguishes](https://mirabox.net/blogs/tutorial/hotkey-vs-super-hotkey-what-s-the-difference) ordinary software-input Hotkeys from Super Hotkeys that emulate a physical keyboard. Read-only inspection of this connected M18's macOS IORegistry shows a separate USB HID keyboard interface (`PrimaryUsagePage=1`, `PrimaryUsage=6`, `BootProtocol=1`) as well as the vendor-control interface. The installed VSD Craft library exposes `SDDevice::addKeyboardDownCommand`, `addKeyboardUpCommand`, and `HotkeyKeyValueHandler::mapToHardwareKey`. Its ARM64 down-command routine constructs an 18-byte packet containing `CRT` and `KEY` marker bytes and key payload. This supports a device-side keyboard path as a viable research direction, **not** a verified packet implementation. The fork currently uses Enigo/CGEvent for both Hotkey and Super Hotkey, so it does not yet preserve the distinction. No experimental keyboard command was sent to the device during this audit.

## Complete VSD Craft action inventory

The following inventory is from every `manifest.json` in VSD Craft's bundled plugin directory and default-plugin directory. “Importer partial” means the current importer recognizes the UUID and produces a generic/native fork instance; it does not mean that the original VSD settings UI or exact behavior is present.

### Brightness — partial

- Brightness — `com.hotspot.streamdock.device.brightness` — importer/native brightness equivalent exists.
- devsleep — `com.hotspot.streamdock.device.devsleep` — importer maps to display sleep; no exact action palette entry.

### Browser — partial

- Back — `com.hotspot.streamdock.hotkey.browser.back` — importer maps to generic input.
- Collect — `com.hotspot.streamdock.hotkey.browser.collect` — importer maps to generic input.
- Forward — `com.hotspot.streamdock.hotkey.browser.forward` — importer maps to generic input.
- Refresh — `com.hotspot.streamdock.hotkey.browser.refresh` — importer maps to generic input.

### Create Folder — partial

- Create Folder — `com.hotspot.streamdock.profile.openchild` — importer maps to profile switching; there is no exact Create Folder action.

### DateTime — missing

- DateTime — `com.mirabox.streamdock.dateTime.action1`

### Eat Gold Coins — missing

- Eat Gold Coins — `com.mirabox.streamdock.eatgoldcoins.action1`

### Emoticons — missing

- Emoticons Library — `com.mirabox.streamdock.emoticons.lib`
- Slelct Emoticons — `com.mirabox.streamdock.emoticons.select`
- Slelct Emoticons — `com.mirabox.streamdock.emoticons.select2`

### Go back — partial

- Go back — `com.hotspot.streamdock.profile.backtoparent` — importer maps to the default profile.

### Hotkey — partial

- Hotkey — `com.hotspot.streamdock.system.hotkey` — importer maps to `Simulate Input`; no exact VSD Hotkey inspector.

### HotkeySwitch — partial

- HotkeySwitch — `com.hotspot.streamdock.system.hotkeySwitch` — importer maps to the native M18 HotkeySwitch, including its two used states.

### K1ProLEDLight — not applicable to the M18

- LED+ — `com.hotspot.streamdock.device.k1proLED+`
- LED- — `com.hotspot.streamdock.device.k1proLED-`
- LEDMODECHANGE — `com.hotspot.streamdock.device.k1proLEDMODECHANGE`
- LEDSPEED+ — `com.hotspot.streamdock.device.k1proLEDSPEED+`
- LEDSPEED- — `com.hotspot.streamdock.device.k1proLEDSPEED-`
- WinqMac — `com.hotspot.streamdock.device.k1proWinqMac`

These are bundled for K1 Pro hardware, not for the connected M18. The fork instead has an M18-specific LED palette action in core.

### Knob Action Group — missing/not applicable

- Knob Action Group — `com.hotspot.streamdock.system.KnobOperatingGroup`

The M18 has no encoder/knob input, so this is not required for the target hardware.

### Mouse event — missing

- Mouse event — `com.hotspot.streamdock.mouse.event`

### Multi Action — partial

- Action Carousel — `com.hotspot.streamdock.multiactions.LunBo`
- Action Cycle — `com.hotspot.streamdock.multiactions.toggle`
- Delay — `com.hotspot.streamdock.multiactions.delay`
- Multi Action — `com.hotspot.streamdock.multiactions.routine` — the fork has a core Multi Action with a different UUID/model.
- Multi Action ActionTrigger (Knob) — `com.hotspot.streamdock.multiactions.ActionTrigger`
- Multi Action ActionWheel (Knob) — `com.hotspot.streamdock.multiactions.ActionWheel`

### Multimedia — partial

- Multimedia — `com.hotspot.streamdock.system.multimedia` — importer maps common indices to Music transport commands.
- SystemVolume (Knob) — `com.hotspot.streamdock.system.volume` — importer maps the volume family to a generic command; no M18 knob exists.

### MusicalRhythm — missing

- MusicalRhythm — `com.streamdock.musicalrhythma.action1`

### Network — missing

- UDP — `com.hotspot.streamdock.network.udp`

### Open — partial

- Open — `com.hotspot.streamdock.system.open` — importer maps to macOS `open`.
- OpenApps — `com.hotspot.streamdock.system.openApps` — importer maps to `open -a` using the selected app path/name.

### Pages — partial

- Go to page — `com.hotspot.streamdock.page.goto` — importer maps to a generated profile target.
- Next page — `com.hotspot.streamdock.page.next` — importer maps to the next generated profile target.
- Page Indicator — `com.hotspot.streamdock.page.indicator` — no native page indicator; importer does not translate it.
- Previous page — `com.hotspot.streamdock.page.previous` — importer maps to the previous generated profile target.
- change page (Knob) — `com.hotspot.streamdock.page.change` — importer maps to profile switching, but M18 has no knob.

### Password — missing

- Password — `com.hotspot.streamdock.system.password`

### Pigment mixing — missing

- Full Screen Paint — `com.mirabox.streamdock.pigmenteffects.action2`
- One-button paint — `com.mirabox.streamdock.pigmenteffects.action1`

### Quick Control — partial

- Calculator — `com.hotspot.streamdock.quicktool.calculator`
- Control panel — `com.hotspot.streamdock.quicktool.controlpanel`
- Display desktop — `com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop` — importer maps to generic input.
- Emoticons — `com.hotspot.streamdock.hotkey.quicktool.emoticons`
- Home page — `com.hotspot.streamdock.quicktool.homepage`
- Increase the volume — `com.hotspot.streamdock.hotkey.quickcontrol.volumeup` — importer maps to macOS volume command.
- Lower the volume — `com.hotspot.streamdock.hotkey.quickcontrol.volumedown` — importer maps to macOS volume command.
- Mail — `com.hotspot.streamdock.quicktool.mail`
- Microphone — `com.hotspot.streamdock.quickcontrol.microphone`
- Music — `com.hotspot.streamdock.quicktool.music`
- Mute — `com.hotspot.streamdock.hotkey.quickcontrol.mute` — importer maps to macOS mute command.
- Notification — `com.hotspot.streamdock.hotkey.quicktool.notification`
- Search bar — `com.hotspot.streamdock.hotkey.quicktool.searchbar`
- Sleep — `com.hotspot.streamdock.quickcontrol.sleep` — importer maps to display sleep.
- Speech recognition — `com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition` — importer maps to generic input.
- Switch screen — `com.hotspot.streamdock.hotkey.quickcontrol.switchscreen` — importer maps to generic input.
- Task Manager — `com.hotspot.streamdock.hotkey.quicktool.taskmanager`

### Scene Shift — partial

- Scene Shift — `com.hotspot.streamdock.profile.rotate` — importer maps to profile rotation.

### Screensaver — requested behavior matched, action parity missing

- Screensaver 1 — `com.mirabox.streamdock.screensaver.action1`
- Screensaver 2 — `com.mirabox.streamdock.screensaver.action2`

The fork's global M18 screensaver is designed for the requested behavior: it accepts a video or photo list, loops/slides it, leaves LEDs running, and consumes the first button press as wake-only input. It is configured in Settings rather than placed on an M18 key. These details are implemented in code but still require on-device verification.

### Soundboard — missing

- Play Audio — `com.hotspot.streamdock.soundboard.playaudio`
- Stop Audio — `com.hotspot.streamdock.soundboard.stopaudioplay`

### Super Hotkeys — partial

- Super Hotkeys — `com.hotspot.streamdock.system.super.hotkey` — importer maps to the native M18 Super Hotkeys action, but its software CGEvent execution does not yet reproduce the distinct VSD Craft Super Hotkey semantics.

### Text — missing/partial

- Text (secondary-screen controller) — `com.hotspot.streamdock.plain.text`
- Text (system) — `com.hotspot.streamdock.system.text`

The fork's generic input action is not a verified replacement for VSD Craft's configurable text action, so these remain unimplemented for parity purposes.

### Time Options — missing

- Countdown — `com.mirabox.streamdock.time.action3`
- Timer — `com.mirabox.streamdock.time.action2`
- World Time — `com.mirabox.streamdock.time.action1`

### Touchbar — partial

- DND Mode — `com.hotspot.streamdock.touchbar.dndmode`
- Decrease screen brightness — `com.hotspot.streamdock.touchbar.decreasescreenbrightness` — importer maps to macOS key code.
- Desktop Saver — `com.hotspot.streamdock.touchbar.desktopsaver` — importer maps to Screen Saver.
- Dictation — `com.hotspot.streamdock.touchbar.dictation`
- Dispatch Center — `com.hotspot.streamdock.touchbar.dispatchcenter` — importer maps to Control Center settings.
- Fast Forward — `com.hotspot.streamdock.touchbar.fastforward`
- Fast Rewind — `com.hotspot.streamdock.touchbar.fastrewind`
- Focus On Search — `com.hotspot.streamdock.touchbar.focusonsearch`
- Increase screen brightness — `com.hotspot.streamdock.touchbar.increasescreenbrightness` — importer maps to macOS key code.
- Input Method — `com.hotspot.streamdock.touchbar.inputmethod`
- Launchpad — `com.hotspot.streamdock.touchbar.launchpad` — importer maps to Launchpad.
- Mute — `com.hotspot.streamdock.touchbar.mute` — importer maps to macOS mute.
- Next Track — `com.hotspot.streamdock.touchbar.nexttrack` — importer maps to Music.
- Notification Center — `com.hotspot.streamdock.touchbar.notificationcenter`
- Play/Pause — `com.hotspot.streamdock.touchbar.playpause` — importer maps to Music.
- Previous Track — `com.hotspot.streamdock.touchbar.previoustrack` — importer maps to Music.
- Screen Lock — `com.hotspot.streamdock.touchbar.screenlock`
- Screen brightness (Knob) — `com.hotspot.streamdock.touchbar.screen.brightness` — importer recognizes the family, but the M18 has no knob.
- Screenshot — `com.hotspot.streamdock.touchbar.screenshot` — importer maps to `screencapture`.
- Show Desktop — `com.hotspot.streamdock.touchbar.showdesktop`
- Siri — `com.hotspot.streamdock.touchbar.siri` — importer maps to opening Siri.
- Sleep — `com.hotspot.streamdock.touchbar.sleep` — importer maps to display sleep.
- Volume (Knob) — `com.hotspot.streamdock.touchbar.volume` — importer recognizes the family, but the M18 has no knob.
- Volume down — `com.hotspot.streamdock.touchbar.volumedown` — importer maps to macOS volume.
- Volume up — `com.hotspot.streamdock.touchbar.volumeup` — importer maps to macOS volume.

### Useful notes — missing

- Record to-do — `com.hotspot.streamdock.memo.action2`
- Remember things — `com.hotspot.streamdock.memo.action1`

### Water tank — missing

- Water tank — `com.mirabox.streamdock.watertank.action1`

### Weather query — missing

- Weather query — `com.hotspot.streamdock.weather.action1`

### Website — partial

- Website — `com.hotspot.streamdock.system.website` — importer maps the VSD path/URL to `Open URL`.

### YouTube — missing

- Chat Message — `com.hotspot.streamdock.youtube.chatmessage`
- Viewers — `com.hotspot.streamdock.youtube.viewers`

### calendar — missing

- calendar — `com.mirabox.streamdock.calendar.action1`

### close — missing

- Close — `com.hotspot.streamdock.system.close`

### emoji — missing

- emoji — `com.mirabox.streamdock.emoji.emoji`
- emoji send — `com.mirabox.streamdock.emoji.emoji_send`

### pr — partial import only

These actions are preset keyboard shortcuts for Adobe Premiere-style editing. The importer recognizes their `.hotkey.` UUID pattern and produces generic input mappings, but there is no Premiere-specific action family or inspector.

- Add Edit — `com.hotspot.streamdock.hotkey.pr.addEdit`
- Add Marker — `com.hotspot.streamdock.hotkey.pr.addMarker`
- Copy — `com.hotspot.streamdock.hotkey.pr.copy`
- Cut — `com.hotspot.streamdock.hotkey.pr.cut`
- Delete — `com.hotspot.streamdock.hotkey.pr.delete`
- Effects panel — `com.hotspot.streamdock.hotkey.pr.effectsPanel`
- Paste — `com.hotspot.streamdock.hotkey.pr.paste`
- Pen Tool — `com.hotspot.streamdock.hotkey.pr.penTool`
- Play — `com.hotspot.streamdock.hotkey.pr.play`
- Razor — `com.hotspot.streamdock.hotkey.pr.razor`
- Rectangle Tool — `com.hotspot.streamdock.hotkey.pr.rectangleTool`
- Ripple Editing Tools — `com.hotspot.streamdock.hotkey.pr.rippleEditingTools`
- Toggle Full Screen — `com.hotspot.streamdock.hotkey.pr.toggleFullScreen`
- Undo — `com.hotspot.streamdock.hotkey.pr.undo`
- timeLine — `com.hotspot.streamdock.pr.action5`

### vMix — missing

- Shortcut — `com.hotspot.streamdock.vmix.shortcut`

## Fork-native action surface today

The action list currently contains these 19 selectable actions:

- Multi Action — `opendeck.multiaction`
- Toggle Action — `opendeck.toggleaction`
- M18 LED Colors — `opendeck.m18.led-colors`
- OpenApps — `opendeck.m18.open-apps`
- Super Hotkeys — `opendeck.m18.super-hotkeys`
- HotkeySwitch — `opendeck.m18.hotkey-switch`
- Super Hotkey Switch — `opendeck.m18.super-hotkey-switch` (software-input fallback)
- Volume down / Volume up — `opendeck.m18.volume-down` / `opendeck.m18.volume-up`
- Mute — `opendeck.m18.mute`
- Siri — `opendeck.m18.siri`
- Previous / Next / Go to page — `opendeck.m18.page-previous` / `opendeck.m18.page-next` / `opendeck.m18.page-goto`
- Run Command — `com.amansprojects.starterpack.runcommand`
- Open URL — `com.amansprojects.starterpack.openurl`
- Simulate Input — `com.amansprojects.starterpack.inputsimulation`
- Switch Profile — `com.amansprojects.starterpack.switchprofile`
- Device Brightness — `com.amansprojects.starterpack.devicebrightness`

This is why the fork can import the current M18 setup without presenting the full VSD catalog: the importer composes a smaller number of generic primitives. That is useful for migration, but it is not yet feature parity for users who expect to create every VSD action from the action list.

## Recommended parity order

The following order gives the highest practical value for this M18 profile and the lowest risk to the direct hardware layer:

1. Verify representative migrated actions, occupied-slot swaps, all three bottom buttons, computer-idle screensaver, and full-display video alignment on hardware. Distinguish ordinary Hotkey from Super Hotkey input at the transport/OS boundary.
2. Reverse-engineer and safely test the persistent 480×272 boot-logo upload; it is separate from the temporary screensaver background.
3. Add native text, soundboard, browser, media, sleep, screen-brightness, screenshot, and app/file/folder actions with macOS-native implementations and inspectors.
4. Add a native page indicator if required; the M18 page navigation model already exists.
5. Add multi-action delay/cycle/carousel behavior.
6. Add dynamic information actions: calendar, DateTime, timer/countdown, weather, memo, YouTube, and emoticons.
7. Add specialist integrations only if needed: UDP, vMix, Premiere, pigment mixing, musical rhythm, water tank, and game-like actions.
8. Add VSD-compatible scene import/export and the VSD Craft title/icon/app-picker inspector only after the action model stabilizes.

The current hardware architecture does not need to be redesigned for these ports. The missing work is mostly action implementations, property inspectors, settings translation, and long-running macOS behavior tests.
