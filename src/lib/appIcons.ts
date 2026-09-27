import type { ActionInstance } from "./ActionInstance.ts";
import type { ActionState } from "./ActionState.ts";

import { invoke } from "@tauri-apps/api/core";

/**
 * Actions that launch or open something, mirroring `icon_target()` in
 * `src-tauri/src/m18_actions.rs`. Their keys show the app's (or file's) own
 * icon until the user chooses an image.
 */
const LAUNCHING_ACTIONS = new Set([
	"opendeck.m18.open-apps",
	"com.hotspot.streamdock.system.openapps",
	"com.hotspot.streamdock.system.open",
	"com.hotspot.streamdock.quicktool.calculator",
	"com.hotspot.streamdock.quicktool.controlpanel",
	"com.hotspot.streamdock.quicktool.mail",
	"com.hotspot.streamdock.quicktool.music",
	"com.hotspot.streamdock.quicktool.taskmanager",
	"com.hotspot.streamdock.hotkey.quicktool.taskmanager",
	"com.hotspot.streamdock.quicktool.homepage",
	"com.hotspot.streamdock.touchbar.launchpad",
]);

export function launchesSomething(uuid: string): boolean {
	return LAUNCHING_ACTIONS.has(uuid.toLowerCase());
}

/** What the automatic icon belongs to, for labels such as "Use app icon". */
export function iconSource(uuid: string): "app" | "file" {
	return uuid.toLowerCase() === "com.hotspot.streamdock.system.open" ? "file" : "app";
}

/** Whether a state shows the action's automatic artwork rather than a chosen image. */
export function isDefaultArtwork(image: string | undefined): boolean {
	return !image || image.startsWith("opendeck/");
}

const cache = new Map<string, Promise<string | null>>();

/** The launching action's default icon, or null. Looked up once per target. */
export function actionIcon(uuid: string, settings: unknown): Promise<string | null> {
	if (!launchesSomething(uuid)) return Promise.resolve(null);
	const key = `${uuid}\u0000${JSON.stringify(settings ?? {})}`;
	let icon = cache.get(key);
	if (!icon) {
		icon = invoke<string | null>("get_action_icon", { uuid, settings: settings ?? {} }).catch(() => null);
		cache.set(key, icon);
		// Ask again later when nothing was found: the app may be installed meanwhile.
		icon.then((result) => {
			if (!result) setTimeout(() => cache.delete(key), 60_000);
		});
		if (cache.size > 300) cache.delete(cache.keys().next().value!);
	}
	return icon;
}

/**
 * The state as it should be drawn: a launching key without a chosen image
 * shows the app's icon, sized to leave room for a title along the bottom.
 * The key's own image scale still applies on top. Mirrors `app_icon_face()`
 * in `src-tauri/src/m18_actions.rs`, which draws the same layout on the M18.
 */
export async function resolveState(slot: Pick<ActionInstance, "action" | "settings">, state: ActionState): Promise<{ state: ActionState; appIcon: boolean }> {
	if (!isDefaultArtwork(state.image)) return { state, appIcon: false };
	const icon = await actionIcon(slot.action.uuid, slot.settings);
	if (!icon) return { state, appIcon: false };
	const titled = state.show && state.text.trim() !== "" && state.alignment === "bottom";
	const chosen = Math.max(10, state.image_scale || 100);
	return { state: { ...state, image: icon, image_scale: Math.round(((titled ? 62 : 84) * chosen) / 100) }, appIcon: true };
}
