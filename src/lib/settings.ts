export type Settings = {
	version: string;
	language: string;
	brightness: number;
	sleep_timeout_minutes: number;
	sleep_when_computer_locked: boolean;
	rotation: number;
	background: boolean;
	autolaunch: boolean;
	updatecheck: boolean;
	statistics: boolean;
	separatewine: boolean;
	developer: boolean;
};

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type Writable, writable } from "svelte/store";
import { locale } from "./i18n.ts";

export const settings: Writable<Settings | null> = writable(null);
export const localisations: Writable<{ [plugin: string]: any } | null> = writable(null);

(async () => {
	try {
		settings.set(await invoke("get_settings"));
	} catch (error) {
		console.error("Failed to load settings", error);
	}
})();

// Save shortly after the last change: a slider drag produces dozens of
// updates, and each save writes the settings file.
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let loadedLanguage: string | null = null;
settings.subscribe((value) => {
	if (!value) return;
	clearTimeout(saveTimer);
	const snapshot = { ...value };
	saveTimer = setTimeout(() => {
		invoke("set_settings", { settings: snapshot }).catch((error) => console.error("Failed to save settings", error));
	}, 120);
	if (value.language !== loadedLanguage) {
		loadedLanguage = value.language;
		locale.set(value.language);
		invoke<{ [plugin: string]: any }>("get_localisations", { locale: value.language })
			.then((result) => localisations.set(result))
			.catch(() => localisations.set({}));
	}
});

// The M18 Brightness action changes the device directly; keep the slider in step.
listen<{ action: string; value: number }>("device_brightness", ({ payload }) => {
	settings.update((current) => {
		if (!current) return current;
		let value = current.brightness;
		if (payload.action === "increase") value += payload.value;
		else if (payload.action === "decrease") value -= payload.value;
		else value = payload.value;
		return { ...current, brightness: Math.max(0, Math.min(100, value)) };
	});
}).catch(() => {});

/** Languages with a translation file in `translations/`. */
export const LANGUAGES: { code: string; name: string }[] = [
	{ code: "en", name: "English" },
	{ code: "de", name: "Deutsch" },
	{ code: "pt_BR", name: "Português (Brasil)" },
	{ code: "sv", name: "Svenska" },
	{ code: "uk", name: "Українська" },
];
