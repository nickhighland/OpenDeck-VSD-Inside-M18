import type { ActionInstance } from "./ActionInstance.ts";
import type { ActionState } from "./ActionState.ts";
import type { Context } from "./Context.ts";
import type { Profile } from "./Profile.ts";

import { resolveState } from "./appIcons.ts";
import { type M18PageSet, pageNumber, pageSets } from "./pages.ts";
import { renderImage } from "./rendererHelper.ts";
import { settings } from "./settings.ts";

import { invoke } from "@tauri-apps/api/core";
import { get } from "svelte/store";

/** The M18's LCD keys; the three bottom buttons have no display. */
const LCD_KEYS = 15;

/**
 * A key's current state as the key shows it: Go to Page and Page Number keys
 * draw their page number over their artwork.
 */
export function displayState(slot: ActionInstance, pageSet: M18PageSet | undefined, profile: string | undefined): ActionState | undefined {
	const state = slot.states[slot.current_state];
	if (!state) return undefined;
	const numberOverlay = { show: true, alignment: "middle" as const, size: 22, colour: "#ffffff", stroke_colour: "#000000", stroke_size: 2 };
	if (slot.action.uuid === "opendeck.m18.page-goto" && slot.settings?.showPageNumber !== false && !state.text.trim()) {
		return { ...state, ...numberOverlay, text: String(Number(slot.settings?.pageIndex ?? 0) + 1) };
	}
	if (slot.action.uuid === "opendeck.m18.page-indicator") {
		return { ...state, ...numberOverlay, text: String(pageNumber(pageSet, profile)) };
	}
	return state;
}

/**
 * Draw the keys of every page that is not on the M18 and hand the images to
 * the core, which saves them in the M18's format. A page turn then shows the
 * whole page at once instead of waiting for the editor to draw it. (The page
 * on the M18 is drawn by its keys in the editor anyway.)
 */
async function drawOtherPages(device: string) {
	const pageSet = get(pageSets)[device];
	if (!pageSet) return;
	const shownProfile = pageSet.pages[pageSet.selected]?.profile;
	for (const page of pageSet.pages) {
		if (page.profile === shownProfile) continue;
		const profile = await invoke<Profile | null>("get_profile", { device, profile: page.profile }).catch(() => null);
		for (const [position, slot] of (profile?.keys ?? []).entries()) {
			if (!slot || position >= LCD_KEYS) continue;
			const shown = displayState(slot, pageSet, page.profile);
			if (!shown) continue;
			const { state } = await resolveState(slot, shown);
			const context: Context = { device, profile: page.profile, controller: "Keypad", position };
			// `active` sends the finished image to the core.
			await renderImage(null, context, state, slot.action.states[slot.current_state]?.image ?? slot.action.icon, false, false, true, true, false, get(settings)?.rotation);
			// Leave room for the editor's own work between keys.
			await new Promise((resolve) => setTimeout(resolve, 15));
		}
	}
}

const scheduled = new Map<string, ReturnType<typeof setTimeout>>();
const running = new Set<string>();
const again = new Set<string>();

/**
 * Draw the other pages of an M18 again once changes settle: after it
 * connects, when pages are added, removed, or reordered, and when settings
 * that change every key (rotation, app icon size) change.
 */
export function refreshSavedPages(device: string, delay = 1500) {
	if (!device.startsWith("18-")) return;
	clearTimeout(scheduled.get(device));
	scheduled.set(
		device,
		setTimeout(() => void run(device), delay),
	);
}

async function run(device: string) {
	scheduled.delete(device);
	if (running.has(device)) {
		again.add(device);
		return;
	}
	running.add(device);
	try {
		await drawOtherPages(device);
	} catch (error) {
		console.warn("Failed to draw the M18's other pages", error);
	} finally {
		running.delete(device);
		if (again.delete(device)) refreshSavedPages(device);
	}
}
