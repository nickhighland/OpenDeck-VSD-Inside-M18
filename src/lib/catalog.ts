import type { Action } from "./Action.ts";
import type { ActionCategory } from "./actionSearch.ts";

import { invoke } from "@tauri-apps/api/core";
import { derived, writable } from "svelte/store";

export type InstalledPlugin = {
	id: string;
	name: string;
	author: string;
	icon: string;
	version: string;
	builtin: boolean;
	registered: boolean;
	has_settings_interface: boolean;
};

/** The action library: built-in groups plus plugin categories. */
export const categories = writable<Record<string, ActionCategory>>({});
export const plugins = writable<InstalledPlugin[]>([]);

export async function reloadCatalog() {
	const [nextCategories, nextPlugins] = await Promise.all([invoke<Record<string, ActionCategory>>("get_categories"), invoke<InstalledPlugin[]>("list_plugins")]);
	categories.set(nextCategories);
	plugins.set(nextPlugins);
}

/** UUID → the library's entry and group, for names, artwork, and badges. */
export const actionIndex = derived(categories, ($categories) => {
	const index = new Map<string, { category: string; action: Action }>();
	for (const [category, { actions }] of Object.entries($categories)) {
		for (const action of actions) {
			// A visible entry wins over a hidden duplicate with the same UUID.
			const existing = index.get(action.uuid);
			if (!existing || (!existing.action.visible_in_action_list && action.visible_in_action_list)) {
				index.set(action.uuid, { category, action });
			}
		}
	}
	return index;
});
