/**
 * Browser preview of the editor, for working on the interface without the app
 * or an M18: run `deno task dev` and open http://localhost:5173 in a browser.
 *
 * It is only loaded in development builds outside Tauri (see
 * `src/hooks.client.ts`). Every backend call is answered from memory: nothing
 * touches the real configuration, plugins, or the device.
 */
import type { Action } from "./Action.ts";
import type { ActionInstance } from "./ActionInstance.ts";
import type { Profile } from "./Profile.ts";

import library from "./devPreviewLibrary.json";

import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const DEVICE = "18-PREVIEW";
const categories: Record<string, { icon: string | null; actions: Action[] }> = structuredClone(library) as any;
categories["OpenDeck"] = {
	icon: null,
	actions: [
		["Run Command", "com.amansprojects.starterpack.runcommand", "Run a command", "open-file"],
		["Open URL", "com.amansprojects.starterpack.openurl", "Open a URL", "open-website"],
		["Simulate Input", "com.amansprojects.starterpack.inputsimulation", "Simulate mouse or keyboard input", "hotkey"],
	].map(([name, uuid, tooltip, face]) => ({
		name,
		uuid,
		plugin: "com.amansprojects.starterpack.sdPlugin",
		tooltip,
		icon: `opendeck/keys/${face}.svg`,
		visible_in_action_list: true,
		supported_in_multi_actions: true,
		property_inspector: "",
		controllers: ["Keypad"],
		states: [{ image: `opendeck/keys/${face}.svg`, image_scale: 100, background_colour: "#000000", name: "", text: "", show: true, colour: "#FFFFFF", stroke_colour: "#000000", alignment: "bottom", family: "Liberation Sans", style: "Regular", size: 16, stroke_size: 3, underline: false }],
	})),
};

function findAction(uuid: string): Action {
	for (const category of Object.values(categories)) {
		const action = category.actions.find((candidate) => candidate.uuid === uuid);
		if (action) return action;
	}
	throw new Error(`Unknown preview action ${uuid}`);
}

function instance(profile: string, position: number, uuid: string, settings: Record<string, unknown> = {}, title = ""): ActionInstance {
	const action = findAction(uuid);
	const states = structuredClone(action.states).map((state) => ({ ...state, text: title || state.text, alignment: "bottom" as const, size: 13 }));
	return {
		action,
		context: `${DEVICE}.${profile}.Keypad.${position}.0`,
		states,
		current_state: 0,
		settings,
		children: ["opendeck.multiaction", "opendeck.toggleaction", "opendeck.carouselaction"].includes(uuid) ? [] : null,
	};
}

type Page = { id: string; name: string; profile: string };
const pageSet: { pages: Page[]; selected: number; folder_history: number[] } = {
	pages: [
		{ id: "Default", name: "", profile: "Default" },
		{ id: "Page 2", name: "Media", profile: "Page 2" },
		{ id: "Page 3", name: "Editing", profile: "Page 3" },
	],
	selected: 0,
	folder_history: [],
};

function emptyProfile(id: string): Profile {
	return { device: DEVICE, id, keys: Array(20).fill(null), sliders: [], infobars: [] };
}

const profiles: Record<string, Profile> = {};
function profile(id: string): Profile {
	return (profiles[id] ??= emptyProfile(id));
}

function seed() {
	const home = profile("Default");
	const layout: [number, string, Record<string, unknown>?, string?][] = [
		[0, "opendeck.m18.open-apps", { appPath: "Safari" }, "Safari"],
		[1, "opendeck.m18.open-apps", { appPath: "Mail" }, "Mail"],
		[2, "com.hotspot.streamdock.system.website", { path: "https://github.com" }, "GitHub"],
		[3, "com.hotspot.streamdock.hotkey.quicktool.searchbar"],
		[4, "opendeck.m18.page-next"],
		[5, "opendeck.m18.play-pause"],
		[6, "opendeck.m18.previous-track"],
		[7, "opendeck.m18.next-track"],
		[8, "opendeck.m18.volume-down"],
		[9, "opendeck.m18.volume-up"],
		[10, "com.hotspot.streamdock.system.hotkey", { down: "[k(Meta,Press),k(Shift,Press),r(1),k(Shift,Release),k(Meta,Release)]", display: "⌘⇧S" }, "Save As"],
		[11, "opendeck.m18.screenshot"],
		[12, "opendeck.m18.dispatch-center"],
		[13, "opendeck.m18.led-colors"],
		[15, "opendeck.m18.mute"],
		[16, "opendeck.m18.page-indicator"],
		[17, "opendeck.m18.sleep"],
	];
	for (const [position, uuid, settings, title] of layout) home.keys[position] = instance("Default", position, uuid, settings ?? {}, title ?? "");
	const flow = instance("Default", 14, "opendeck.multiaction", { delays: [250] }, "Focus");
	flow.children = [
		{ ...instance("Default", 14, "com.hotspot.streamdock.touchbar.dndmode"), context: `${DEVICE}.Default.Keypad.14.1` },
		{ ...instance("Default", 14, "opendeck.m18.open-apps", { appPath: "Music" }), context: `${DEVICE}.Default.Keypad.14.2` },
	];
	home.keys[14] = flow;

	const media = profile("Page 2");
	["opendeck.m18.play-pause", "opendeck.m18.previous-track", "opendeck.m18.next-track", "com.hotspot.streamdock.soundboard.playaudio", "opendeck.m18.page-previous"].forEach((uuid, position) => {
		media.keys[position] = instance("Page 2", position, uuid);
	});
	const editing = profile("Page 3");
	const toggle = instance("Page 3", 0, "opendeck.m18.hotkey-switch", {
		hotkeys: [
			{ down: "[k(Meta,Press),k(Shift,Press),t(\"s\"),k(Shift,Release),k(Meta,Release)]", up: "", display: "⌘⇧S" },
			{ down: "[k(Meta,Press),k(Shift,Press),t(\"e\"),k(Shift,Release),k(Meta,Release)]", up: "", display: "⌘⇧E" },
		],
		index: 0,
	});
	toggle.states[0].text = "START SCRIPT";
	toggle.states[1].text = "END SCRIPT";
	editing.keys[0] = toggle;
}
seed();

const SWITCH_ACTIONS = ["opendeck.m18.hotkey-switch", "opendeck.m18.super-hotkey-switch"];

// Mirrors `match_switch_states()` in the core: one state per shortcut.
function matchSwitchStates(target: ActionInstance) {
	const count = Math.max(1, (target.settings as any)?.hotkeys?.length ?? 0);
	const fresh = () => ({ ...structuredClone(findAction(target.action.uuid).states[0]), text: "", alignment: "bottom" as const });
	target.states = target.states.slice(0, count);
	while (target.states.length < count) target.states.push(fresh());
	target.current_state = Math.min(Number((target.settings as any)?.index ?? 0), count - 1);
}

// Stand-ins for the icons that macOS provides for installed apps.
function previewAppIcon(name: string): string | null {
	const colours: Record<string, [string, string]> = { Safari: ["#5AC8FA", "#0A84FF"], Mail: ["#6AC4FF", "#1D6BF3"], Music: ["#FF6482", "#FA2D48"] };
	const colour = colours[name];
	if (!colour) return null;
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${colour[0]}"/><stop offset="1" stop-color="${colour[1]}"/></linearGradient></defs><rect x="22" y="22" width="212" height="212" rx="50" fill="url(#g)"/><text x="128" y="163" font-family="-apple-system, Helvetica, sans-serif" font-size="104" font-weight="600" fill="#fff" text-anchor="middle">${name[0]}</text></svg>`;
	return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

let selectedProfile = "Default";
let applicationProfiles: Record<string, Record<string, string>> = { Safari: { [DEVICE]: "Default" }, Music: { [DEVICE]: "Page 2" } };
let settings = {
	version: "2.14.0",
	language: "en",
	brightness: 70,
	sleep_timeout_minutes: 10,
	sleep_when_computer_locked: true,
	rotation: 0,
	background: true,
	autolaunch: true,
	updatecheck: false,
	statistics: false,
	separatewine: false,
	developer: false,
	led_color: "#780000",
	led_brightness: 100,
	app_icon_scale: 100,
};

function parseContext(context: string) {
	const parts = context.split(".");
	const indices = parts.slice(4).map(Number);
	return {
		device: parts[0]!,
		profile: parts[1]!,
		controller: parts[2]!,
		position: Number(parts[3]),
		indices,
		index: indices[indices.length - 1] ?? 0,
		root: indices.length === 1 && indices[0] === 0,
	};
}

function findInstance(context: string): ActionInstance | undefined {
	const { profile: id, position } = parseContext(context);
	const slot = profile(id).keys[position];
	if (!slot) return undefined;
	function find(instance: ActionInstance): ActionInstance | undefined {
		if (instance.context === context) return instance;
		for (const child of instance.children ?? []) {
			const match = find(child);
			if (match) return match;
		}
		return undefined;
	}
	return find(slot);
}

function switchTo(index: number) {
	pageSet.selected = Math.max(0, Math.min(index, pageSet.pages.length - 1));
	selectedProfile = pageSet.pages[pageSet.selected].profile;
	void emit("m18_pages_changed", { device: DEVICE, pageSet: structuredClone(pageSet) });
}

async function handle(command: string, args: any): Promise<unknown> {
	switch (command) {
		case "get_port_base":
			return 57116;
		case "get_settings":
			return structuredClone(settings);
		case "set_settings":
			settings = args.settings;
			return null;
		case "get_localisations":
			return {};
		case "get_devices":
			return { [DEVICE]: { id: DEVICE, plugin: "", name: "VSD Inside M18", rows: 4, columns: 5, encoders: 0, touchpoints: 0, infobars: 0, type: 0 } };
		case "get_selected_profile":
			return structuredClone(profile(selectedProfile));
		case "set_selected_profile":
			selectedProfile = args.id;
			return null;
		case "get_profiles":
			return Object.keys(profiles);
		case "get_m18_pages":
			return structuredClone(pageSet);
		case "switch_m18_page_index":
			switchTo(args.index);
			return null;
		case "add_m18_page": {
			const id = `Page ${pageSet.pages.length + 1}`;
			profile(id);
			pageSet.pages.push({ id, name: "", profile: id });
			switchTo(pageSet.pages.length - 1);
			return structuredClone(pageSet);
		}
		case "rename_m18_page":
			pageSet.pages[args.index].name = String(args.name).trim().slice(0, 40);
			return structuredClone(pageSet);
		case "duplicate_m18_page": {
			const source = pageSet.pages[args.index];
			const id = `${source.profile} copy`;
			profiles[id] = structuredClone(profile(source.profile));
			profiles[id].id = id;
			pageSet.pages.splice(args.index + 1, 0, { id, name: source.name ? `${source.name} copy` : "", profile: id });
			return structuredClone(pageSet);
		}
		case "delete_m18_page": {
			if (pageSet.pages.length <= 1) throw "The M18 needs at least one page";
			const [removed] = pageSet.pages.splice(args.index, 1);
			delete profiles[removed.profile];
			switchTo(Math.min(args.index, pageSet.pages.length - 1));
			return structuredClone(pageSet);
		}
		case "move_m18_page": {
			const current = pageSet.pages[pageSet.selected].profile;
			const [page] = pageSet.pages.splice(args.from, 1);
			pageSet.pages.splice(args.to, 0, page);
			pageSet.selected = pageSet.pages.findIndex((candidate) => candidate.profile === current);
			return structuredClone(pageSet);
		}
		case "get_categories":
			return structuredClone(categories);
		case "list_plugins":
			return [{ id: "com.amansprojects.starterpack.sdPlugin", name: "OpenDeck Starter Pack", author: "nekename", icon: "", version: "3.0.0", builtin: true, registered: true, has_settings_interface: false }];
		case "create_instance": {
			const { context, action } = args;
			const target = profile(context.profile);
			const slot = target.keys[context.position];
			if (slot?.children) {
				const child = { ...instance(context.profile, context.position, action.uuid), context: `${DEVICE}.${context.profile}.Keypad.${context.position}.${slot.children.length + 1}` };
				slot.children.push(child);
				return structuredClone(slot);
			}
			if (slot) return null;
			target.keys[context.position] = instance(context.profile, context.position, action.uuid);
			return structuredClone(target.keys[context.position]);
		}
		case "create_child_instance": {
			const parent = findInstance(args.parentContext);
			if (!parent?.children) return null;
			const child = {
				...instance(parent.context.split(".")[1], parseContext(parent.context).position, args.action.uuid),
				context: `${parent.context}.${parent.children.length + 1}`,
				children: ["opendeck.multiaction", "opendeck.toggleaction", "opendeck.carouselaction"].includes(args.action.uuid) ? [] : null,
			};
			parent.children.push(child);
			return structuredClone(profile(parseContext(parent.context).profile).keys[parseContext(parent.context).position]);
		}
		case "remove_instance": {
			const { profile: id, position, root } = parseContext(args.context);
			const slot = profile(id).keys[position];
			if (root) profile(id).keys[position] = null;
			else {
				function remove(instance: ActionInstance): boolean {
					if (!instance.children) return false;
					const index = instance.children.findIndex((child) => child.context === args.context);
					if (index >= 0) {
						instance.children.splice(index, 1);
						return true;
					}
					return instance.children.some(remove);
				}
			if (slot) remove(slot);
			}
			return null;
		}
		case "move_instance": {
			const { source, destination, retain } = args;
			const moved = structuredClone(profile(source.profile).keys[source.position]);
			if (!moved || profile(destination.profile).keys[destination.position]) return null;
			moved.context = `${DEVICE}.${destination.profile}.Keypad.${destination.position}.0`;
			profile(destination.profile).keys[destination.position] = moved;
			if (!retain) profile(source.profile).keys[source.position] = null;
			return structuredClone(moved);
		}
		case "swap_m18_instances": {
			const { source, destination } = args;
			const keys = profile(source.profile).keys;
			const a = keys[source.position]!;
			const b = keys[destination.position]!;
			a.context = `${DEVICE}.${source.profile}.Keypad.${destination.position}.0`;
			b.context = `${DEVICE}.${source.profile}.Keypad.${source.position}.0`;
			keys[source.position] = b;
			keys[destination.position] = a;
			return { source: structuredClone(b), destination: structuredClone(a) };
		}
		case "set_state": {
			const target = findInstance(args.context);
			if (target) {
				target.states[args.index] = args.state;
				void emit("update_state", { context: args.context, contents: structuredClone(target) });
			}
			return null;
		}
		case "set_instance_settings":
		case "set_m18_led_palette": {
			const target = findInstance(args.context);
			if (target) {
				target.settings = command === "set_m18_led_palette" ? { ledColors: args.colors } : args.settings;
				if (SWITCH_ACTIONS.includes(target.action.uuid)) matchSwitchStates(target);
				void emit("update_state", { context: args.context, contents: structuredClone(target) });
			}
			return null;
		}
		case "remove_switch_shortcut": {
			const target = findInstance(args.context);
			const hotkeys = (target?.settings as any)?.hotkeys as unknown[] | undefined;
			if (!target || !hotkeys || hotkeys.length < 2) return null;
			hotkeys.splice(args.index, 1);
			target.states.splice(args.index, 1);
			const next = Number((target.settings as any).index ?? 0);
			(target.settings as any).index = args.index < next ? next - 1 : Math.min(next, hotkeys.length - 1);
			matchSwitchStates(target);
			void emit("update_state", { context: args.context, contents: structuredClone(target) });
			return structuredClone(target);
		}
		case "set_child_delay": {
			const target = findInstance(args.parentContext);
			if (!target) return null;
			const delays = [...(target.settings.delays ?? [])];
			delays[args.index] = args.delayMs;
			target.settings = { ...target.settings, delays };
			return structuredClone(target.settings);
		}
		case "trigger_virtual_press": {
			const context = { ...args.context };
			void emit("key_moved", { context, pressed: true });
			setTimeout(() => void emit("key_moved", { context, pressed: false }), 140);
			return null;
		}
		case "get_applications":
			return ["Safari", "Mail", "Music", "Final Cut Pro", "Xcode", "Figma"];
		case "get_application_profiles":
			return structuredClone(applicationProfiles);
		case "set_application_profiles":
			applicationProfiles = args.value;
			return null;
		case "get_audio_output_devices":
			return [
				{ id: "speakers", name: "MacBook Pro Speakers" },
				{ id: "studio", name: "Studio Display Speakers" },
			];
		case "get_mouse_position":
			return [960, 540];
		case "get_fonts":
			return ["Helvetica Neue", "SF Pro", "Menlo"];
		case "get_build_info":
			return "<details><summary>OpenDeck VSD M18 (browser preview) on aarch64-apple-darwin</summary>Preview data only: no device or configuration is used.</details>";
		case "get_input_permission":
			return true;
		case "get_profile":
			return structuredClone(profile(args.profile));
		case "check_for_updates":
			return { current: "2.15.3", latest: "2.15.3", newer: false, url: "https://github.com/nickhighland/OpenDeck-VSD-Inside-M18/releases/latest", notes: "" };
		case "get_action_icon":
			return previewAppIcon(String(args.settings?.appPath ?? ""));
		case "plugin:dialog|ask":
		case "plugin:dialog|confirm":
			return true;
		case "plugin:dialog|message":
		case "plugin:dialog|open":
		case "plugin:dialog|save":
			return null;
		default:
			// update_image, switch_property_inspector, restart, open_url, …
			return null;
	}
}

export function installPreviewMocks() {
	mockWindows("main");
	mockIPC((command, args) => handle(command, args), { shouldMockEvents: true });
	console.info("OpenDeck VSD M18: browser preview with simulated data");
}
