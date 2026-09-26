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
	{:else}
		<h2 class="font-semibold">{instance?.action?.name ?? "M18 action"}</h2>
		<p class="mt-1 text-xs text-neutral-400">This action is built into the M18 core and has no plugin settings.</p>
	{/if}
</div>
