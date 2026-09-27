import type { Action } from "./Action.ts";
import type { Context } from "./Context.ts";

import { type Writable, writable } from "svelte/store";

/**
 * The inspected key: an instance context string for a key with an action, or
 * the slot's Context for an empty key.
 */
export const inspectedInstance: Writable<string | Context | null> = writable(null);

/** Which inspector tab is showing. */
export const inspectorTab: Writable<"behavior" | "appearance"> = writable("behavior");

/**
 * A stable key for a slot, so selections survive re-renders that recreate
 * Context objects. Instance context strings end with the instance index, which
 * is dropped: a flow's child maps to its parent's slot.
 */
export function contextKey(context: string | Context | null | undefined): string {
	if (!context) return "";
	if (typeof context === "string") return context.split(".").slice(0, -1).join(".");
	return `${context.device}.${context.profile}.${context.controller}.${context.position}`;
}

import { invoke } from "@tauri-apps/api/core";
let old: string | Context | null = null;
inspectedInstance.subscribe(async (value) => {
	const previous = old;
	old = value;
	try {
		await invoke("switch_property_inspector", {
			old: typeof previous == "string" ? previous : null,
			new: typeof value == "string" ? value : null,
		});
	} catch {
		// The instance may have been removed in the meantime.
	}
});
inspectedInstance.subscribe(() => inspectorTab.set("behavior"));

export const inspectedParentAction: Writable<Context | null> = writable(null);

export const openContextMenu: Writable<{ context: Context; x: number; y: number } | null> = writable(null);
document.addEventListener("click", () => openContextMenu.set(null));
document.addEventListener("keydown", (event) => {
	if (event.key == "Escape") openContextMenu.set(null);
});
globalThis.addEventListener("blur", () => openContextMenu.set(null));

export type CopiedItem = { type: "instance"; source: Context } | { type: "action"; action: Action };
export const copiedItem: Writable<CopiedItem | null> = writable(null);
