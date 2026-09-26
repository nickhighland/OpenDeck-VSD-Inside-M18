<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { DeviceInfo } from "$lib/DeviceInfo";

	import { invoke } from "@tauri-apps/api/core";
	import { onMount } from "svelte";

	export let instance: ActionInstance;
	export let device: DeviceInfo;

	let context = "";
	let settings: any = {};
	let pages: { id: string; name: string; profile: string }[] = [];
	let saving = false;

	$: if (instance && instance.context !== context) {
		context = instance.context;
		settings = structuredClone(instance.settings ?? {});
	}

	$: uuid = instance?.action?.uuid ?? "";
	$: isOpenApps = uuid == "opendeck.m18.open-apps";
	$: isSuperHotkeys = uuid == "opendeck.m18.super-hotkeys";
	$: isHotkeySwitch = uuid == "opendeck.m18.hotkey-switch";
	$: isSuperHotkeySwitch = uuid == "opendeck.m18.super-hotkey-switch";
	$: isPageGoto = uuid == "opendeck.m18.page-goto";
	$: isUnsupportedVsd = uuid == "opendeck.m18.unsupported-vsd-action";
	$: isVsdCoreAction = uuid.startsWith("com.hotspot.streamdock.") || uuid.startsWith("com.mirabox.streamdock.") || uuid.startsWith("com.streamdock.");

	onMount(async () => {
		if (device?.id) {
			try {
				const pageSet: { pages: { id: string; name: string; profile: string }[] } = await invoke("get_m18_pages", { device: device.id });
				pages = pageSet.pages;
			} catch {}
		}
	});

	async function persist() {
		if (!instance) return;
		saving = true;
		try {
			const next = structuredClone(settings);
			instance.settings = next;
			await invoke("set_instance_settings", { context: instance.context, settings: next });
		} finally {
			saving = false;
		}
	}

	function setText(key: string, event: Event) {
		settings = { ...settings, [key]: (event.currentTarget as HTMLInputElement).value, ...(key === "down" ? { display: "" } : {}) };
		void persist();
	}

	function setIndex(event: Event) {
		settings = { ...settings, index: Number((event.currentTarget as HTMLSelectElement).value) };
		void persist();
	}

	function setPage(event: Event) {
		const page = pages[Number((event.currentTarget as HTMLSelectElement).value)];
		settings = { ...settings, page: page?.profile ?? "", pageIndex: page ? pages.indexOf(page) : 0 };
		void persist();
	}

	function setShowPageNumber(event: Event) {
		settings = { ...settings, showPageNumber: (event.currentTarget as HTMLInputElement).checked };
		void persist();
	}

	function setVsdSetting(key: string, event: Event, kind: "text" | "number" | "boolean") {
		const field = event.currentTarget as HTMLInputElement;
		const value = kind === "boolean" ? field.checked : kind === "number" ? Number(field.value) : field.value;
		settings = { ...settings, [key]: value };
		void persist();
	}

	function setVsdJsonSetting(key: string, event: Event) {
		try {
			const value = JSON.parse((event.currentTarget as HTMLTextAreaElement).value);
			settings = { ...settings, [key]: value };
			void persist();
		} catch {
			// Keep the last valid setting until the edited JSON parses.
		}
	}

	function settingLabel(key: string): string {
		return key.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
	}

	function hotkeys(): { down: string; up: string; display?: string }[] {
		return Array.isArray(settings.hotkeys) ? settings.hotkeys : [];
	}

	function setHotkey(index: number, field: "down" | "up", event: Event) {
		const next = hotkeys().map((hotkey) => ({ ...hotkey }));
		next[index] = { ...next[index], [field]: (event.currentTarget as HTMLInputElement).value, ...(field === "down" ? { display: "" } : {}) };
		settings = { ...settings, hotkeys: next };
		void persist();
	}

	function addHotkey() {
		settings = { ...settings, hotkeys: [...hotkeys(), { down: "", up: "" }] };
		void persist();
	}

	function removeHotkey(index: number) {
		const next = hotkeys().filter((_, hotkeyIndex) => hotkeyIndex !== index);
		settings = { ...settings, hotkeys: next.length ? next : [{ down: "", up: "" }], index: Math.min(Number(settings.index ?? 0), Math.max(next.length - 1, 0)) };
		void persist();
	}
</script>

<div class="h-full overflow-auto p-3 text-neutral-300">
	{#if isOpenApps}
		<h2 class="font-semibold">OpenApps</h2>
		<p class="mt-1 text-xs text-neutral-400">Open an application by path, bundle identifier, or name.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="m18-app-path">Application</label>
		<input id="m18-app-path" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.appPath ?? ""} on:input={(event) => setText("appPath", event)} placeholder="/Applications/Preview.app" disabled={saving} />
	{:else if isSuperHotkeys}
		<h2 class="font-semibold">Super Hotkeys</h2>
		<p class="mt-1 text-xs text-neutral-400">Enter Enigo RON tokens. The down action runs when pressed; the up action runs when released.</p>
		{#if settings.display}<p class="mt-2 text-sm text-neutral-200">Imported shortcut: {settings.display}</p>{/if}
		<label class="mt-4 block text-xs text-neutral-400" for="m18-super-down">Down</label>
		<input id="m18-super-down" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={settings.down ?? ""} on:input={(event) => setText("down", event)} placeholder="[k(meta,uni('o'))]" disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="m18-super-up">Up</label>
		<input id="m18-super-up" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={settings.up ?? ""} on:input={(event) => setText("up", event)} placeholder="optional" disabled={saving} />
	{:else if isHotkeySwitch || isSuperHotkeySwitch}
		<h2 class="font-semibold">{isSuperHotkeySwitch ? "Super Hotkey Switch" : "HotkeySwitch"}</h2>
		<p class="mt-1 text-xs text-neutral-400">Each press sends the active shortcut, then switches the icon and shortcut to the next state.</p>
		{#if isSuperHotkeySwitch}
			<p class="mt-1 text-xs text-amber-300">Super Hotkey uses the macOS software-input fallback for now. It does not yet reproduce VSD Craft’s physical-keyboard-like input path.</p>
		{/if}
		<label class="mt-4 block text-xs text-neutral-400" for="m18-hotkey-state">Active state</label>
		<select id="m18-hotkey-state" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.index ?? 0} on:change={setIndex} disabled={saving}>
			{#each hotkeys() as _, index}
				<option value={index}>State {index + 1}</option>
			{/each}
		</select>
		<div class="mt-3 space-y-2">
			{#each hotkeys() as hotkey, index}
				<div class="rounded border border-neutral-700 p-2">
					<div class="flex items-center justify-between text-xs text-neutral-400"><span>{isSuperHotkeySwitch ? "Super Hotkey" : "Hotkey"} {index + 1}</span><button class="rounded border border-neutral-600 px-2 hover:bg-neutral-700" on:click={() => removeHotkey(index)} disabled={saving || hotkeys().length < 2} aria-label={`Remove state ${index + 1}`}>−</button></div>
					{#if hotkey.display}<p class="mt-2 text-sm text-neutral-200">Imported shortcut: {hotkey.display}</p>{/if}
					<label class="mt-2 block text-xs text-neutral-400" for={`m18-switch-down-${index}`}>Press</label>
					<input id={`m18-switch-down-${index}`} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={hotkey.down ?? ""} on:input={(event) => setHotkey(index, "down", event)} placeholder="[k(uni('a'))]" disabled={saving} />
					<label class="mt-2 block text-xs text-neutral-400" for={`m18-switch-up-${index}`}>Release (optional)</label>
					<input id={`m18-switch-up-${index}`} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={hotkey.up ?? ""} on:input={(event) => setHotkey(index, "up", event)} placeholder="optional" disabled={saving} />
				</div>
			{/each}
		</div>
		<button class="mt-3 rounded border border-neutral-600 px-2 py-1 text-xs hover:bg-neutral-700" on:click={addHotkey} disabled={saving}>Add state</button>
	{:else if isPageGoto}
		<h2 class="font-semibold">Go to page</h2>
		<p class="mt-1 text-xs text-neutral-400">Switches the native M18 page without invoking a plugin.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="m18-page-target">Page</label>
		<select id="m18-page-target" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.pageIndex ?? 0} on:change={setPage} disabled={saving}>
			{#each pages as page, index}
				<option value={index}>{page.name} — {page.profile}</option>
			{/each}
		</select>
		<label class="mt-3 flex items-center gap-2 text-sm text-neutral-300" for="m18-show-page-number">
			<input id="m18-show-page-number" type="checkbox" checked={settings.showPageNumber !== false} on:change={setShowPageNumber} disabled={saving} />
			Show page number
		</label>
	{:else if isUnsupportedVsd}
		<h2 class="font-semibold text-amber-300">Unsupported VSD Craft action</h2>
		<p class="mt-2 text-sm text-neutral-300">This button, its artwork, and its original settings were preserved, but its behavior is not implemented yet. Pressing it will not run the VSD action.</p>
		<p class="mt-3 text-xs text-neutral-400">Original action: {settings.sourceName ?? "Unknown"}</p>
		<p class="mt-1 break-all font-mono text-xs text-neutral-400">{settings.sourceUuid ?? "Unknown UUID"}</p>
	{:else if isVsdCoreAction}
		<h2 class="font-semibold">{instance?.action?.name ?? "VSD Craft action"}</h2>
		<p class="mt-1 text-xs text-neutral-400">This M18 action runs in the app core, not as an action plugin. Imported VSD Craft settings are retained below.</p>
		{#if Object.keys(settings ?? {}).length > 0}
			<div class="mt-4 space-y-3">
				{#each Object.keys(settings) as key}
					{#if typeof settings[key] === "boolean"}
						<label class="flex items-center gap-2 text-sm text-neutral-300" for={`vsd-setting-${key}`}>
							<input id={`vsd-setting-${key}`} type="checkbox" checked={settings[key]} on:change={(event) => setVsdSetting(key, event, "boolean")} disabled={saving} />
							{settingLabel(key)}
						</label>
					{:else if typeof settings[key] === "number"}
						<label class="block text-xs text-neutral-400" for={`vsd-setting-${key}`}>{settingLabel(key)}</label>
						<input id={`vsd-setting-${key}`} type="number" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key]} on:input={(event) => setVsdSetting(key, event, "number")} disabled={saving} />
					{:else if typeof settings[key] === "string"}
						<label class="block text-xs text-neutral-400" for={`vsd-setting-${key}`}>{settingLabel(key)}</label>
						<input id={`vsd-setting-${key}`} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key]} on:input={(event) => setVsdSetting(key, event, "text")} disabled={saving} />
					{:else}
					<label class="block text-xs text-neutral-400" for={`vsd-setting-${key}`}>{settingLabel(key)} (JSON)</label>
					<textarea id={`vsd-setting-${key}`} rows="4" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 font-mono text-xs" value={JSON.stringify(settings[key], null, 2)} on:change={(event) => setVsdJsonSetting(key, event)} disabled={saving}></textarea>
					{/if}
				{/each}
			</div>
		{:else}
			<p class="mt-3 text-xs text-neutral-400">No settings were serialized by VSD Craft for this action.</p>
		{/if}
	{:else}
		<h2 class="font-semibold">{instance?.action?.name ?? "M18 action"}</h2>
		<p class="mt-1 text-xs text-neutral-400">This action is built into the M18 core and has no plugin settings.</p>
	{/if}
</div>
