import { invoke } from "@tauri-apps/api/core";
import { writable } from "svelte/store";

/** Opens the dialog that explains how to let the app send keystrokes. */
export const inputPermissionDialog = writable(false);

/** The Accessibility list in System Settings. */
export const ACCESSIBILITY_SETTINGS = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/** Whether macOS lets the app send keystrokes, or null where no permission is needed. */
export async function inputPermission(): Promise<boolean | null> {
	try {
		return await invoke<boolean | null>("get_input_permission");
	} catch {
		return null;
	}
}
