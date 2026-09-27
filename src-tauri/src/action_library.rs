//! The action library shown in the editor.
//!
//! Every built-in action lives in exactly one group, with a clear name, a
//! one-line description, and its own key artwork. VSD Craft ships several
//! UUIDs for the same function (three "Volume up" actions, two "Sleep"
//! actions, …); the duplicates stay in the catalog so imported keys keep
//! working, but only one of each is listed. Actions whose behaviour is not
//! implemented yet are grouped under "Coming Soon" rather than mixed in with
//! working ones.

use crate::shared::{Action, ActionState, Category};

use std::collections::HashMap;

/// Library groups, in the order the editor shows them. The editor mirrors
/// these names (see `src/lib/actionLibrary.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
	Apps,
	Keyboard,
	Media,
	System,
	DisplayPower,
	Pages,
	Flows,
	Device,
	Browser,
	Premiere,
	Network,
	ComingSoon,
}

impl Group {
	pub const ALL: [Group; 12] = [
		Group::Apps,
		Group::Keyboard,
		Group::Media,
		Group::System,
		Group::DisplayPower,
		Group::Pages,
		Group::Flows,
		Group::Device,
		Group::Browser,
		Group::Premiere,
		Group::Network,
		Group::ComingSoon,
	];

	pub fn name(self) -> &'static str {
		match self {
			Group::Apps => "Apps & Websites",
			Group::Keyboard => "Keyboard & Text",
			Group::Media => "Media & Audio",
			Group::System => "System",
			Group::DisplayPower => "Display & Power",
			Group::Pages => "Pages & Folders",
			Group::Flows => "Action Flows",
			Group::Device => "M18 Device",
			Group::Browser => "Browser",
			Group::Premiere => "Premiere Pro",
			Group::Network => "Network",
			Group::ComingSoon => "Coming Soon",
		}
	}
}

/// How an action appears in the library.
pub struct Entry {
	pub uuid: &'static str,
	pub group: Group,
	pub name: &'static str,
	pub tooltip: &'static str,
	/// Key artwork in `static/keys/`.
	pub face: &'static str,
	/// `false` for duplicates and retired actions: they stay available to
	/// imported profiles but are not offered for new keys.
	pub listed: bool,
}

const fn listed(uuid: &'static str, group: Group, name: &'static str, tooltip: &'static str, face: &'static str) -> Entry {
	Entry {
		uuid,
		group,
		name,
		tooltip,
		face,
		listed: true,
	}
}

const fn hidden(uuid: &'static str, group: Group, name: &'static str, tooltip: &'static str, face: &'static str) -> Entry {
	Entry {
		uuid,
		group,
		name,
		tooltip,
		face,
		listed: false,
	}
}

use crate::m18_actions as native;
use Group::*;

/// Every built-in action, grouped and in the order the library lists them.
/// Hidden duplicates follow the listed actions of their group.
pub const LIBRARY: &[Entry] = &[
	// Apps & websites
	listed(native::OPEN_APPS_UUID, Apps, "Open App", "Launch or switch to an app; the key shows the app's icon", "open-app"),
	listed(
		"com.hotspot.streamdock.system.open",
		Apps,
		"Open File or Folder",
		"Open a file, folder, or app with its default handler",
		"open-file",
	),
	listed(
		"com.hotspot.streamdock.system.website",
		Apps,
		"Open Website",
		"Open a web address in the default browser",
		"open-website",
	),
	listed("com.hotspot.streamdock.system.close", Apps, "Quit App", "Quit an app by name", "quit-app"),
	listed("com.hotspot.streamdock.quicktool.calculator", Apps, "Calculator", "Open Calculator", "calculator"),
	listed("com.hotspot.streamdock.quicktool.mail", Apps, "Mail", "Open Mail", "mail"),
	listed("com.hotspot.streamdock.quicktool.music", Apps, "Music", "Open Music", "music-app"),
	listed(
		"com.hotspot.streamdock.hotkey.quicktool.taskmanager",
		Apps,
		"Activity Monitor",
		"Open Activity Monitor",
		"activity-monitor",
	),
	listed("com.hotspot.streamdock.quicktool.controlpanel", Apps, "System Settings", "Open System Settings", "system-settings"),
	listed("com.hotspot.streamdock.quicktool.homepage", Apps, "Safari", "Open Safari", "safari"),
	hidden("com.hotspot.streamdock.system.openApps", Apps, "Open App (VSD Craft)", "Open an app", "open-app"),
	// Keyboard & text
	listed("com.hotspot.streamdock.system.hotkey", Keyboard, "Hotkey", "Press a keyboard shortcut", "hotkey"),
	listed(native::HOTKEY_SWITCH_UUID, Keyboard, "Hotkey Switch", "Alternate between shortcuts on each press", "hotkey-switch"),
	listed(
		native::SUPER_HOTKEYS_UUID,
		Keyboard,
		"Super Hotkey",
		"Run one key sequence on press and another on release",
		"super-hotkey",
	),
	listed(
		native::SUPER_HOTKEY_SWITCH_UUID,
		Keyboard,
		"Super Hotkey Switch",
		"Alternate between press/release sequences on each press",
		"super-hotkey-switch",
	),
	listed("com.hotspot.streamdock.system.text", Keyboard, "Type Text", "Type a saved piece of text", "type-text"),
	listed("com.hotspot.streamdock.system.password", Keyboard, "Type Password", "Type a saved password", "password"),
	listed("com.mirabox.streamdock.emoji.emoji", Keyboard, "Emoji", "Type a saved emoji, or open the emoji picker", "emoji"),
	listed("com.hotspot.streamdock.mouse.event", Keyboard, "Mouse Action", "Click, move, scroll, or drag the pointer", "mouse"),
	hidden("com.mirabox.streamdock.emoji.emoji_send", Keyboard, "Send Emoji", "Type a saved emoji", "emoji"),
	hidden(
		"com.hotspot.streamdock.system.hotkeySwitch",
		Keyboard,
		"Hotkey Switch (VSD Craft)",
		"Alternate between shortcuts on each press",
		"hotkey-switch",
	),
	hidden(
		"com.hotspot.streamdock.system.super.hotkey",
		Keyboard,
		"Super Hotkey (VSD Craft)",
		"Run one key sequence on press and another on release",
		"super-hotkey",
	),
	// Media & audio
	listed(native::PLAY_PAUSE_UUID, Media, "Play / Pause", "Play or pause whatever is playing", "play-pause"),
	listed(native::PREVIOUS_TRACK_UUID, Media, "Previous Track", "Skip to the previous track", "previous-track"),
	listed(native::NEXT_TRACK_UUID, Media, "Next Track", "Skip to the next track", "next-track"),
	listed(
		"com.hotspot.streamdock.touchbar.fastforward",
		Media,
		"Fast Forward",
		"Skip ahead in the current track or video",
		"fast-forward",
	),
	listed("com.hotspot.streamdock.touchbar.fastrewind", Media, "Rewind", "Skip back in the current track or video", "rewind"),
	listed(native::VOLUME_UP_UUID, Media, "Volume Up", "Raise the output volume", "volume-up"),
	listed(native::VOLUME_DOWN_UUID, Media, "Volume Down", "Lower the output volume", "volume-down"),
	listed(native::MUTE_UUID, Media, "Mute", "Mute or unmute the output volume", "mute"),
	listed(
		"com.hotspot.streamdock.quickcontrol.microphone",
		Media,
		"Microphone Mute",
		"Mute or unmute the default microphone",
		"microphone",
	),
	listed(
		"com.hotspot.streamdock.system.multimedia",
		Media,
		"Media Key",
		"Send any media key: play, pause, stop, skip, or volume",
		"media-key",
	),
	listed(
		"com.hotspot.streamdock.soundboard.playaudio",
		Media,
		"Play Sound",
		"Play an audio file, with volume, looping, and fades",
		"play-sound",
	),
	listed(
		"com.hotspot.streamdock.soundboard.stopaudioplay",
		Media,
		"Stop All Sounds",
		"Stop every sound started by Play Sound",
		"stop-sounds",
	),
	hidden(
		"com.hotspot.streamdock.touchbar.playpause",
		Media,
		"Play / Pause (VSD Craft)",
		"Play or pause whatever is playing",
		"play-pause",
	),
	hidden("com.hotspot.streamdock.touchbar.nexttrack", Media, "Next Track (VSD Craft)", "Skip to the next track", "next-track"),
	hidden(
		"com.hotspot.streamdock.touchbar.previoustrack",
		Media,
		"Previous Track (VSD Craft)",
		"Skip to the previous track",
		"previous-track",
	),
	hidden("com.hotspot.streamdock.touchbar.volumeup", Media, "Volume Up (VSD Craft)", "Raise the output volume", "volume-up"),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.volumeup",
		Media,
		"Increase the Volume (VSD Craft)",
		"Raise the output volume",
		"volume-up",
	),
	hidden("com.hotspot.streamdock.touchbar.volumedown", Media, "Volume Down (VSD Craft)", "Lower the output volume", "volume-down"),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.volumedown",
		Media,
		"Lower the Volume (VSD Craft)",
		"Lower the output volume",
		"volume-down",
	),
	hidden("com.hotspot.streamdock.touchbar.mute", Media, "Mute (VSD Craft)", "Mute or unmute the output volume", "mute"),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.mute",
		Media,
		"Mute (VSD Craft Quick Control)",
		"Mute or unmute the output volume",
		"mute",
	),
	// System
	listed(native::DISPATCH_CENTER_UUID, System, "Mission Control", "Show all open windows", "mission-control"),
	listed(native::LAUNCHPAD_UUID, System, "Launchpad", "Open Launchpad", "launchpad"),
	listed("com.hotspot.streamdock.hotkey.quicktool.searchbar", System, "Spotlight", "Open Spotlight search", "spotlight"),
	listed(native::SIRI_UUID, System, "Siri", "Open Siri", "siri"),
	listed(native::SCREENSHOT_UUID, System, "Screenshot", "Capture a selected area to the clipboard", "screenshot"),
	listed(
		"com.hotspot.streamdock.touchbar.showdesktop",
		System,
		"Show Desktop",
		"Move windows aside to show the desktop",
		"show-desktop",
	),
	listed("com.hotspot.streamdock.touchbar.dictation", System, "Dictation", "Start dictation", "dictation"),
	listed(
		"com.hotspot.streamdock.touchbar.inputmethod",
		System,
		"Switch Input Source",
		"Switch to the next keyboard input source",
		"input-source",
	),
	listed(
		"com.hotspot.streamdock.hotkey.quicktool.emoticons",
		System,
		"Emoji & Symbols",
		"Open the Emoji & Symbols viewer",
		"emoji-viewer",
	),
	listed("com.hotspot.streamdock.touchbar.dndmode", System, "Focus Settings", "Open Focus (Do Not Disturb) settings", "focus"),
	listed(
		"com.hotspot.streamdock.touchbar.notificationcenter",
		System,
		"Notification Settings",
		"Open Notification settings",
		"notifications",
	),
	hidden("com.hotspot.streamdock.touchbar.siri", System, "Siri (VSD Craft)", "Open Siri", "siri"),
	hidden(
		"com.hotspot.streamdock.touchbar.dispatchcenter",
		System,
		"Mission Control (VSD Craft)",
		"Show all open windows",
		"mission-control",
	),
	hidden("com.hotspot.streamdock.touchbar.launchpad", System, "Launchpad (VSD Craft)", "Open Launchpad", "launchpad"),
	hidden(
		"com.hotspot.streamdock.touchbar.screenshot",
		System,
		"Screenshot (VSD Craft)",
		"Capture a selected area to the clipboard",
		"screenshot",
	),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.displaydesktop",
		System,
		"Display Desktop (VSD Craft)",
		"Show the desktop",
		"show-desktop",
	),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.speechrecognition",
		System,
		"Speech Recognition (VSD Craft)",
		"Start dictation",
		"dictation",
	),
	hidden(
		"com.hotspot.streamdock.hotkey.quicktool.notification",
		System,
		"Notification (VSD Craft)",
		"Open Notification settings",
		"notifications",
	),
	hidden(
		"com.hotspot.streamdock.hotkey.quickcontrol.switchscreen",
		System,
		"Switch Screen (VSD Craft)",
		"Send Control-F1",
		"show-desktop",
	),
	hidden(
		"com.mirabox.streamdock.emoticons.lib",
		System,
		"Emoticons Library (VSD Craft)",
		"Open the Emoji & Symbols viewer",
		"emoji-viewer",
	),
	// Display & power
	listed(native::SCREEN_BRIGHTNESS_UP_UUID, DisplayPower, "Brightness Up", "Brighten the Mac display", "brightness-up"),
	listed(native::SCREEN_BRIGHTNESS_DOWN_UUID, DisplayPower, "Brightness Down", "Dim the Mac display", "brightness-down"),
	listed(native::SLEEP_UUID, DisplayPower, "Sleep Displays", "Turn off the Mac's displays", "display-sleep"),
	listed(native::DESKTOP_SAVER_UUID, DisplayPower, "Screen Saver", "Start the macOS screen saver", "screen-saver"),
	listed("com.hotspot.streamdock.touchbar.screenlock", DisplayPower, "Lock Screen", "Lock the Mac", "lock-screen"),
	hidden(
		"com.hotspot.streamdock.touchbar.increasescreenbrightness",
		DisplayPower,
		"Increase Screen Brightness (VSD Craft)",
		"Brighten the Mac display",
		"brightness-up",
	),
	hidden(
		"com.hotspot.streamdock.touchbar.decreasescreenbrightness",
		DisplayPower,
		"Decrease Screen Brightness (VSD Craft)",
		"Dim the Mac display",
		"brightness-down",
	),
	hidden(
		"com.hotspot.streamdock.touchbar.desktopsaver",
		DisplayPower,
		"Desktop Saver (VSD Craft)",
		"Start the macOS screen saver",
		"screen-saver",
	),
	hidden(
		"com.hotspot.streamdock.touchbar.sleep",
		DisplayPower,
		"Sleep (VSD Craft Touch Bar)",
		"Turn off the Mac's displays",
		"display-sleep",
	),
	hidden(
		"com.hotspot.streamdock.quickcontrol.sleep",
		DisplayPower,
		"Sleep (VSD Craft Quick Control)",
		"Turn off the Mac's displays",
		"display-sleep",
	),
	hidden(
		"com.hotspot.streamdock.device.devsleep",
		DisplayPower,
		"Device Sleep (VSD Craft)",
		"Turn off the Mac's displays",
		"display-sleep",
	),
	// Pages & folders
	listed(native::PAGE_NEXT_UUID, Pages, "Next Page", "Show the next M18 page", "page-next"),
	listed(native::PAGE_PREVIOUS_UUID, Pages, "Previous Page", "Show the previous M18 page", "page-previous"),
	listed(native::PAGE_GOTO_UUID, Pages, "Go to Page", "Jump straight to a chosen page", "page-goto"),
	listed(native::PAGE_INDICATOR_UUID, Pages, "Page Number", "Show the number of the current page", "page-indicator"),
	listed(
		"com.hotspot.streamdock.profile.openchild",
		Pages,
		"Open Folder",
		"Open a page as a folder; Go Back returns",
		"open-folder",
	),
	listed("com.hotspot.streamdock.profile.backtoparent", Pages, "Go Back", "Return to the page that opened this folder", "go-back"),
	listed("com.hotspot.streamdock.profile.rotate", Pages, "Scene Shift", "Switch to a chosen page, or the next one", "scene-shift"),
	hidden("com.hotspot.streamdock.page.next", Pages, "Next Page (VSD Craft)", "Show the next M18 page", "page-next"),
	hidden(
		"com.hotspot.streamdock.page.previous",
		Pages,
		"Previous Page (VSD Craft)",
		"Show the previous M18 page",
		"page-previous",
	),
	hidden("com.hotspot.streamdock.page.goto", Pages, "Go to Page (VSD Craft)", "Jump straight to a chosen page", "page-goto"),
	hidden(
		"com.hotspot.streamdock.page.indicator",
		Pages,
		"Page Indicator (VSD Craft)",
		"Show the number of the current page",
		"page-indicator",
	),
	// Action flows
	listed("opendeck.multiaction", Flows, "Multi Action", "Run several actions in order with one press", "multi-action"),
	listed("opendeck.toggleaction", Flows, "Action Cycle", "Each press runs the next action in the list", "action-cycle"),
	listed(
		"opendeck.carouselaction",
		Flows,
		"Action Carousel",
		"Each press runs one action, then rotates to the next",
		"action-carousel",
	),
	listed("com.hotspot.streamdock.multiactions.delay", Flows, "Delay", "Wait between the steps of a Multi Action", "delay"),
	// M18 device
	listed(crate::m18::LED_ACTION_UUID, Device, "LED Colors", "Set the colors of the M18's 24 LEDs", "led-colors"),
	listed(
		"com.hotspot.streamdock.device.brightness",
		Device,
		"M18 Brightness",
		"Make the M18 screen brighter or dimmer",
		"m18-brightness",
	),
	hidden(
		"com.mirabox.streamdock.screensaver.action1",
		Device,
		"Screensaver 1 (VSD Craft)",
		"Not supported: the M18 uses display sleep instead",
		"coming-soon",
	),
	hidden(
		"com.mirabox.streamdock.screensaver.action2",
		Device,
		"Screensaver 2 (VSD Craft)",
		"Not supported: the M18 uses display sleep instead",
		"coming-soon",
	),
	// Browser
	listed("com.hotspot.streamdock.hotkey.browser.back", Browser, "Back", "Go back a page (⌘[)", "browser-back"),
	listed("com.hotspot.streamdock.hotkey.browser.forward", Browser, "Forward", "Go forward a page (⌘])", "browser-forward"),
	listed("com.hotspot.streamdock.hotkey.browser.refresh", Browser, "Reload", "Reload the page (⌘R)", "browser-reload"),
	listed("com.hotspot.streamdock.hotkey.browser.collect", Browser, "Bookmark Page", "Bookmark the current page (⌘D)", "bookmark"),
	// Premiere Pro
	listed("com.hotspot.streamdock.hotkey.pr.play", Premiere, "Play", "Play or stop the timeline (Space)", "pr-play"),
	listed("com.hotspot.streamdock.hotkey.pr.addEdit", Premiere, "Add Edit", "Split clips at the playhead (⌘K)", "pr-add-edit"),
	listed("com.hotspot.streamdock.hotkey.pr.razor", Premiere, "Razor Tool", "Select the Razor tool (C)", "pr-razor"),
	listed(
		"com.hotspot.streamdock.hotkey.pr.rippleEditingTools",
		Premiere,
		"Ripple Edit Tool",
		"Select the Ripple Edit tool (B)",
		"pr-ripple",
	),
	listed("com.hotspot.streamdock.hotkey.pr.penTool", Premiere, "Pen Tool", "Select the Pen tool (P)", "pr-pen"),
	listed(
		"com.hotspot.streamdock.hotkey.pr.rectangleTool",
		Premiere,
		"Rectangle Tool",
		"Select the Rectangle tool (Q)",
		"pr-rectangle",
	),
	listed("com.hotspot.streamdock.hotkey.pr.addMarker", Premiere, "Add Marker", "Add a marker (M)", "pr-marker"),
	listed("com.hotspot.streamdock.hotkey.pr.effectsPanel", Premiere, "Effects Panel", "Show the Effects panel (⇧7)", "pr-effects"),
	listed("com.hotspot.streamdock.pr.action5", Premiere, "Fit Timeline", "Zoom the timeline to fit the sequence (\\)", "pr-fit"),
	listed(
		"com.hotspot.streamdock.hotkey.pr.toggleFullScreen",
		Premiere,
		"Toggle Full Screen",
		"Toggle full-screen playback (⌃`)",
		"pr-fullscreen",
	),
	listed("com.hotspot.streamdock.hotkey.pr.cut", Premiere, "Cut", "Cut (⌘X)", "pr-cut"),
	listed("com.hotspot.streamdock.hotkey.pr.copy", Premiere, "Copy", "Copy (⌘C)", "pr-copy"),
	listed("com.hotspot.streamdock.hotkey.pr.paste", Premiere, "Paste", "Paste (⌘V)", "pr-paste"),
	listed("com.hotspot.streamdock.hotkey.pr.delete", Premiere, "Delete", "Delete the selection (⌦)", "pr-delete"),
	listed("com.hotspot.streamdock.hotkey.pr.undo", Premiere, "Undo", "Undo (⌘Z)", "pr-undo"),
	// Network
	listed("com.hotspot.streamdock.network.udp", Network, "UDP Message", "Send a text message to a UDP address", "udp"),
	// Not implemented yet
	listed("com.mirabox.streamdock.time.action2", ComingSoon, "Timer", "Count up from zero; press to start or pause", "timer"),
	listed(
		"com.mirabox.streamdock.time.action3",
		ComingSoon,
		"Countdown",
		"Count down to zero; press to start or pause",
		"countdown",
	),
	listed("com.mirabox.streamdock.time.action1", ComingSoon, "World Clock", "Show the time in another time zone", "world-clock"),
	listed("com.mirabox.streamdock.dateTime.action1", ComingSoon, "Date & Time", "Show the current date and time", "date-time"),
	listed("com.mirabox.streamdock.calendar.action1", ComingSoon, "Calendar", "Show today's date", "calendar"),
	listed("com.hotspot.streamdock.weather.action1", ComingSoon, "Weather", "Show the weather for a city", "weather"),
	listed("com.hotspot.streamdock.memo.action1", ComingSoon, "Notes", "Keep short notes on a key", "notes"),
	listed("com.hotspot.streamdock.memo.action2", ComingSoon, "To-Do List", "Keep a to-do list on a key", "todo"),
	listed(
		"com.hotspot.streamdock.youtube.chatmessage",
		ComingSoon,
		"YouTube Chat Message",
		"Post a saved message to a YouTube live chat",
		"youtube-chat",
	),
	listed(
		"com.hotspot.streamdock.youtube.viewers",
		ComingSoon,
		"YouTube Viewers",
		"Show a YouTube live stream's viewer count",
		"youtube-viewers",
	),
	listed("com.hotspot.streamdock.vmix.shortcut", ComingSoon, "vMix Shortcut", "Trigger a vMix shortcut", "vmix"),
	listed(
		"com.hotspot.streamdock.touchbar.focusonsearch",
		ComingSoon,
		"Focus Search",
		"Move focus to the search field",
		"focus-search",
	),
	listed("com.mirabox.streamdock.emoticons.select", ComingSoon, "Emoticon Stickers", "Send an animated emoticon", "stickers"),
	listed(
		"com.mirabox.streamdock.pigmenteffects.action1",
		ComingSoon,
		"One-Button Paint",
		"Animated paint effect on a key",
		"paint",
	),
	listed(
		"com.mirabox.streamdock.pigmenteffects.action2",
		ComingSoon,
		"Full-Screen Paint",
		"Animated paint effect across the M18",
		"paint-full",
	),
	listed("com.streamdock.musicalrhythma.action1", ComingSoon, "Musical Rhythm", "Animated audio visualiser", "rhythm"),
	listed("com.mirabox.streamdock.eatgoldcoins.action1", ComingSoon, "Eat Gold Coins", "A small key game", "game"),
	listed("com.mirabox.streamdock.watertank.action1", ComingSoon, "Water Tank", "An animated water-tank effect", "water"),
	hidden("com.mirabox.streamdock.emoticons.select2", ComingSoon, "Emoticon Stickers 2", "Send an animated emoticon", "stickers"),
];

pub fn face_path(face: &str) -> String {
	format!("opendeck/keys/{face}.svg")
}

/// Whether an image is an app icon that an earlier version generated: exactly
/// 256×256. Images chosen in the editor are stored at 288×288.
fn is_baked_app_icon(image: &str) -> bool {
	if image.is_empty() || image.starts_with("opendeck/") {
		return false;
	}
	if let Some((_, data)) = image.strip_prefix("data:").and_then(|rest| rest.split_once(";base64,")) {
		return base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
			.ok()
			.and_then(|bytes| image::load_from_memory(&bytes).ok())
			.is_some_and(|decoded| (decoded.width(), decoded.height()) == (256, 256));
	}
	image::image_dimensions(image).is_ok_and(|dimensions| dimensions == (256, 256))
}

/// Placeholder artwork that earlier versions gave every built-in key.
const LEGACY_PLACEHOLDERS: [&str; 3] = ["opendeck/multi-action.png", "opendeck/toggle-action.png", "opendeck/led-colors.svg"];

/// Give built-in keys that still show an old placeholder their action's own
/// artwork. Images the user chose are never touched. Returns whether anything
/// changed.
pub fn refresh_default_artwork(instance: &mut crate::shared::ActionInstance) -> bool {
	let mut changed = false;
	if let Some(entry) = entry(&instance.action.uuid) {
		let face = face_path(entry.face);
		// Earlier versions copied the app's icon into every Open App state (as a
		// 256-pixel PNG), which also overwrote any image the user chose. Those
		// copies go back to automatic, so the icon follows the app again.
		let baked_app_icons = instance.action.uuid == crate::m18_actions::OPEN_APPS_UUID;
		let images = std::iter::once(&mut instance.action.icon)
			.chain(instance.action.states.iter_mut().map(|state| &mut state.image))
			.chain(instance.states.iter_mut().map(|state| &mut state.image));
		for image in images {
			if LEGACY_PLACEHOLDERS.contains(&image.as_str()) || (baked_app_icons && is_baked_app_icon(image)) {
				image.clone_from(&face);
				changed = true;
			}
		}
	}
	for child in instance.children.iter_mut().flatten() {
		changed |= refresh_default_artwork(child);
	}
	changed
}

pub fn entry(uuid: &str) -> Option<&'static Entry> {
	LIBRARY.iter().find(|entry| entry.uuid.eq_ignore_ascii_case(uuid))
}

/// Whether the action does nothing yet (it is kept for imported profiles).
pub fn is_coming_soon(uuid: &str) -> bool {
	entry(uuid).is_some_and(|entry| entry.group == Group::ComingSoon) || uuid.to_ascii_lowercase().starts_with("com.mirabox.streamdock.screensaver.")
}

/// The library's default appearance for one state of an action: its key
/// artwork with no title. Switch actions start each new shortcut from it.
pub fn default_state(uuid: &str, index: usize) -> Option<ActionState> {
	let entry = entry(uuid)?;
	Some(ActionState {
		image: face_path(entry.face),
		name: format!("{} {}", entry.name, index + 1),
		// Titles sit below the icon on built-in artwork.
		alignment: "bottom".to_owned(),
		..Default::default()
	})
}

fn action(entry: &Entry, plugin: &str, state_count: usize, supported_in_multi_actions: bool) -> Action {
	let image = face_path(entry.face);
	let state_count = state_count.max(1);
	Action {
		name: entry.name.to_owned(),
		uuid: entry.uuid.to_owned(),
		plugin: plugin.to_owned(),
		tooltip: entry.tooltip.to_owned(),
		icon: image.clone(),
		disable_automatic_states: false,
		visible_in_action_list: entry.listed,
		supported_in_multi_actions,
		property_inspector: String::new(),
		controllers: vec!["Keypad".to_owned()],
		encoder: None,
		states: (0..state_count)
			.map(|index| ActionState {
				image: image.clone(),
				name: if state_count > 1 { format!("{} {}", entry.name, index + 1) } else { entry.name.to_owned() },
				// Titles sit below the icon on built-in artwork.
				alignment: "bottom".to_owned(),
				..Default::default()
			})
			.collect(),
	}
}

/// Build the library: every group, in order, with its actions in the order
/// they are declared above.
pub fn categories() -> HashMap<String, Category> {
	let mut groups: HashMap<Group, Vec<Action>> = HashMap::new();
	let mut add = |group: Group, action: Action| groups.entry(group).or_default().push(action);

	for entry in LIBRARY {
		let built_action = if matches!(entry.uuid, "opendeck.multiaction" | "opendeck.toggleaction" | "opendeck.carouselaction") || entry.uuid == crate::m18::LED_ACTION_UUID {
			// Multi Action, Action Cycle, and LED Colors keep their historical
			// plugin ID; saved profiles identify them by it.
			let plugin = if entry.uuid == "opendeck.carouselaction" { "" } else { "opendeck" };
			action(entry, plugin, 1, false)
		} else if let Some(definition) = crate::vsd_actions::definition(entry.uuid) {
			let mut vsd = action(entry, "", definition.state_count, definition.supported_in_multi_actions);
			// Keep the catalogue's UUID spelling: imports match it exactly.
			vsd.uuid = definition.uuid.clone();
			vsd
		} else {
			let states = if native::is_switch_action(entry.uuid) { 2 } else { 1 };
			action(entry, "", states, true)
		};
		add(entry.group, built_action);
	}

	Group::ALL
		.iter()
		.filter_map(|group| {
			let actions = groups.remove(group)?;
			Some((group.name().to_owned(), Category { icon: None, actions }))
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_catalogue_action_has_exactly_one_library_entry() {
		for definition in crate::vsd_actions::definitions() {
			if crate::vsd_actions::is_composite_action(&definition.uuid) {
				continue;
			}
			let matches = LIBRARY.iter().filter(|entry| entry.uuid.eq_ignore_ascii_case(&definition.uuid)).count();
			assert_eq!(matches, 1, "{} should have exactly one library entry", definition.uuid);
		}
		// Everything else in the library is a built-in core action.
		for entry in LIBRARY.iter().filter(|entry| crate::vsd_actions::definition(entry.uuid).is_none()) {
			assert!(
				entry.uuid.starts_with("opendeck.")
					&& (native::is_native_action(entry.uuid) || entry.uuid == crate::m18::LED_ACTION_UUID || entry.uuid.starts_with("opendeck.") && entry.group == Group::Flows),
				"{} is neither a VSD catalogue action nor a built-in action",
				entry.uuid
			);
		}
	}

	#[test]
	fn every_native_action_is_in_the_library() {
		for uuid in [
			native::OPEN_APPS_UUID,
			native::SUPER_HOTKEYS_UUID,
			native::HOTKEY_SWITCH_UUID,
			native::SUPER_HOTKEY_SWITCH_UUID,
			native::VOLUME_DOWN_UUID,
			native::VOLUME_UP_UUID,
			native::MUTE_UUID,
			native::SIRI_UUID,
			native::DISPATCH_CENTER_UUID,
			native::SCREENSHOT_UUID,
			native::LAUNCHPAD_UUID,
			native::DESKTOP_SAVER_UUID,
			native::SLEEP_UUID,
			native::SCREEN_BRIGHTNESS_UP_UUID,
			native::SCREEN_BRIGHTNESS_DOWN_UUID,
			native::PREVIOUS_TRACK_UUID,
			native::PLAY_PAUSE_UUID,
			native::NEXT_TRACK_UUID,
			native::PAGE_PREVIOUS_UUID,
			native::PAGE_NEXT_UUID,
			native::PAGE_GOTO_UUID,
			native::PAGE_INDICATOR_UUID,
		] {
			assert!(entry(uuid).is_some_and(|entry| entry.listed), "{uuid} should be listed");
		}
	}

	#[test]
	fn each_function_is_listed_once() {
		let categories = categories();
		let mut names = std::collections::HashSet::new();
		for (group, category) in &categories {
			for action in category.actions.iter().filter(|action| action.visible_in_action_list) {
				assert!(names.insert((group.clone(), action.name.clone())), "{} is listed twice in {group}", action.name);
			}
		}
		let listed = categories
			.iter()
			.filter(|(group, _)| group.as_str() != Group::ComingSoon.name())
			.flat_map(|(_, category)| &category.actions)
			.filter(|action| action.visible_in_action_list)
			.count();
		assert_eq!(listed, 79, "the curated library lists every working action once");
	}

	#[test]
	fn imports_can_still_find_hidden_duplicates() {
		let categories = categories();
		let all = categories.values().flat_map(|category| &category.actions).collect::<Vec<_>>();
		for uuid in [
			"com.hotspot.streamdock.touchbar.volumeup",
			"com.hotspot.streamdock.page.goto",
			"com.hotspot.streamdock.system.hotkeySwitch",
		] {
			let action = all.iter().find(|action| action.uuid == uuid).expect("hidden duplicate stays in the catalogue");
			assert!(!action.visible_in_action_list);
		}
	}

	#[test]
	fn every_library_entry_has_key_artwork() {
		let keys = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../static/keys");
		for entry in LIBRARY {
			assert!(keys.join(format!("{}.svg", entry.face)).is_file(), "missing static/keys/{}.svg", entry.face);
		}
		assert!(keys.join("unsupported.svg").is_file(), "missing the unsupported-action artwork");
	}

	#[test]
	fn unfinished_actions_are_kept_apart() {
		assert!(is_coming_soon("com.hotspot.streamdock.weather.action1"));
		assert!(is_coming_soon("com.mirabox.streamdock.screensaver.action1"));
		assert!(!is_coming_soon("com.hotspot.streamdock.system.hotkey"));
	}

	#[test]
	fn old_placeholders_become_action_artwork_but_custom_images_stay() {
		let categories = categories();
		let volume = categories[Group::Media.name()].actions.iter().find(|action| action.uuid == native::VOLUME_UP_UUID).unwrap().clone();
		let mut instance = crate::shared::ActionInstance {
			action: Action {
				icon: "opendeck/multi-action.png".to_owned(),
				..volume.clone()
			},
			context: "18-test.Default.Keypad.0.0".parse().unwrap(),
			states: vec![
				ActionState {
					image: "opendeck/multi-action.png".to_owned(),
					..Default::default()
				},
				ActionState {
					image: "data:image/png;base64,Y3VzdG9t".to_owned(),
					..Default::default()
				},
			],
			current_state: 0,
			settings: serde_json::json!({}),
			children: None,
		};
		assert!(refresh_default_artwork(&mut instance));
		assert_eq!(instance.action.icon, "opendeck/keys/volume-up.svg");
		assert_eq!(instance.states[0].image, "opendeck/keys/volume-up.svg");
		assert_eq!(instance.states[1].image, "data:image/png;base64,Y3VzdG9t");
		assert!(!refresh_default_artwork(&mut instance), "a second pass changes nothing");
	}

	/// Writes the library for the editor's browser preview (`deno task dev`
	/// outside Tauri). Run with:
	/// OPENDECK_PREVIEW_LIBRARY=../src/lib/devPreviewLibrary.json cargo test export_preview_library -- --ignored
	#[test]
	#[ignore = "writes the editor preview fixture; run explicitly"]
	fn export_preview_library() {
		let path = std::env::var("OPENDECK_PREVIEW_LIBRARY").expect("set OPENDECK_PREVIEW_LIBRARY to the output path");
		std::fs::write(path, serde_json::to_string_pretty(&categories()).unwrap() + "\n").unwrap();
	}

	#[test]
	fn groups_list_actions_in_their_curated_order() {
		let categories = categories();
		let names = |group: Group| {
			categories[group.name()]
				.actions
				.iter()
				.filter(|action| action.visible_in_action_list)
				.map(|action| action.name.as_str())
				.collect::<Vec<_>>()
		};
		assert_eq!(names(Group::Apps)[..3], ["Open App", "Open File or Folder", "Open Website"]);
		assert_eq!(names(Group::Keyboard)[..2], ["Hotkey", "Hotkey Switch"]);
		assert_eq!(names(Group::Media)[..3], ["Play / Pause", "Previous Track", "Next Track"]);
		assert_eq!(names(Group::Flows), ["Multi Action", "Action Cycle", "Action Carousel", "Delay"]);
	}

	#[test]
	fn recorded_shortcut_sequences_are_valid_enigo_tokens() {
		// The exact shapes the editor's shortcut recorder saves (src/lib/shortcuts.ts).
		for sequence in [
			"[k(Meta,Press),r(40),k(Meta,Release)]",
			"[k(Control,Press),k(Alt,Press),r(122,Press)]",
			"[r(122,Release),k(Alt,Release),k(Control,Release)]",
			"[k(Meta,Press),k(Unicode('k'),Click),k(Meta,Release)]",
			"[k(Unicode('\\\\'),Click)]",
			"[k(Unicode('\\''),Click)]",
			"[k(F13,Click)]",
			"[k(Return,Click)]",
			"[k(Numpad1,Click)]",
			"[k(Add,Click)]",
			"[k(LeftArrow,Click)]",
		] {
			assert!(ron::from_str::<Vec<enigo::agent::Token>>(sequence).is_ok(), "{sequence} should parse");
		}
	}

	#[test]
	fn app_keys_show_the_app_icon_until_an_image_is_chosen() {
		use crate::m18_actions::{icon_target, uses_default_artwork};
		let app = serde_json::json!({ "appPath": "Safari" });
		assert_eq!(icon_target(native::OPEN_APPS_UUID, &app).as_deref(), Some("Safari"));
		assert_eq!(icon_target("com.hotspot.streamdock.quicktool.calculator", &serde_json::json!({})).as_deref(), Some("Calculator"));
		assert_eq!(
			icon_target("com.hotspot.streamdock.system.open", &serde_json::json!({ "path": "~/Documents" })).as_deref(),
			Some("~/Documents")
		);
		assert_eq!(icon_target("com.hotspot.streamdock.system.open", &serde_json::json!({ "path": "https://example.com" })), None);
		assert_eq!(icon_target(native::VOLUME_UP_UUID, &serde_json::json!({})), None);
		assert!(uses_default_artwork("opendeck/keys/open-app.svg"));
		assert!(!uses_default_artwork("data:image/png;base64,Y3VzdG9t"));
	}

	#[test]
	fn old_generated_app_icons_become_automatic_but_chosen_images_stay() {
		let encode = |size: u32| {
			let mut bytes = std::io::Cursor::new(Vec::new());
			image::DynamicImage::new_rgba8(size, size).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
			format!("data:image/png;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes.into_inner()))
		};
		assert!(is_baked_app_icon(&encode(256)));
		assert!(!is_baked_app_icon(&encode(288)));
		assert!(!is_baked_app_icon("opendeck/keys/open-app.svg"));
	}

	#[test]
	fn switch_actions_get_one_state_per_shortcut() {
		let categories = categories();
		let switch = categories[Group::Keyboard.name()].actions.iter().find(|action| action.uuid == native::HOTKEY_SWITCH_UUID).unwrap();
		assert_eq!(switch.states.len(), 2);
		assert_eq!(switch.icon, "opendeck/keys/hotkey-switch.svg");
	}
}
