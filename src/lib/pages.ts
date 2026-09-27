import { invoke } from "@tauri-apps/api/core";
import { writable } from "svelte/store";

export type M18Page = { id: string; name: string; profile: string };
export type M18PageSet = { pages: M18Page[]; selected: number; folder_history?: number[] };

/** Page sets by device ID, kept current by the device listener in DeviceSelector. */
export const pageSets = writable<Record<string, M18PageSet>>({});

export async function loadPageSet(device: string): Promise<M18PageSet | undefined> {
	try {
		const pageSet = await invoke<M18PageSet>("get_m18_pages", { device });
		pageSets.update((sets) => ({ ...sets, [device]: pageSet }));
		return pageSet;
	} catch {
		return undefined;
	}
}

export function setPageSet(device: string, pageSet: M18PageSet) {
	pageSets.update((sets) => ({ ...sets, [device]: pageSet }));
}

/** The label shown for a page: its custom name, or its position. Plain numbers from older versions count as unnamed. */
export function pageLabel(page: M18Page, index: number): string {
	const name = page.name.trim();
	return name && !/^\d+$/.test(name) ? name : `Page ${index + 1}`;
}

/** The one-based page number of a profile, falling back to the selected page. */
export function pageNumber(pageSet: M18PageSet | undefined, profile: string | undefined): number {
	if (!pageSet || pageSet.pages.length === 0) return 1;
	const index = pageSet.pages.findIndex((page) => page.profile === profile);
	return (index >= 0 ? index : Math.min(pageSet.selected, pageSet.pages.length - 1)) + 1;
}

/**
 * Incremented when every key must be drawn again and re-sent to the M18
 * (for example after the backend asks for a redraw).
 */
export const redrawEpoch = writable(0);
