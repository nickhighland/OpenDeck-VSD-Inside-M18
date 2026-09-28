import type { Action } from "./Action.ts";
import type { ActionInstance } from "./ActionInstance.ts";
import type { Context } from "./Context.ts";
import type { Profile } from "./Profile.ts";

import { type Writable, writable } from "svelte/store";

/**
 * The inspected key: an instance context string for a key with an action, or
 * the slot's Context for an empty key.
 */
export const inspectedInstance: Writable<string | Context | null> = writable(null);

/** Which inspector tab is showing. */
export const inspectorTab: Writable<"behavior" | "appearance"> = writable("behavior");

/**
 * A stable key for a physical slot, so selections survive re-renders that
 * recreate Context objects. Every hierarchy index after the slot is removed:
 * a flow's child maps to its parent's physical key.
 */
export function actionContextString(context: Context): string {
	return `${context.device}.${context.profile}.${context.controller}.${context.position}.0`;
}

/** The physical slot portion of either a Context object or an ActionContext string. */
export function contextKey(context: string | Context | null | undefined): string {
	if (!context) return "";
	if (typeof context === "string") return context.split(".").slice(0, 4).join(".");
	return `${context.device}.${context.profile}.${context.controller}.${context.position}`;
}

export type ParsedActionContext = Context & {
	/** All hierarchy indices after the physical slot; the final item is the instance index. */
	indices: number[];
	index: number;
	root: boolean;
};

export function parseActionContext(context: string): ParsedActionContext {
	const parts = context.split(".");
	const indices = parts.slice(4).map((value) => Number.parseInt(value, 10));
	const safeIndices = indices.length > 0 && indices.every(Number.isFinite) ? indices : [0];
	return {
		device: parts[0] ?? "",
		profile: parts[1] ?? "",
		controller: parts[2] ?? "Keypad",
		position: Number.parseInt(parts[3] ?? "0", 10),
		indices: safeIndices,
		index: safeIndices[safeIndices.length - 1],
		root: safeIndices.length === 1 && safeIndices[0] === 0,
	};
}

function walkInstances(instances: (ActionInstance | null)[], context: string): ActionInstance | undefined {
	for (const instance of instances) {
		if (!instance) continue;
		if (instance.context === context) return instance;
		const nested = instance.children ? walkInstances(instance.children, context) : undefined;
		if (nested) return nested;
	}
	return undefined;
}

export function findInstance(profile: Profile, context: string): ActionInstance | undefined {
	return walkInstances([...profile.keys, ...profile.sliders, ...profile.infobars], context);
}

export function removeInstance(profile: Profile, context: string): boolean {
	function remove(instances: (ActionInstance | null)[]): boolean {
		for (const instance of instances) {
			if (!instance?.children) continue;
			const index = instance.children.findIndex((child) => child.context === context);
			if (index >= 0) {
				instance.children.splice(index, 1);
				return true;
			}
			if (remove(instance.children)) return true;
		}
		return false;
	}

	const parsed = parseActionContext(context);
	const slots = parsed.controller === "Encoder" ? profile.sliders : parsed.controller === "Infobar" ? profile.infobars : profile.keys;
	if (parsed.root && slots[parsed.position]?.context === context) {
		slots[parsed.position] = null;
		return true;
	}
	return remove(slots);
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

export const inspectedParentAction: Writable<string | Context | null> = writable(null);

export const openContextMenu: Writable<{ context: Context; x: number; y: number } | null> = writable(null);
document.addEventListener("click", () => openContextMenu.set(null));
document.addEventListener("keydown", (event) => {
	if (event.key == "Escape") openContextMenu.set(null);
});
globalThis.addEventListener("blur", () => openContextMenu.set(null));

export type CopiedItem = { type: "instance"; source: Context } | { type: "action"; action: Action };
export const copiedItem: Writable<CopiedItem | null> = writable(null);
