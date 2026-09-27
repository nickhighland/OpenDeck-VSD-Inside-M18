<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { DeviceInfo } from "$lib/DeviceInfo";

	import { invoke } from "@tauri-apps/api/core";
	import { open } from "@tauri-apps/plugin-dialog";
	import { onMount } from "svelte";

	export let instance: ActionInstance;
	export let device: DeviceInfo;

	let context = "";
	let settings: any = {};
	let pages: { id: string; name: string; profile: string }[] = [];
	let audioOutputDevices: { id: string; name: string }[] = [];
	let saving = false;
	let mousePositionError = "";

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
	$: isVsdHotkey = uuid == "com.hotspot.streamdock.system.hotkey";
	$: isVsdSuperHotkey = uuid == "com.hotspot.streamdock.system.super.hotkey";
	$: isFolderOpen = uuid == "com.hotspot.streamdock.profile.openchild";
	$: isFolderBack = uuid == "com.hotspot.streamdock.profile.backtoparent";
	$: isUnsupportedVsd = uuid == "opendeck.m18.unsupported-vsd-action";
	$: isVsdCoreAction = uuid.startsWith("com.hotspot.streamdock.") || uuid.startsWith("com.mirabox.streamdock.") || uuid.startsWith("com.streamdock.");
	$: isTextAction = uuid == "com.hotspot.streamdock.system.text" || uuid == "com.hotspot.streamdock.plain.text";
	$: isPasswordAction = uuid == "com.hotspot.streamdock.system.password";
	$: isOpenAction = uuid == "com.hotspot.streamdock.system.open";
	$: isWebsiteAction = uuid == "com.hotspot.streamdock.system.website";
	$: isCloseAction = uuid == "com.hotspot.streamdock.system.close";
	$: isUdpAction = uuid == "com.hotspot.streamdock.network.udp";
	$: isSoundboardAction = uuid == "com.hotspot.streamdock.soundboard.playaudio";
	$: isWorldTimeAction = uuid == "com.mirabox.streamdock.time.action1";
	$: isTimerAction = uuid == "com.mirabox.streamdock.time.action2";
	$: isCountdownAction = uuid == "com.mirabox.streamdock.time.action3";
	$: isDateTimeAction = uuid == "com.mirabox.streamdock.dateTime.action1";
	$: isWeatherAction = uuid == "com.hotspot.streamdock.weather.action1";
	$: isMemoAction = uuid == "com.hotspot.streamdock.memo.action1" || uuid == "com.hotspot.streamdock.memo.action2";
	$: isYoutubeAction = uuid == "com.hotspot.streamdock.youtube.chatmessage" || uuid == "com.hotspot.streamdock.youtube.viewers";
	$: isBrightnessAction = uuid == "com.hotspot.streamdock.device.brightness";
	$: isMultimediaAction = uuid == "com.hotspot.streamdock.system.multimedia";
	$: isMouseEventAction = uuid == "com.hotspot.streamdock.mouse.event";
	$: isMicrophoneAction = uuid == "com.hotspot.streamdock.quickcontrol.microphone";

	onMount(async () => {
		try {
			audioOutputDevices = await invoke<{ id: string; name: string }[]>("get_audio_output_devices");
		} catch {}
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

	function textValue(keys: string[], fallback = ""): string {
		for (const key of keys) {
			if (typeof settings?.[key] === "string") return settings[key];
		}
		return fallback;
	}

	function setTextFor(keys: string[], fallbackKey: string, event: Event) {
		const key = keys.find((candidate) => Object.prototype.hasOwnProperty.call(settings ?? {}, candidate)) ?? fallbackKey;
		setText(key, event);
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

	function setFolderTarget(event: Event) {
		const page = pages[Number((event.currentTarget as HTMLSelectElement).value)];
		settings = { ...settings, profile: page?.profile ?? "" };
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

	async function choosePath(key: string, filters: { name: string; extensions: string[] }[] = []) {
		const selected = await open({ multiple: false, directory: false, ...(filters.length ? { filters } : {}) });
		if (typeof selected === "string") {
			settings = { ...settings, [key]: selected };
			await persist();
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

	function setMouseModifier(modifier: string, event: Event) {
		const selected = new Set(Array.isArray(settings.modifiers) ? settings.modifiers : []);
		const isChecked = (event.currentTarget as HTMLInputElement).checked;
		isChecked ? selected.add(modifier.toLowerCase()) : selected.delete(modifier.toLowerCase());
		settings = { ...settings, modifiers: [...selected] };
		void persist();
	}

	async function captureMousePosition() {
		mousePositionError = "";
		try {
			const [x, y] = await invoke<[number, number]>("get_mouse_position");
			settings = { ...settings, x, y, coordinate: "absolute" };
			await persist();
		} catch (error) {
			mousePositionError = String(error);
		}
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
		<p class="mt-1 text-xs text-neutral-400">The down action runs when pressed; the up action runs when released.</p>
		<p class="mt-1 text-xs text-amber-300">VSD Craft distinguishes Super Hotkey from Hotkey by its physical-keyboard-like input path. This fork currently routes both through software key injection, so Super Hotkey is not yet at behavioral parity.</p>
		{#if settings.display}<p class="mt-2 text-sm text-neutral-200">Imported shortcut: {settings.display}</p>{/if}
		<label class="mt-4 block text-xs text-neutral-400" for="m18-super-down">Down</label>
		<input id="m18-super-down" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={settings.down ?? ""} on:input={(event) => setText("down", event)} placeholder="[k(meta,uni('o'))]" disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="m18-super-up">Up</label>
		<input id="m18-super-up" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={settings.up ?? ""} on:input={(event) => setText("up", event)} placeholder="optional" disabled={saving} />
	{:else if isVsdHotkey || isVsdSuperHotkey}
		<h2 class="font-semibold">{isVsdSuperHotkey ? "Super Hotkey" : "Hotkey"}</h2>
		<p class="mt-1 text-xs text-neutral-400">{isVsdSuperHotkey ? "VSD Craft uses a physical-keyboard-like input path for Super Hotkey." : "Hotkey sends a software-level virtual key input."} The Press field runs on key-down; Release runs when the M18 key is released.</p>
		{#if isVsdSuperHotkey}
			<p class="mt-1 text-xs text-amber-300">This fork currently uses the same software injector for Hotkey and Super Hotkey; the distinct physical-HID behavior is not implemented yet.</p>
		{/if}
		{#if settings.display}<p class="mt-2 text-sm text-neutral-200">Imported shortcut: {settings.display}</p>{/if}
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-hotkey-down">Press</label>
		<input id="vsd-hotkey-down" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={textValue(["down", "Down"])} on:input={(event) => setTextFor(["down", "Down"], "down", event)} placeholder="[k(meta,uni('o'))]" disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-hotkey-up">Release (optional)</label>
		<input id="vsd-hotkey-up" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm font-mono" value={textValue(["up", "Up"])} on:input={(event) => setTextFor(["up", "Up"], "up", event)} placeholder="optional" disabled={saving} />
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
	{:else if isFolderOpen || isFolderBack}
		<h2 class="font-semibold">{isFolderOpen ? "Create Folder" : "Go back"}</h2>
		{#if isFolderOpen}
			<p class="mt-1 text-xs text-neutral-400">Opens the selected M18 page as a nested folder. A Go back action in that page returns to the page that opened it.</p>
			<label class="mt-4 block text-xs text-neutral-400" for="m18-folder-target">Folder contents</label>
			<select id="m18-folder-target" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={pages.findIndex((page) => page.profile === settings.profile)} on:change={setFolderTarget} disabled={saving || pages.length === 0}>
				<option value={-1}>Choose a page…</option>
				{#each pages as page, index}<option value={index}>{page.name} — {page.profile}</option>{/each}
			</select>
		{:else}
			<p class="mt-1 text-xs text-neutral-400">Returns to the folder’s parent page. Folder navigation is handled by the M18 core, without an action plugin.</p>
		{/if}
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
	{:else if isTextAction}
		<h2 class="font-semibold">Text</h2>
		<p class="mt-1 text-xs text-neutral-400">Types this text into the currently focused app when the M18 key is pressed.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-text-value">Text to type</label>
		<textarea id="vsd-text-value" rows="6" class="mt-1 w-full resize-y rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={textValue(["text", "Text", "value", "Value"])} on:input={(event) => setTextFor(["text", "Text", "value", "Value"], "text", event)} disabled={saving}></textarea>
	{:else if isPasswordAction}
		<h2 class="font-semibold">Password</h2>
		<p class="mt-1 text-xs text-neutral-400">Types the saved password into the currently focused app. The value is stored in this profile.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-password-value">Password</label>
		<input id="vsd-password-value" type="password" autocomplete="new-password" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={textValue(["password", "Password", "text", "Text"])} on:input={(event) => setTextFor(["password", "Password", "text", "Text"], "password", event)} disabled={saving} />
	{:else if isOpenAction || isWebsiteAction || isCloseAction}
		<h2 class="font-semibold">{isCloseAction ? "Close application" : isWebsiteAction ? "Open website" : "Open item"}</h2>
		<p class="mt-1 text-xs text-neutral-400">{isCloseAction ? "Quits the named application." : isWebsiteAction ? "Opens the URL in the default browser." : "Opens an application, file, or folder with its default macOS handler."}</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-open-target">{isCloseAction ? "Application name or path" : isWebsiteAction ? "Website URL" : "Application, file, or folder"}</label>
		<input id="vsd-open-target" type={isWebsiteAction ? "url" : "text"} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={isCloseAction ? textValue(["appPath", "path", "application", "app", "bundleId"]) : textValue(["path", "Path", "url", "URL", "website", "link", "file", "folder"])} on:input={(event) => setTextFor(isCloseAction ? ["appPath", "path", "application", "app", "bundleId"] : ["path", "Path", "url", "URL", "website", "link", "file", "folder"], isCloseAction ? "appPath" : "path", event)} placeholder={isWebsiteAction ? "https://example.com" : isCloseAction ? "Safari" : "/Applications/Preview.app"} disabled={saving} />
		{#if isOpenAction}
			<div class="mt-2 flex gap-2">
				<button class="rounded border border-neutral-600 px-2 py-1 text-xs hover:bg-neutral-700" on:click={() => void choosePath("path")}>Choose file</button>
				<button class="rounded border border-neutral-600 px-2 py-1 text-xs hover:bg-neutral-700" on:click={() => void choosePath("path", [{ name: "Applications", extensions: ["app"] }])}>Choose application</button>
			</div>
		{/if}
	{:else if isUdpAction}
		<h2 class="font-semibold">UDP</h2>
		<p class="mt-1 text-xs text-neutral-400">Sends one UTF-8 UDP datagram to the configured address when pressed.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-udp-host">Host or IP address</label>
		<input id="vsd-udp-host" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={textValue(["host", "Host", "ip", "IP", "address", "Address"], "127.0.0.1")} on:input={(event) => setTextFor(["host", "Host", "ip", "IP", "address", "Address"], "host", event)} disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-udp-port">Port</label>
		<input id="vsd-udp-port" type="number" min="1" max="65535" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.port ?? settings.Port ?? 5000} on:input={(event) => setVsdSetting(Object.prototype.hasOwnProperty.call(settings ?? {}, "Port") ? "Port" : "port", event, "number")} disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-udp-message">Message</label>
		<textarea id="vsd-udp-message" rows="4" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={textValue(["message", "Message", "data", "Data"])} on:input={(event) => setTextFor(["message", "Message", "data", "Data"], "message", event)} disabled={saving}></textarea>
	{:else if isBrightnessAction}
		<h2 class="font-semibold">Device brightness</h2>
		<p class="mt-1 text-xs text-neutral-400">Adjusts the M18 screen brightness without changing your macOS display brightness.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-device-brightness-mode">Action</label>
		<select id="vsd-device-brightness-mode" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.actionIdx ?? 0} on:change={(event) => setVsdSetting("actionIdx", event, "number")} disabled={saving}>
			<option value="0">Increase brightness</option><option value="1">Decrease brightness</option>
		</select>
	{:else if isMultimediaAction}
		<h2 class="font-semibold">Multimedia</h2>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-multimedia-action">Operation</label>
		<select id="vsd-multimedia-action" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.actionIdx ?? 1} on:change={(event) => setVsdSetting("actionIdx", event, "number")} disabled={saving}>
			<option value="0">Previous</option><option value="1">Play/Pause</option><option value="2">Next</option><option value="3">Stop</option><option value="4">Mute</option><option value="5">Increase volume</option><option value="6">Lower volume</option>
		</select>
		{#if Number(settings.actionIdx ?? 1) === 3}
			<p class="mt-2 text-xs text-amber-300">macOS has no global media-stop key. Stop currently targets Music when it is already running; other platforms send the system media-stop key.</p>
		{/if}
	{:else if isMouseEventAction}
		<h2 class="font-semibold">Mouse event</h2>
		<p class="mt-1 text-xs text-neutral-400">Simulates clicks, movement, scrolling, or dragging. macOS may require Accessibility permission.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-mouse-type">Event</label>
		<select id="vsd-mouse-type" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.eventType ?? "click"} on:change={(event) => setText("eventType", event)} disabled={saving}>
			<option value="click">Mouse click</option><option value="doubleClick">Mouse double click</option><option value="move">Mouse move</option><option value="scroll">Mouse wheel scroll</option><option value="drag">Drag and drop</option>
		</select>
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-mouse-button">Mouse button</label>
		<select id="vsd-mouse-button" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.button ?? "left"} on:change={(event) => setText("button", event)} disabled={saving}>
			<option value="left">Left</option><option value="right">Right</option><option value="middle">Middle</option><option value="side1">Side button 1</option><option value="side2">Side button 2</option>
		</select>
		<div class="mt-3 grid grid-cols-2 gap-2">
			<label class="block text-xs text-neutral-400" for="vsd-mouse-x">X</label><label class="block text-xs text-neutral-400" for="vsd-mouse-y">Y</label>
			<input id="vsd-mouse-x" type="number" class="rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.x ?? 0} on:input={(event) => setVsdSetting("x", event, "number")} disabled={saving} />
			<input id="vsd-mouse-y" type="number" class="rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.y ?? 0} on:input={(event) => setVsdSetting("y", event, "number")} disabled={saving} />
		</div>
		{#if settings.eventType === "move" || settings.eventType === "drag"}
			<label class="mt-3 block text-xs text-neutral-400" for="vsd-mouse-coordinate">Coordinate mode</label>
			<select id="vsd-mouse-coordinate" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.coordinate ?? "absolute"} on:change={(event) => setText("coordinate", event)} disabled={saving}>
				<option value="absolute">Absolute screen position</option><option value="relative">Relative to the pointer</option>
			</select>
			<button class="mt-2 rounded border border-neutral-600 px-2 py-1 text-xs hover:bg-neutral-700" on:click={() => void captureMousePosition()} disabled={saving}>Capture current pointer position</button>
			{#if mousePositionError}<p role="alert" class="mt-1 text-xs text-red-300">{mousePositionError}</p>{/if}
		{/if}
		{#if settings.eventType === "scroll"}
			<label class="mt-3 block text-xs text-neutral-400" for="vsd-mouse-amount">Scroll amount</label><input id="vsd-mouse-amount" type="number" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.amount ?? 3} on:input={(event) => setVsdSetting("amount", event, "number")} disabled={saving} />
			<label class="mt-3 block text-xs text-neutral-400" for="vsd-mouse-axis">Scroll axis</label><select id="vsd-mouse-axis" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.axis ?? "vertical"} on:change={(event) => setText("axis", event)} disabled={saving}><option value="vertical">Vertical</option><option value="horizontal">Horizontal</option></select>
		{/if}
		<div class="mt-3 flex flex-wrap gap-3 text-sm">
			{#each ["Shift", "Control", "Alt", "Command"] as modifier}
				<label class="flex items-center gap-1" for={`vsd-mouse-mod-${modifier}`}><input id={`vsd-mouse-mod-${modifier}`} type="checkbox" checked={(Array.isArray(settings.modifiers) ? settings.modifiers : []).includes(modifier.toLowerCase())} on:change={(event) => setMouseModifier(modifier, event)} disabled={saving} />{modifier}</label>
			{/each}
		</div>
	{:else if isMicrophoneAction}
		<h2 class="font-semibold">Microphone</h2>
		<p class="mt-2 text-sm text-neutral-400">Toggles mute on the current default input device. macOS requires that device to expose a writable Core Audio mute control; if it does not, the action reports an error instead of only opening Sound settings.</p>
	{:else if isSoundboardAction}
		<h2 class="font-semibold">Play Audio</h2>
		<p class="mt-1 text-xs text-neutral-400">Plays on the selected macOS output device with per-action volume and fade behavior.</p>
		<label class="mt-4 block text-xs text-neutral-400" for="vsd-sound-path">Audio file</label>
		<div class="mt-1 flex gap-2">
			<input id="vsd-sound-path" class="min-w-0 flex-1 rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={textValue(["path", "filePath", "audio", "musicUrl"])} on:input={(event) => setTextFor(["path", "filePath", "audio", "musicUrl"], "path", event)} disabled={saving} />
			<button class="rounded border border-neutral-600 px-2 text-xs hover:bg-neutral-700" on:click={() => void choosePath("path", [{ name: "Audio", extensions: ["mp3", "wav", "mp4", "m4a", "m4b", "m4p", "mov", "aiff", "flac"] }])}>Browse</button>
		</div>
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-sound-mode">Playback mode</label>
		<select id="vsd-sound-mode" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.mode ?? "Play/Stop"} on:change={(event) => setText("mode", event)} disabled={saving}>
			<option>Play/Stop</option><option>Play/Overlap</option><option>Play/Replay</option><option>Loop/Stop</option>
		</select>
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-sound-volume">Volume: {settings.volume ?? 100}%</label>
		<input id="vsd-sound-volume" type="range" min="0" max="100" class="mt-1 w-full" value={settings.volume ?? 100} on:input={(event) => setVsdSetting("volume", event, "number")} disabled={saving} />
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-sound-output">Output device</label>
		<select id="vsd-sound-output" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.outputDevice ?? settings.device?.description ?? settings.device?.name ?? settings.device?.id ?? "default"} on:change={(event) => setText("outputDevice", event)} disabled={saving}>
			<option value="default">System default</option>
			{#each audioOutputDevices as outputDevice}<option value={outputDevice.id}>{outputDevice.name}</option>{/each}
		</select>
		<label class="mt-3 block text-xs text-neutral-400" for="vsd-sound-fade">Fade</label>
		<select id="vsd-sound-fade" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.fadeType ?? "none"} on:change={(event) => setText("fadeType", event)} disabled={saving}>
			<option value="none">None</option><option value="fadeIn">Fade in</option><option value="fadeOut">Fade out</option><option value="fadeInOut">Fade in and out</option>
		</select>
		{#if (settings.fadeType ?? "none") !== "none"}
			<label class="mt-3 block text-xs text-neutral-400" for="vsd-sound-fade-duration">Fade duration: {settings.fadeDuration ?? 0} seconds</label>
			<input id="vsd-sound-fade-duration" type="number" min="0" max="3600" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings.fadeDuration ?? 0} on:input={(event) => setVsdSetting("fadeDuration", event, "number")} disabled={saving} />
		{/if}
	{:else if isWorldTimeAction || isTimerAction || isCountdownAction || isDateTimeAction}
		<h2 class="font-semibold">{isWorldTimeAction ? "World Time" : isTimerAction ? "Timer" : isCountdownAction ? "Countdown" : "DateTime"}</h2>
		<p class="mt-1 text-xs text-amber-300">The VSD display behavior and timer lifecycle are not yet at parity; values here are preserved while that runtime is implemented.</p>
		{#each Object.keys(settings ?? {}) as key}
			{#if typeof settings[key] === "boolean"}
				<label class="mt-3 flex items-center gap-2 text-sm text-neutral-300" for={`vsd-time-${key}`}><input id={`vsd-time-${key}`} type="checkbox" checked={settings[key]} on:change={(event) => setVsdSetting(key, event, "boolean")} disabled={saving} />{settingLabel(key)}</label>
			{:else if typeof settings[key] === "number"}
				<label class="mt-3 block text-xs text-neutral-400" for={`vsd-time-${key}`}>{settingLabel(key)}</label><input id={`vsd-time-${key}`} type="number" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key]} on:input={(event) => setVsdSetting(key, event, "number")} disabled={saving} />
			{:else}
				<label class="mt-3 block text-xs text-neutral-400" for={`vsd-time-${key}`}>{settingLabel(key)}</label><input id={`vsd-time-${key}`} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key] ?? ""} on:input={(event) => setText(key, event)} disabled={saving} />
			{/if}
		{/each}
	{:else if isWeatherAction || isMemoAction || isYoutubeAction}
		<h2 class="font-semibold">{isWeatherAction ? "Weather" : isMemoAction ? (uuid.endsWith("action2") ? "Record to-do" : "Remember things") : "YouTube"}</h2>
		<p class="mt-1 text-xs text-amber-300">The service/action runtime is still being ported. This editor retains settings but does not claim parity yet.</p>
		{#each Object.keys(settings ?? {}) as key}
			{#if typeof settings[key] === "boolean"}
				<label class="mt-3 flex items-center gap-2 text-sm text-neutral-300" for={`vsd-extra-${key}`}><input id={`vsd-extra-${key}`} type="checkbox" checked={settings[key]} on:change={(event) => setVsdSetting(key, event, "boolean")} disabled={saving} />{settingLabel(key)}</label>
			{:else if typeof settings[key] === "number"}
				<label class="mt-3 block text-xs text-neutral-400" for={`vsd-extra-${key}`}>{settingLabel(key)}</label><input id={`vsd-extra-${key}`} type="number" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key]} on:input={(event) => setVsdSetting(key, event, "number")} disabled={saving} />
			{:else if typeof settings[key] === "string"}
				<label class="mt-3 block text-xs text-neutral-400" for={`vsd-extra-${key}`}>{settingLabel(key)}</label><input id={`vsd-extra-${key}`} class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 text-sm" value={settings[key]} on:input={(event) => setText(key, event)} disabled={saving} />
			{:else}
				<label class="mt-3 block text-xs text-neutral-400" for={`vsd-extra-${key}`}>{settingLabel(key)} (JSON)</label><textarea id={`vsd-extra-${key}`} rows="4" class="mt-1 w-full rounded border border-neutral-600 bg-neutral-900 px-2 py-1 font-mono text-xs" value={JSON.stringify(settings[key], null, 2)} on:change={(event) => setVsdJsonSetting(key, event)} disabled={saving}></textarea>
			{/if}
		{/each}
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
