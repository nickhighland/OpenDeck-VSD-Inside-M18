# VSD Craft feature and parity audit

Audit date: 2026-09-25; live migration update: 2026-09-26
VSD Craft version: `3.10.205.0918`
Connected device: VSD Inside M18 (serial omitted)
Fork: OpenDeck VSD M18

## Executive result

The M18 hardware replacement is viable, but the fork does **not** yet have one-for-one VSD Craft feature parity.

The audit found:

- VSD Craft ships 123 distinct action UUIDs across its bundled and default action packages.
- The fork covers all 109 VSD Craft action UUIDs applicable to the M18 keypad in its core catalog/import layer: 106 single-action catalog entries plus native Multi Action, Action Carousel, and Action Cycle parents, grouped into nine searchable categories. Six K1 Pro-only actions, seven knob-only actions, and the secondary-screen-only Text action are excluded. Common system controls, page navigation, OpenApps, Hotkey, Multimedia, M18 brightness, mouse events, and Scene Shift have direct core entries. Super Hotkey still uses software input, not VSD Craft's distinct hardware-keyboard path.
- Catalog and import coverage are ahead of behavior parity: several vendor-specific actions still have incomplete or missing runtime behavior and action-specific inspectors. Native folder navigation is implemented, but real nested-profile target resolution still needs a live comparison. Composite imports normalize flat `MultiActionData` children to native M18/OpenDeck actions and dispatch them in core. Nested composite children remain explicit inactive placeholders because the existing editor/context addressing model is flat. Synthetic schema tests do not replace a sanitized M18 profile; `Delay1`/`Delay2` timing and Carousel/Cycle artwork/state still need validation.
- The importer supports nested M18 pages and maps supported actions to native M18/OpenDeck actions. Private profile exports and selected-app lists are intentionally excluded from this public audit.

The strongest parity area is the hardware layer: the fork drives the M18 directly from the application core, including its 15 LCD keys, three physical bottom buttons, LED output, reconnect handling, and sleep/wake. Display sleep follows the computer's state (computer idle time and, optionally, the lock screen), with a wake-only first press; it still needs physical-device verification. The largest parity gap is the action catalog and its property inspectors.

## Verification snapshot — 2026-09-26

Current local source checks pass: 43 Rust tests, 3 frontend action-search tests, Svelte checking with 0 errors and 0 warnings, Rust formatting, and the production frontend build. The frontend build emits SvelteKit/Svelte SSR-export and large-chunk warnings but completes. The optimized macOS arm64 app and DMG also build locally; strict ad-hoc code-signature verification passes after clearing macOS provenance metadata from the generated bundle. These checks establish source/build coverage only; they do not establish GUI behavior or M18 hardware parity.

The repository-level Unraid runner is registered: Unraid showed `github-runner-opendeck-vsd-inside-m18` started with Autostart enabled, and GitHub listed `unraid-opendeck-vsd-inside-m18` as Active with the `unraid` label. The first two jobs reached the runner but stopped at package installation because its previous Ubuntu 20.04/Focal image did not provide WebKitGTK 4.1. The Unraid template now shows `myoung34/github-runner:ubuntu-jammy`; persistent files and `DISABLE_AUTOMATIC_DEREGISTRATION=true` remain set, and the same runner returned Active after the image change. The next run exposed missing `rustfmt`; after adding it to workflow setup, [run 36291988009](https://github.com/nickhighland/OpenDeck-VSD-Inside-M18/actions/runs/36291988009) passed Rust formatting, frontend type checks, 3 action-search tests, 41 Rust tests, and the production frontend build on the Jammy runner. Sustained uptime and reconnect-after-restart remain open.

## Evidence and method

The audit used three independent sources:

1. Live inspection of the installed VSD Craft window while an M18 was connected. This covered device selection, pages, scene controls, the settings window, action categories, search, and a representative action property inspector.
2. Read-only inspection of VSD Craft's installed manifests, bundled plugin directories, profile manifests, images, videos, logs, preferences, and `DataCache.db`.
3. Read-only inspection of the fork source, especially `src-tauri/src/m18.rs`, `src-tauri/src/vsd_import.rs`, the profile manager, and the action catalog.

Actions were not executed when doing so would launch applications, send keystrokes, change system settings, or modify the user's profile. For those actions, the installed manifest and property-inspector definitions were used as the feature evidence.

The live screenshots reviewed inline are valid evidence for visible UI parity (controls, labels, defaults, and layout); a saved screenshot archive is useful for durable review but is not a prerequisite for continuing the comparison. Static screenshots do not establish runtime semantics, so behavioral parity is assessed separately using manifest/source evidence, sanitized import fixtures, and safe action tests.

## Application-level feature matrix

| VSD Craft feature | What was verified | Fork status |
|---|---|---|
| Menu-bar/background operation | The app bundle sets `LSUIElement=true`; the live Settings window has checked `Power on` and `Minimize` options. | **Partial.** The fork has autolaunch/background settings, but not the same VSD Craft menu-bar/settings surface. |
| Device discovery | Live device selector showed a VSD Inside M18; the device dropdown showed the connected M18. `Automatically Detect and Prompt` is present in VSD Craft settings. | **Matched for M18.** The fork has direct M18 discovery and registration in the core. Its hardware support is not an installable hardware plugin. |
| M18 LCD layout | Live editor shows a 3 × 5 grid of 15 LCD keys. | **Matched.** The fork renders the same 15 LCD positions. |
| Three bottom physical buttons | Live editor shows three separate circular buttons below the LCD grid; the profile contains positions `5,0`, `5,1`, and `5,2`. | **Matched.** The fork maps the bottom buttons to logical positions 15–17, accepts actions on them, and does not attempt LCD image writes for them. |
| Device image output | VSD Craft displays per-key PNG/GIF/JPG artwork and custom titles. | **Partial.** The fork renders still images and converts the rendered result to M18 JPEG output. Animated images are not continuously animated on the device; MP4/MBG per-key media is not supported. |
| OpenApps artwork | VSD Craft's OpenApps action supplies an icon for the selected application. | **Matched for app selections.** Native OpenApps resolves the selected app path, bundle identifier, or name and uses the macOS application icon in the editor and on the M18. |
| Device brightness | VSD Craft exposes a device Settings slider and a key action with Increase/Decrease choices; its guide notes a non-zero minimum. | **Partial.** The fork now mirrors the two key choices, tracks the M18's last commanded brightness for repeated relative presses, and updates the settings slider. The step size and enforced minimum still need comparison; the fork's global slider currently permits zero. Sleep remains a core setting/behavior rather than an exact VSD action. |
| M18 LEDs | VSD Craft's M18 profile does not use an explicit LED action; its bundle also contains K1 Pro LED actions for another device family. | **Matched/enhanced for M18.** The fork has a built-in `M18 LED Colors` action and core LED output. |
| Reconnect/keepalive | VSD Craft maintains a connected device session, but the user reports frequent application crashes. | **Matched at the hardware boundary.** The fork's HID session, keepalive, reconnect, and sleep/wake path are in Rust core code, outside a hardware plugin process. Stability still needs long-running real-device testing. |
| Scenes | VSD Craft has `Create Scene`, remove, rename, backup, import, export, and a default-scene checkbox. | **Partial.** The fork has profiles, folders, create/delete/rename/duplicate, backup/restore, and application-profile selection, but not the same scene editor or scene import/export format. |
| Pages | VSD Craft visibly has page buttons `1`, `2`, and `+`; the live editor exposes a Go to Page selector and checked “Show page number” option. | **Partial, implementation added.** The importer creates native M18 pages, treats VSD page numbers as one-based, preserves the page-number visibility option (defaulting to VSD Craft’s checked state), and renders page numbers for Go to Page and Page Indicator. The actual M18 visual match remains unverified. |
| App-based automatic switching | VSD Craft Scenes settings contain three application selectors and a “following app foreground, auto-switch to scene” option. | **Partial/matched conceptually.** The fork has an application watcher and per-application profile mappings. The VSD three-slot scene UI is not replicated. |
| Action catalog | VSD Craft has category groups, drag-to-key actions, a search field, and a large bundled catalog. | **Cataloged; behavior partial.** The fork covers all 109 VSD Craft action UUIDs applicable to the M18 keypad in nine searchable core categories: 106 single-action entries and three native flow-parent actions. Search covers labels, localized labels, tooltips, identifiers, and category names, with matching groups opened automatically. It omits K1 Pro, knob-only, and secondary-screen-only actions. |
| Action property inspector | VSD Craft's selected OpenApps key showed delete, title entry, title styling, app selection, app-list reload, and icon customization. | **Partial.** The fork has dedicated inspectors for OpenApps, hotkey switches, and page navigation, plus an editor for retained scalar/JSON settings on catalogued VSD actions. This does not reproduce all VSD-specific controls or establish behavior parity. |
| Multi-action composition | VSD Craft includes Multi Action, Action Carousel, Action Cycle, Delay, and knob variants. | **Partial, code path implemented.** The fork has separate native Multi Action, Carousel, and Cycle parents; it imports and dispatches flat child sequences through application core and handles explicit Delay actions. Nested composite children are preserved as inactive placeholders pending hierarchical editor/context addressing. `Delay1`/`Delay2` order, exact Cycle-vs-Carousel behavior, and imported artwork/state remain unverified against a sanitized VSD profile and the live device. Knob variants do not apply to the M18. |
| VSD Craft profile import | VSD profiles contain nested page profiles, per-position actions, state images/titles, and device metadata. | **Partial.** The importer follows M18 pages, preserves state artwork/titles and OpenApps icons, maps known system actions, and imports flat composite children into native core instances. Nested composite children remain inactive placeholders. Synthetic tests cover the inferred parent/child shape; a sanitized real profile is still needed to validate timing, custom child names/artwork, and Cycle/Carousel state. Private profile contents are not included here. |
| Per-key GIF/video artwork | The installed VSD Craft profile library contains PNG, GIF, MBG, JPG, and MP4 assets. | **Partial.** GIFs can be decoded for the UI's image path, but the M18 receives a rendered JPEG frame rather than an animated stream. MP4/MBG is not a supported per-key image format. |
| VSD Craft screensaver actions | The bundle includes `Screensaver 1` and `Screensaver 2` action plugins. | **Intentionally excluded.** The M18 is an LCD panel without burn-in risk, so the fork removed its device screensaver. Display sleep follows the computer's state instead: on while the computer is in use, off after the configured computer idle time or while locked. Imported screensaver keys are kept but do nothing. |
| Persistent boot logo | VSD Craft Settings → Device offers `Replace the boot logo (Resolution 480*272)`. | **Intentionally excluded per user request.** Do not implement or send logo-upload commands. |
| Startup/update/settings | VSD Craft Settings includes version, update check, language, power-on, minimize, reset-current-device, application-folder, reset-all-devices, and auto-detect options. | **Partial.** The fork has language, autolaunch, update checks, brightness, sleep, rotation, background, backup/restore, developer/statistics, config/log folders, and VSD import. Its reset/minimize/power-on controls are not one-for-one. |
| Store/account/notifications | The main VSD Craft window has store, account, and notification controls, plus Community/Reddit/Discord/mail links. | **Missing/partial.** The fork retains OpenDeck plugin management but does not reproduce the VSD Craft store/account/community surface. |
| Device-specific scope | VSD Craft bundles actions for M18 and many other MiraBox/VSD devices. | **Intentional divergence.** The fork is M18-only and does not need to retain the generic Stream Deck layout/device catalogue. |

## Profile migration coverage

The importer follows nested M18 pages and maps supported VSD action UUIDs to native actions or generic OpenDeck equivalents. The public repository intentionally omits private profile exports, selected-app lists, local profile paths, and device serial numbers. Validate migrations with sanitized fixtures and a backup of the user's own profile; do not commit personal exports.

Boot-logo replacement is explicitly out of scope. Do not reverse-engineer or send boot-logo commands; parity work covers the M18's runtime controls, not its persistent startup image.

### Super Hotkey feasibility finding

[MiraBox distinguishes](https://mirabox.net/blogs/tutorial/hotkey-vs-super-hotkey-what-s-the-difference) ordinary software-input Hotkeys from Super Hotkeys that emulate a physical keyboard. Read-only inspection of this connected M18's macOS IORegistry shows a separate USB HID keyboard interface (`PrimaryUsagePage=1`, `PrimaryUsage=6`, `BootProtocol=1`) as well as the vendor-control interface. The installed VSD Craft library exposes `SDDevice::addKeyboardDownCommand`, `addKeyboardUpCommand`, and `HotkeyKeyValueHandler::mapToHardwareKey`. ARM64 disassembly shows a 127-entry macOS virtual-key-to-USB-usage table, a separate modifier map, and construction of an 18-byte `CRT`/`KEY` command queued with type `0x0f`. This is strong protocol evidence, not a verified end-to-end report format or press/release implementation. The fork currently uses Enigo/CGEvent for both Hotkey and Super Hotkey, so it does not yet preserve the distinction. No experimental keyboard command was sent to the device during this audit.

## Complete VSD Craft action inventory

The following inventory is from every `manifest.json` in VSD Craft's bundled plugin directory and default-plugin directory. “Importer partial” means the current importer recognizes the UUID and produces a generic/native fork instance; it does not mean that the original VSD settings UI or exact behavior is present.

### Brightness — partial

- Brightness — `com.hotspot.streamdock.device.brightness` — the key inspector now matches VSD Craft's Increase/Decrease choices, and its action adjusts tracked device brightness and updates the settings slider. Step amount and minimum brightness still need exact comparison with VSD Craft; its separate settings slider is only partially matched.
- devsleep — `com.hotspot.streamdock.device.devsleep` — importer maps to display sleep; no exact action palette entry.

### Browser — partial

- Back — `com.hotspot.streamdock.hotkey.browser.back` — importer maps to generic input.
- Collect — `com.hotspot.streamdock.hotkey.browser.collect` — importer maps to generic input.
- Forward — `com.hotspot.streamdock.hotkey.browser.forward` — importer maps to generic input.
- Refresh — `com.hotspot.streamdock.hotkey.browser.refresh` — importer maps to generic input.

### Create Folder — partial

- Create Folder — `com.hotspot.streamdock.profile.openchild` — imports as a native core action. The importer resolves the vendor `ProfileUUID` to an imported M18 page, and opening it pushes the current page onto a persisted navigation stack. The inspector lets a user pick the target page.

MiraBox's [core-features guide](https://mirabox.net/it/blogs/tutorial/streamdock-core-features-guide) describes Create Folder as a nestable one-page container. Native page-stack behavior and import target resolution are implemented and covered by sanitized tests; validating the actual exported nested-profile relationship and on-device navigation remains open.

### DateTime — missing

- DateTime — `com.mirabox.streamdock.dateTime.action1`

### Eat Gold Coins — missing

- Eat Gold Coins — `com.mirabox.streamdock.eatgoldcoins.action1`

### Emoticons — missing

- Emoticons Library — `com.mirabox.streamdock.emoticons.lib`
- Slelct Emoticons — `com.mirabox.streamdock.emoticons.select`
- Slelct Emoticons — `com.mirabox.streamdock.emoticons.select2`

### Go back — partial

- Go back — `com.hotspot.streamdock.profile.backtoparent` — imports as a native core action and pops the M18 folder-navigation stack. Runtime/device comparison for nested folders remains open.

### Hotkey — partial

- Hotkey — `com.hotspot.streamdock.system.hotkey` — imports as a native core action and translates VSD virtual-key codes and modifiers into multi-key Enigo tokens; OS-behavior testing remains.

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

### Mouse event — partial

- Mouse event — `com.hotspot.streamdock.mouse.event` — native core click/double-click, move, scroll, drag, modifier tokens, current-pointer capture, and absolute/relative coordinate controls are implemented. Exact VSD coordinate conventions, mouse-wheel button and drag semantics, plus real macOS Accessibility behavior still need comparison.

### Multi Action — partial

- Action Carousel — `com.hotspot.streamdock.multiactions.LunBo`
- Action Cycle — `com.hotspot.streamdock.multiactions.toggle`
- Delay — `com.hotspot.streamdock.multiactions.delay`
- Multi Action — `com.hotspot.streamdock.multiactions.routine` — the fork has a core Multi Action with a different UUID/model.
- Multi Action ActionTrigger (Knob) — `com.hotspot.streamdock.multiactions.ActionTrigger`
- Multi Action ActionWheel (Knob) — `com.hotspot.streamdock.multiactions.ActionWheel`

MiraBox's [Operation Flow guide](https://mirabox.net/blogs/tutorial/how-to-use-operation-flow) describes Multi Action/Action Flow as sequential execution, Carousel as advancing to the next item on each press, and Action Toggle as switching the active state on each press. The installed VSD UUID `com.hotspot.streamdock.multiactions.toggle` is its Action Cycle entry. Read-only ARM64 inspection identifies the parent's `MultiActionData` sequence, per-item `Settings`, `ActionID`, `UUID`, and `MultiActionDelays` fields (including `Delay1`/`Delay2`), plus an `Index` setting for cycle state. The importer now maps flat sequences to native core children, dispatches child actions through the core, handles explicit Delay items, and preserves child delays/artwork. Nested flow children remain inactive placeholders because the current editor/context addressing model is flat. Synthetic tests exercise the inferred fields, but no sanitized real profile has yet confirmed `Delay1`/`Delay2` direction/units, Carousel-vs-Cycle selection and reset semantics, or artwork matching. The Action Wheel and Action Trigger entries are knob-controller variants; the M18 has no knob, so they are not M18 features.

### Multimedia — partial

- Multimedia — `com.hotspot.streamdock.system.multimedia` — the inspector exposes all seven vendor operations in documented order; previous/play-pause/next and mute/volume use macOS system media keys. Stop targets Music only when Music is already running; generic-player Stop parity remains open. Hardware/media-session behavior has not been manually verified.
- SystemVolume (Knob) — `com.hotspot.streamdock.system.volume` — importer maps the volume family to a generic command; no M18 knob exists.

### MusicalRhythm — missing

- MusicalRhythm — `com.streamdock.musicalrhythma.action1`

### Network — partial

- UDP — `com.hotspot.streamdock.network.udp` — sends a configured UTF-8 datagram; wire-level behavior and imported setting-name compatibility still need tests.

### Open — partial

- Open — `com.hotspot.streamdock.system.open` — importer maps to macOS `open`.
- OpenApps — `com.hotspot.streamdock.system.openApps` — importer maps to `open -a` using the selected app path/name.

### Pages — partial

- Go to page — `com.hotspot.streamdock.page.goto` — importer maps its one-based VSD page number to the matching native M18 page and adds the “Show page number” control.
- Next page — `com.hotspot.streamdock.page.next` — importer maps to the next generated profile target.
- Page Indicator — `com.hotspot.streamdock.page.indicator` — importer maps to the native M18 Page Indicator, which renders the current page’s one-based number in a full 72×72 key image. Visual matching on the physical display remains unverified.
- Previous page — `com.hotspot.streamdock.page.previous` — importer maps to the previous generated profile target.
- change page (Knob) — `com.hotspot.streamdock.page.change` — importer maps to profile switching, but M18 has no knob.

### Password — partial

- Password — `com.hotspot.streamdock.system.password` — runtime types a saved string; a password-specific inspector now obscures the field, but storage is still plaintext in the profile and behavior needs parity tests.

### Pigment mixing — missing

- Full Screen Paint — `com.mirabox.streamdock.pigmenteffects.action2`
- One-button paint — `com.mirabox.streamdock.pigmenteffects.action1`

### Quick Control — partial

- Calculator — `com.hotspot.streamdock.quicktool.calculator`
- Control panel — `com.hotspot.streamdock.quicktool.controlpanel`
- Display desktop — `com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop` — importer maps to F11 through a valid Enigo key token; exact VSD Craft mapping and macOS keyboard preferences still need comparison.
- Emoticons — `com.hotspot.streamdock.hotkey.quicktool.emoticons`
- Home page — `com.hotspot.streamdock.quicktool.homepage`
- Increase the volume — `com.hotspot.streamdock.hotkey.quickcontrol.volumeup` — importer maps to macOS volume command.
- Lower the volume — `com.hotspot.streamdock.hotkey.quickcontrol.volumedown` — importer maps to macOS volume command.
- Mail — `com.hotspot.streamdock.quicktool.mail`
- Microphone — `com.hotspot.streamdock.quickcontrol.microphone` — macOS now toggles mute on the default input device through Core Audio and reports an error when the active device has no writable mute control. Behavior against the same device/control that VSD Craft uses still needs hardware verification.
- Music — `com.hotspot.streamdock.quicktool.music`
- Mute — `com.hotspot.streamdock.hotkey.quickcontrol.mute` — importer maps to macOS mute command.
- Notification — `com.hotspot.streamdock.hotkey.quicktool.notification` — currently opens Notification settings as a fallback rather than the Notification Center panel; not parity.
- Search bar — `com.hotspot.streamdock.hotkey.quicktool.searchbar`
- Sleep — `com.hotspot.streamdock.quickcontrol.sleep` — importer maps to display sleep.
- Speech recognition — `com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition` — maps to F5 through a valid Enigo key token; macOS dictation configuration and vendor mapping remain unverified.
- Switch screen — `com.hotspot.streamdock.hotkey.quickcontrol.switchscreen` — maps to Control+F1 through a valid Enigo key token; exact VSD Craft behavior remains unverified.
- Task Manager — `com.hotspot.streamdock.hotkey.quicktool.taskmanager`

### Scene Shift — partial

- Scene Shift — `com.hotspot.streamdock.profile.rotate` — native core action cycles the M18's page/profile set; imported VSD scene linkage and nested folder navigation still need validation.

### Screensaver — intentionally excluded

- Screensaver 1 — `com.mirabox.streamdock.screensaver.action1`
- Screensaver 2 — `com.mirabox.streamdock.screensaver.action2`

The M18's keys are windows onto one LCD panel, which is not prone to burn-in, so the fork has no device screensaver. Its display sleep follows the computer's state (computer idle time and, optionally, the lock screen) and turns the backlight off, which is what actually protects the panel. Imported screensaver keys are preserved but do nothing.

### Soundboard — partial

- Play Audio — `com.hotspot.streamdock.soundboard.playaudio`
- Stop Audio — `com.hotspot.streamdock.soundboard.stopaudioplay`

The installed action manifest gives Play Audio two visual states and Stop Audio one. MiraBox's [Audio Player guide](https://mirabox.net/blogs/tutorial/how-to-use-the-audio-player-feature-on-streamdock) documents four Play Audio modes—Play/Stop, Play/Overlap, Play/Replay, and Loop/Stop—plus per-action volume, output-device selection, and MP3/WAV/MP4/M4A/M4B/M4P/MOV/AIFF/FLAC files. VSD Craft's installed inspector strings show No fade, Fade in, Fade out, and Fade in/out with duration in seconds. The fork now uses Rodio/Symphonia for macOS playback, device selection, volume, and those fades. Actual output switching, fade timing, and the vendor's full codec list still need manual comparison; encrypted M4P support is unverified.

### Super Hotkeys — partial

- Super Hotkeys — `com.hotspot.streamdock.system.super.hotkey` — importer maps to the native M18 Super Hotkeys action, but its software CGEvent execution does not yet reproduce the distinct VSD Craft Super Hotkey semantics.

### Text — partial

- Text (secondary-screen controller) — `com.hotspot.streamdock.plain.text` — intentionally excluded; the M18 has no secondary screen.
- Text (system) — `com.hotspot.streamdock.system.text`

The M18 catalog includes only the system Text action, which types its configured string through the core input path and has a dedicated multiline editor. Exact VSD Craft handling of key-up/down and Unicode still needs verification.

### Time Options — missing

- Countdown — `com.mirabox.streamdock.time.action3`
- Timer — `com.mirabox.streamdock.time.action2`
- World Time — `com.mirabox.streamdock.time.action1`

### Touchbar — partial

- DND Mode — `com.hotspot.streamdock.touchbar.dndmode` — currently opens Focus settings instead of toggling DND; not parity.
- Decrease screen brightness — `com.hotspot.streamdock.touchbar.decreasescreenbrightness` — importer maps to macOS key code.
- Desktop Saver — `com.hotspot.streamdock.touchbar.desktopsaver` — importer maps to Screen Saver.
- Dictation — `com.hotspot.streamdock.touchbar.dictation` — maps to F5, but its match to the vendor action and system dictation setup is unverified.
- Dispatch Center — `com.hotspot.streamdock.touchbar.dispatchcenter` — importer maps to Control Center settings.
- Fast Forward — `com.hotspot.streamdock.touchbar.fastforward`
- Fast Rewind — `com.hotspot.streamdock.touchbar.fastrewind`
- Focus On Search — `com.hotspot.streamdock.touchbar.focusonsearch`
- Increase screen brightness — `com.hotspot.streamdock.touchbar.increasescreenbrightness` — importer maps to macOS key code.
- Input Method — `com.hotspot.streamdock.touchbar.inputmethod` — maps to Control+Space; actual input-source switching remains unverified.
- Launchpad — `com.hotspot.streamdock.touchbar.launchpad` — importer maps to Launchpad.
- Mute — `com.hotspot.streamdock.touchbar.mute` — importer maps to macOS mute.
- Next Track — `com.hotspot.streamdock.touchbar.nexttrack` — importer maps to Music.
- Notification Center — `com.hotspot.streamdock.touchbar.notificationcenter` — currently opens notification settings rather than the Notification Center panel; not parity.
- Play/Pause — `com.hotspot.streamdock.touchbar.playpause` — importer maps to Music.
- Previous Track — `com.hotspot.streamdock.touchbar.previoustrack` — importer maps to Music.
- Screen Lock — `com.hotspot.streamdock.touchbar.screenlock` — sends Command+Control+Q through a valid Enigo key token; not hardware-verified.
- Screen brightness (Knob) — `com.hotspot.streamdock.touchbar.screen.brightness` — importer recognizes the family, but the M18 has no knob.
- Screenshot — `com.hotspot.streamdock.touchbar.screenshot` — importer maps to `screencapture`.
- Show Desktop — `com.hotspot.streamdock.touchbar.showdesktop` — sends F11 through a valid Enigo key token; exact VSD Craft mapping remains unverified.
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

### close — partial

- Close — `com.hotspot.streamdock.system.close` — a macOS quit request exists; application resolution, cross-platform behavior, and its inspector need parity tests.

### emoji — partial

- emoji — `com.mirabox.streamdock.emoji.emoji` — opens Character Viewer unless configured text is present.
- emoji send — `com.mirabox.streamdock.emoji.emoji_send` — types configured text; native picker, recent/favorite behavior, and imported settings are not yet matched.

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

The application core currently exposes these 23 native M18 actions:

- M18 LED Colors — `opendeck.m18.led-colors`
- OpenApps — `opendeck.m18.open-apps`
- Super Hotkeys — `opendeck.m18.super-hotkeys`
- HotkeySwitch — `opendeck.m18.hotkey-switch`
- Super Hotkey Switch — `opendeck.m18.super-hotkey-switch` (software-input fallback)
- Volume down / Volume up — `opendeck.m18.volume-down` / `opendeck.m18.volume-up`
- Mute — `opendeck.m18.mute`
- Siri — `opendeck.m18.siri`
- Dispatch Center, Screenshot, Launchpad, Desktop Saver, Sleep
- Increase/decrease screen brightness
- Previous Track, Play/Pause, Next Track
- Previous / Next / Go to page — `opendeck.m18.page-previous` / `opendeck.m18.page-next` / `opendeck.m18.page-goto`
- Page Indicator — `opendeck.m18.page-indicator`

The broader selectable catalog also includes OpenDeck's Multi Action and Toggle Action, plus generic Starter Pack actions such as Run Command, Open URL, Simulate Input, Switch Profile, and Device Brightness. The VSD catalog is not reproduced one-for-one.

This is why the fork can import the current M18 setup without presenting the full VSD catalog: the importer composes a smaller number of generic primitives. That is useful for migration, but it is not yet feature parity for users who expect to create every VSD action from the action list.

## M18 parity scope and remaining work

Parity means every VSD Craft action and workflow supported by the M18, including the three physical bottom buttons. Boot-logo replacement is the sole intentionally excluded M18 capability. K1 Pro lighting and knob/encoder-only actions do not apply to the M18. A catalog entry, retained settings, or import placeholder does not count as behavior parity.

Work order:

1. Add hierarchical action editing/context addressing for nested composites, then validate composite timing/state/artwork and folder target serialization using a sanitized real profile and live M18 comparison.
2. Give every M18-compatible catalog action an action-specific inspector and executable behavior; remove silent no-op branches. Use vendor manifests/profile fixtures to record settings semantics, and explicitly document any external service or OS permission required.
3. Complete local actions first: ordinary vs Super Hotkey, text/password, app/file/website open and close, system controls, media, soundboard, UDP, emoji, timer/countdown/date/time/calendar, and memo actions.
4. Complete service/integration actions: weather, YouTube, vMix, Premiere presets, paint/pigment, rhythm, water-tank, and game-like actions where the installed VSD Craft M18 package exposes them.
5. Match M18 scene/page, settings, state-artwork, title/icon/app-selection, and import/export workflows that are part of the M18 experience.
6. Verify code-level behavior with unit/integration fixtures, then keep hardware-only checks explicit: buttons 1–18, input transport distinction, dynamic display refresh, display sleep/wake, LEDs, reconnect, and visual alignment.

The existing direct M18 hardware architecture remains the target. Each item must be reported as implemented, code-tested, hardware-verified, or blocked by an external dependency; boot-logo work must not appear in the remaining-work list.
