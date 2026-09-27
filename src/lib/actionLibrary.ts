import type { Action } from "./Action.ts";

/**
 * Library groups, mirroring `Group::name()` in `src-tauri/src/action_library.rs`.
 * The order here is the order the editor shows them in.
 */
export const LIBRARY_GROUPS = [
	{ name: "Apps & Websites", slug: "apps", icon: "AppWindow", blurb: "Launch apps, files, and websites" },
	{ name: "Keyboard & Text", slug: "keyboard", icon: "Keyboard", blurb: "Shortcuts, typing, and the mouse" },
	{ name: "Media & Audio", slug: "media", icon: "SpeakerHigh", blurb: "Playback, volume, and sounds" },
	{ name: "System", slug: "system", icon: "Desktop", blurb: "macOS features and tools" },
	{ name: "Display & Power", slug: "display", icon: "Sun", blurb: "Brightness, sleep, and locking" },
	{ name: "Pages & Folders", slug: "pages", icon: "Stack", blurb: "Move between M18 pages" },
	{ name: "Action Flows", slug: "flows", icon: "FlowArrow", blurb: "Several actions on one key" },
	{ name: "M18 Device", slug: "device", icon: "Lightbulb", blurb: "LEDs and screen brightness" },
	{ name: "Browser", slug: "browser", icon: "Globe", blurb: "Web browser shortcuts" },
	{ name: "Premiere Pro", slug: "premiere", icon: "FilmSlate", blurb: "Adobe Premiere Pro shortcuts" },
	{ name: "Network", slug: "network", icon: "Broadcast", blurb: "Talk to other apps and devices" },
	{ name: "Coming Soon", slug: "soon", icon: "Hourglass", blurb: "Not working yet; kept for VSD Craft imports" },
] as const;

export type LibraryGroup = (typeof LIBRARY_GROUPS)[number];

export const COMING_SOON = "Coming Soon";

export function libraryGroup(categoryName: string): LibraryGroup | undefined {
	return LIBRARY_GROUPS.find((group) => group.name === categoryName);
}

/** Group colour as a CSS custom property, for chips, badges, and accents. */
export function groupColor(categoryName: string | undefined): string {
	const group = categoryName ? libraryGroup(categoryName) : undefined;
	return `var(--color-group-${group?.slug ?? "plugin"})`;
}

/** Actions built into the app (no plugin process). */
export function isBuiltIn(action: Pick<Action, "plugin">): boolean {
	return action.plugin === "" || action.plugin === "opendeck";
}

export const FLOW_PARENT_UUIDS = ["opendeck.multiaction", "opendeck.toggleaction", "opendeck.carouselaction"];

export function isFlowParent(uuid: string): boolean {
	return FLOW_PARENT_UUIDS.includes(uuid);
}

/** Look up an action's library entry (and its group) by UUID. */
export function findLibraryAction(categories: Record<string, { actions: Action[] }>, uuid: string): { category: string; action: Action } | undefined {
	for (const [category, { actions }] of Object.entries(categories)) {
		const action = actions.find((candidate) => candidate.uuid === uuid);
		if (action) return { category, action };
	}
	return undefined;
}
