<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { ActionState } from "$lib/ActionState";
	import type { DeviceInfo } from "$lib/DeviceInfo";

	import Crosshair from "phosphor-svelte/lib/Crosshair";
	import Eye from "phosphor-svelte/lib/Eye";
	import EyeSlash from "phosphor-svelte/lib/EyeSlash";
	import FolderOpen from "phosphor-svelte/lib/FolderOpen";
	import ImageSquare from "phosphor-svelte/lib/ImageSquare";
	import Info from "phosphor-svelte/lib/Info";
	import Plus from "phosphor-svelte/lib/Plus";
	import Trash from "phosphor-svelte/lib/Trash";
	import Warning from "phosphor-svelte/lib/Warning";
	import KeyFace from "./KeyFace.svelte";
	import ShortcutRecorder from "./ShortcutRecorder.svelte";

	import { COMING_SOON } from "$lib/actionLibrary";
	import { isDefaultArtwork } from "$lib/appIcons";
	import { actionIndex } from "$lib/catalog";
	import { pageLabel, pageSets } from "$lib/pages";
	import { resizeImage } from "$lib/rendererHelper";
	import { describeSequenceText } from "$lib/shortcuts";
	import { errorText, toast } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { listen, type UnlistenFn } from "@tauri-apps/api/event";
	import { open } from "@tauri-apps/plugin-dialog";
	import { createEventDispatcher, onDestroy, onMount } from "svelte";

	export let instance: ActionInstance;
	export let device: DeviceInfo;

	// Appearance changes are made to `instance` in place; this tells the
	// inspector to redraw its header.
	const dispatch = createEventDispatcher<{ edit: void }>();

	let context = "";
	let settings: any = {};
	let audioOutputDevices: { id: string; name: string }[] = [];
	let mousePositionError = "";
	let revealPassword = false;

	$: if (instance && instance.context !== context) {
		flushPending();
		void flushStates();
		context = instance.context;
		settings = structuredClone(instance.settings ?? {});
		revealPassword = false;
	}

	$: pages = $pageSets[device?.id]?.pages ?? [];

	$: uuid = instance?.action?.uuid ?? "";
	$: isOpenApps = uuid == "opendeck.m18.open-apps";
	$: isSuperHotkeys = uuid == "opendeck.m18.super-hotkeys";
	$: isHotkeySwitch = uuid == "opendeck.m18.hotkey-switch" || uuid == "com.hotspot.streamdock.system.hotkeySwitch";
	$: isSuperHotkeySwitch = uuid == "opendeck.m18.super-hotkey-switch";
	$: isPageGoto = uuid == "opendeck.m18.page-goto";
	$: isVsdHotkey = uuid == "com.hotspot.streamdock.system.hotkey";
	$: isVsdSuperHotkey = uuid == "com.hotspot.streamdock.system.super.hotkey";
	$: isFolderOpen = uuid == "com.hotspot.streamdock.profile.openchild";
	$: isFolderBack = uuid == "com.hotspot.streamdock.profile.backtoparent";
	$: isSceneShift = uuid == "com.hotspot.streamdock.profile.rotate";
	$: isDelay = uuid == "com.hotspot.streamdock.multiactions.delay";
	$: isUnsupportedVsd = uuid == "opendeck.m18.unsupported-vsd-action";
	$: isVsdCoreAction = uuid.startsWith("com.hotspot.streamdock.") || uuid.startsWith("com.mirabox.streamdock.") || uuid.startsWith("com.streamdock.");
	$: isTextAction = uuid == "com.hotspot.streamdock.system.text" || uuid == "com.hotspot.streamdock.plain.text";
	$: isPasswordAction = uuid == "com.hotspot.streamdock.system.password";
	$: isOpenAction = uuid == "com.hotspot.streamdock.system.open";
	$: isWebsiteAction = uuid == "com.hotspot.streamdock.system.website";
	$: isCloseAction = uuid == "com.hotspot.streamdock.system.close";
	$: isUdpAction = uuid == "com.hotspot.streamdock.network.udp";
	$: isSoundboardAction = uuid == "com.hotspot.streamdock.soundboard.playaudio";
	$: isBrightnessAction = uuid == "com.hotspot.streamdock.device.brightness";
	$: isMultimediaAction = uuid == "com.hotspot.streamdock.system.multimedia";
	$: isMouseEventAction = uuid == "com.hotspot.streamdock.mouse.event";
	$: isMicrophoneAction = uuid == "com.hotspot.streamdock.quickcontrol.microphone";
	$: isEmojiAction = uuid == "com.mirabox.streamdock.emoji.emoji" || uuid == "com.mirabox.streamdock.emoji.emoji_send";
	$: libraryEntry = $actionIndex.get(uuid);
	$: comingSoon = libraryEntry?.category === COMING_SOON || uuid.startsWith("com.mirabox.streamdock.screensaver.");

	onMount(async () => {
		try {
			audioOutputDevices = await invoke<{ id: string; name: string }[]>("get_audio_output_devices");
		} catch {}
	});

	onMount(() => {
		let disposed = false;
		let unlisten: UnlistenFn | undefined;
		// A press moves a switch on to its next shortcut, and adding or removing
		// shortcuts changes its states: follow the key's saved state.
		listen<{ context: string; contents: ActionInstance | null }>("update_state", ({ payload }) => {
			const contents = payload.contents;
			if (!instance || !contents || payload.context !== instance.context || !(isHotkeySwitch || isSuperHotkeySwitch)) return;
			// Keep appearance edits that are still being saved.
			instance.states = contents.states.map((state, index) => (pendingStates.has(index) ? (instance.states[index] ?? state) : state));
			instance.current_state = contents.current_state;
			if (typeof contents.settings?.index === "number") settings = { ...settings, index: contents.settings.index };
		}).then((stop) => (disposed ? stop() : (unlisten = stop)));
		return () => {
			disposed = true;
			unlisten?.();
		};
	});

	async function persist() {
		if (!instance) return;
		clearTimeout(pendingTimer);
		pendingTimer = undefined;
		const next = structuredClone(settings);
		instance.settings = next;
		try {
			await invoke("set_instance_settings", { context: instance.context, settings: next });
		} catch (error) {
			console.warn("Failed to save key settings", error);
		}
	}

	// Typing saves shortly after the last keystroke instead of on every one.
	let pendingTimer: ReturnType<typeof setTimeout> | undefined;
	function persistSoon() {
		clearTimeout(pendingTimer);
		pendingTimer = setTimeout(() => void persist(), 300);
	}
	function flushPending() {
		if (pendingTimer !== undefined) void persist();
	}
	onDestroy(() => {
		flushPending();
		void flushStates();
	});

	// Each shortcut of a switch has its own appearance: the key's state with
	// the same position. Titles save shortly after the last keystroke.
	type PendingState = { timer: ReturnType<typeof setTimeout>; send: () => Promise<void> };
	const pendingStates = new Map<number, PendingState>();

	function saveState(index: number, immediate: boolean) {
		const state = instance?.states?.[index];
		if (!state) return;
		clearTimeout(pendingStates.get(index)?.timer);
		const request = { context: instance.context, index, state: structuredClone(state) };
		const send = async () => {
			pendingStates.delete(index);
			try {
				await invoke("set_state", request);
			} catch (error) {
				console.warn("Failed to save the shortcut's appearance", error);
			}
		};
		if (immediate) void send();
		else pendingStates.set(index, { timer: setTimeout(send, 300), send });
		dispatch("edit");
	}

	async function flushStates() {
		const pending = [...pendingStates.values()];
		for (const { timer } of pending) clearTimeout(timer);
		await Promise.all(pending.map(({ send }) => send()));
	}

	function setAppearance(index: number, patch: Partial<ActionState>, immediate = true) {
		const state = instance?.states?.[index];
		if (!state) return;
		Object.assign(state, patch);
		instance = instance;
		saveState(index, immediate);
	}

	let imageInput: HTMLInputElement;
	let imageFor = 0;
	function chooseImage(index: number) {
		imageFor = index;
		imageInput.click();
	}

	async function useImage(file: File | undefined, index: number) {
		if (!file || !file.type.startsWith("image/")) return;
		const source = await new Promise<string>((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => resolve(String(reader.result));
			reader.onerror = () => reject(reader.error);
			reader.readAsDataURL(file);
		});
		setAppearance(index, { image: (await resizeImage(source)) ?? source, image_scale: 100 });
	}

	$: defaultFace = libraryEntry?.action.icon ?? instance?.action.icon ?? "";

	function update(patch: Record<string, unknown>, immediate = true) {
		settings = { ...settings, ...patch };
		if (immediate) void persist();
		else persistSoon();
	}

	// Takes the settings explicitly so templates re-render when they change.
	function textValue(source: any, keys: string[], fallback = ""): string {
		for (const key of keys) {
			if (typeof source?.[key] === "string") return source[key];
		}
		return fallback;
	}

	/** Write to whichever of the accepted keys the imported settings already use. */
	function setTextFor(keys: string[], fallbackKey: string, value: string) {
		const key = keys.find((candidate) => Object.prototype.hasOwnProperty.call(settings ?? {}, candidate)) ?? fallbackKey;
		update({ [key]: value }, false);
	}

	async function choosePath(key: string, filters: { name: string; extensions: string[] }[] = [], directory = false) {
		const selected = await open({ multiple: false, directory, ...(filters.length ? { filters } : {}) });
		if (typeof selected === "string") update({ [key]: selected });
	}

	function settingLabel(key: string): string {
		return key.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
	}

	$: hotkeyList = (Array.isArray(settings.hotkeys) ? settings.hotkeys : []) as { down: string; up: string; display?: string }[];
	function hotkeys(): { down: string; up: string; display?: string }[] {
		return hotkeyList;
	}

	function setHotkey(index: number, patch: { down?: string; up?: string; display?: string }) {
		const next = hotkeys().map((hotkey) => ({ ...hotkey }));
		next[index] = { ...next[index], ...patch };
		update({ hotkeys: next });
	}

	function addHotkey() {
		update({ hotkeys: [...hotkeys(), { down: "", up: "" }] });
	}

	// A shortcut is removed together with its image and title.
	async function removeHotkey(index: number) {
		if (pendingTimer !== undefined) await persist();
		await flushStates();
		try {
			const updated = await invoke<ActionInstance | null>("remove_switch_shortcut", { context: instance.context, index });
			if (!updated) return;
			settings = structuredClone(updated.settings);
			instance.settings = updated.settings;
			instance.states = updated.states;
			instance.current_state = updated.current_state;
			dispatch("edit");
		} catch (error) {
			toast("error", "Could not remove the shortcut", errorText(error));
		}
	}

	function setMouseModifier(modifier: string, checked: boolean) {
		const selected = new Set<string>(Array.isArray(settings.modifiers) ? settings.modifiers : []);
		checked ? selected.add(modifier.toLowerCase()) : selected.delete(modifier.toLowerCase());
		update({ modifiers: [...selected] });
	}

	async function captureMousePosition() {
		mousePositionError = "";
		try {
			const [x, y] = await invoke<[number, number]>("get_mouse_position");
			update({ x, y, coordinate: "absolute" });
		} catch (error) {
			mousePositionError = String(error);
		}
	}

	// Super Hotkey: "hold" keeps the shortcut pressed while the M18 key is held.
	let superHoldMode: "tap" | "hold" = "tap";
	$: superHoldMode = typeof settings.up === "string" && settings.up.trim() !== "" ? "hold" : "tap";
	let superModeOverride: "tap" | "hold" | null = null;
	$: if (context) superModeOverride = null;
	let superMode: "tap" | "hold" = "tap";
	$: superMode = superModeOverride ?? superHoldMode;

	const soundModes = ["Play/Stop", "Play/Overlap", "Play/Replay", "Loop/Stop"];
	const soundModeHelp: Record<string, string> = {
		"Play/Stop": "Press to play; press again to stop.",
		"Play/Overlap": "Every press starts another copy, layered on top.",
		"Play/Replay": "Every press restarts the sound from the beginning.",
		"Loop/Stop": "Press to loop continuously; press again to stop.",
	};
	const mouseEvents = [
		["click", "Click"],
		["doubleClick", "Double-click"],
		["move", "Move"],
		["scroll", "Scroll"],
		["drag", "Drag"],
	];
</script>

<div class="mx-auto flex max-w-2xl flex-col gap-4 px-5 py-4">
	{#if comingSoon}
		<div class="notice notice-warning">
			<Warning size="16" class="mt-px shrink-0" />
			<span>This action is not available yet. Its VSD Craft settings are kept so it can work once it is implemented; pressing the key shows an alert.</span>
		</div>
	{/if}

	{#if isOpenApps}
		<div class="field">
			<label class="label" for="m18-app-path">Application</label>
			<div class="flex gap-2">
				<input id="m18-app-path" class="input" value={settings.appPath ?? ""} on:input={(event) => update({ appPath: event.currentTarget.value }, false)} placeholder="Safari, com.apple.Safari, or /Applications/Safari.app" />
				<button class="btn shrink-0" on:click={() => choosePath("appPath", [{ name: "Applications", extensions: ["app", "exe"] }])}><FolderOpen size="14" /> Choose…</button>
			</div>
			<p class="hint">An app name, bundle identifier, or path. The key shows the app's icon automatically.</p>
		</div>
	{:else if isVsdHotkey}
		<div class="field">
			<span class="label">Shortcut</span>
			<ShortcutRecorder
				value={textValue(settings, ["down", "Down"])}
				display={settings.display ?? ""}
				label="Hotkey shortcut"
				on:change={({ detail }) => update({ down: detail.down, display: detail.display, up: settings.up ?? "" })}
			/>
			<p class="hint">Sent when the key is released.</p>
		</div>
		<details class="group">
			<summary class="label list-none select-none">▸ Advanced: extra sequence on release</summary>
			<div class="mt-2">
				<ShortcutRecorder value={textValue(settings, ["up", "Up"])} label="Release sequence" on:change={({ detail }) => update({ up: detail.down })} />
			</div>
		</details>
	{:else if isSuperHotkeys || isVsdSuperHotkey}
		<div class="flex items-center justify-between gap-3">
			<span class="label">When the M18 key is pressed</span>
			<div class="segmented">
				<button aria-pressed={superMode === "tap"} on:click={() => (superModeOverride = "tap")}>Tap the shortcut</button>
				<button aria-pressed={superMode === "hold"} on:click={() => (superModeOverride = "hold")}>Hold it down</button>
			</div>
		</div>
		<div class="field">
			<span class="label">Shortcut</span>
			<ShortcutRecorder
				value={textValue(settings, ["down", "Down"])}
				display={settings.display ?? ""}
				mode={superMode}
				label="Super Hotkey shortcut"
				on:change={({ detail }) => update({ down: detail.down, up: detail.up ?? "", display: detail.display })}
			/>
			<p class="hint">
				{superMode === "hold" ? "The keys stay pressed while you hold the M18 key, like push-to-talk, and are released when you let go." : "The whole shortcut is sent the moment the key is pressed."}
			</p>
		</div>
		<div class="notice notice-info">
			<Info size="16" class="mt-px shrink-0" />
			<span>VSD Craft sends Super Hotkeys as a hardware keyboard. This app sends them as software input, which works in almost every app.</span>
		</div>
	{:else if isHotkeySwitch || isSuperHotkeySwitch}
		<p class="hint">Each press sends one shortcut and moves on to the next. The key shows the image and title of the shortcut that the next press sends.</p>
		<div class="flex flex-col gap-2">
			{#each hotkeyList as hotkey, index}
				{@const state = instance.states[index]}
				{@const next = Number(settings.index ?? 0) === index}
				<div class="card flex items-start gap-3 p-3" class:next-shortcut={next}>
					<div class="flex w-16 shrink-0 flex-col items-center gap-1">
						<button
							class="group relative size-16 overflow-hidden rounded-[22%] shadow-[inset_0_0_0_1px_rgb(255_255_255/0.08)]"
							on:click={() => chooseImage(index)}
							on:dragover|preventDefault
							on:drop|preventDefault={(event) => useImage(event.dataTransfer?.files?.[0], index)}
							title="Choose an image for this shortcut"
							aria-label={`Choose an image for shortcut ${index + 1}`}
						>
							<KeyFace {state} fallback={defaultFace} />
							<span class="absolute inset-0 flex items-center justify-center bg-black/60 opacity-0 transition-opacity group-hover:opacity-100"><ImageSquare size="18" class="text-white" /></span>
						</button>
						{#if state && !isDefaultArtwork(state.image)}
							<button class="whitespace-nowrap text-[10.5px] text-ink-faint hover:text-ink-muted" on:click={() => setAppearance(index, { image: defaultFace, image_scale: 100 })}>Default image</button>
						{:else}
							<span class="whitespace-nowrap text-[10.5px] text-ink-faint tabular-nums">Shortcut {index + 1}</span>
						{/if}
					</div>
					<div class="flex min-w-0 flex-1 flex-col gap-2">
						<ShortcutRecorder value={hotkey.down ?? ""} display={hotkey.display ?? ""} label={`Shortcut ${index + 1}`} on:change={({ detail }) => setHotkey(index, { down: detail.down, display: detail.display })} />
						<input
							class="input"
							value={state?.text ?? ""}
							placeholder="Title on the key, such as START SCRIPT"
							disabled={!state}
							aria-label={`Title for shortcut ${index + 1}`}
							on:input={(event) => setAppearance(index, { text: event.currentTarget.value, show: true }, false)}
						/>
					</div>
					<div class="flex shrink-0 flex-col items-end gap-1.5">
						{#if next}<span class="badge text-accent">Next</span>{/if}
						<button class="btn btn-ghost btn-icon" on:click={() => removeHotkey(index)} disabled={hotkeyList.length < 2} aria-label={`Remove shortcut ${index + 1}`}><Trash size="14" /></button>
					</div>
				</div>
			{/each}
		</div>
		<input
			bind:this={imageInput}
			type="file"
			accept="image/*"
			class="hidden"
			on:change={() => {
				useImage(imageInput.files?.[0], imageFor);
				imageInput.value = "";
			}}
		/>
		<div class="flex items-center justify-between gap-3">
			<button class="btn btn-sm" on:click={addHotkey}><Plus size="13" weight="bold" /> Add shortcut</button>
			<label class="flex items-center gap-2 text-xs text-ink-muted">
				Next press sends
				<select class="select h-7 min-h-0 w-auto py-0" value={settings.index ?? 0} on:change={(event) => update({ index: Number(event.currentTarget.value) })}>
					{#each hotkeyList as hotkey, index}<option value={index}>{index + 1}{hotkey.down ? ` · ${describeSequenceText(hotkey.down) || "custom"}` : ""}</option>{/each}
				</select>
			</label>
		</div>
	{:else if isFolderOpen || isFolderBack}
		{#if isFolderOpen}
			<div class="field">
				<label class="label" for="m18-folder-target">Folder contents</label>
				<select id="m18-folder-target" class="select" value={pages.findIndex((page) => page.profile === settings.profile)} on:change={(event) => update({ profile: pages[Number(event.currentTarget.value)]?.profile ?? "" })} disabled={pages.length === 0}>
					<option value={-1}>Choose a page…</option>
					{#each pages as page, index}<option value={index}>{pageLabel(page, index)}</option>{/each}
				</select>
				<p class="hint">Opens that page as a folder. Put a Go Back key on it to return here.</p>
			</div>
		{:else}
			<p class="hint">Returns to the page that opened the current folder. Nothing to set up.</p>
		{/if}
	{:else if isSceneShift}
		<div class="field">
			<label class="label" for="m18-scene-target">Switch to</label>
			<select id="m18-scene-target" class="select" value={pages.findIndex((page) => page.profile === settings.profile)} on:change={(event) => update({ profile: pages[Number(event.currentTarget.value)]?.profile ?? "" })}>
				<option value={-1}>The next page</option>
				{#each pages as page, index}<option value={index}>{pageLabel(page, index)}</option>{/each}
			</select>
		</div>
	{:else if isPageGoto}
		<div class="field">
			<label class="label" for="m18-page-target">Page</label>
			<select id="m18-page-target" class="select" value={settings.pageIndex ?? 0} on:change={(event) => update({ page: pages[Number(event.currentTarget.value)]?.profile ?? "", pageIndex: Number(event.currentTarget.value) })}>
				{#each pages as page, index}<option value={index}>{pageLabel(page, index)}</option>{/each}
			</select>
		</div>
		<label class="flex items-center justify-between gap-3">
			<span>
				<span class="block text-[13px] text-ink">Show the page number on the key</span>
				<span class="hint">Drawn over the key's image unless the key has a title.</span>
			</span>
			<input type="checkbox" class="switch" checked={settings.showPageNumber !== false} on:change={(event) => update({ showPageNumber: event.currentTarget.checked })} />
		</label>
	{:else if isDelay}
		<div class="field max-w-60">
			<label class="label" for="vsd-delay">Wait</label>
			<div class="flex items-center gap-2">
				<input id="vsd-delay" type="number" min="0" max="300000" step="100" class="input tabular-nums" value={Number(settings.delay ?? settings.Delay ?? settings.time ?? settings.Time ?? 0)} on:input={(event) => update({ delay: Number(event.currentTarget.value) }, false)} />
				<span class="text-xs text-ink-muted">ms</span>
			</div>
			<p class="hint">Only meaningful inside a Multi Action, between two steps.</p>
		</div>
	{:else if isTextAction}
		<div class="field">
			<label class="label" for="vsd-text-value">Text to type</label>
			<textarea id="vsd-text-value" rows="5" class="textarea" value={textValue(settings, ["text", "Text", "value", "Value"])} on:input={(event) => setTextFor(["text", "Text", "value", "Value"], "text", event.currentTarget.value)} placeholder="Typed into the app in front when the key is pressed"></textarea>
			<p class="hint">Typed into whichever app is in front. Line breaks are typed as Return.</p>
		</div>
	{:else if isPasswordAction}
		<div class="field">
			<label class="label" for="vsd-password-value">Password</label>
			<div class="relative">
				<input id="vsd-password-value" type={revealPassword ? "text" : "password"} autocomplete="new-password" spellcheck="false" class="input pr-9" value={textValue(settings, ["password", "Password", "text", "Text"])} on:input={(event) => setTextFor(["password", "Password", "text", "Text"], "password", event.currentTarget.value)} />
				<button class="btn btn-ghost btn-sm btn-icon absolute top-1/2 right-1 -translate-y-1/2" on:click={() => (revealPassword = !revealPassword)} aria-label={revealPassword ? "Hide password" : "Show password"}>
					{#if revealPassword}<EyeSlash size="14" />{:else}<Eye size="14" />{/if}
				</button>
			</div>
		</div>
		<div class="notice notice-warning">
			<Warning size="16" class="mt-px shrink-0" />
			<span>The password is saved unencrypted in this profile, and in configuration backups. Anyone with access to your Mac account can read it.</span>
		</div>
	{:else if isOpenAction || isWebsiteAction || isCloseAction}
		<div class="field">
			<label class="label" for="vsd-open-target">{isCloseAction ? "App to quit" : isWebsiteAction ? "Web address" : "File, folder, or app"}</label>
			<div class="flex gap-2">
				<input
					id="vsd-open-target"
					type={isWebsiteAction ? "url" : "text"}
					class="input"
					value={isCloseAction ? textValue(settings, ["appPath", "path", "application", "app", "bundleId"]) : textValue(settings, ["path", "Path", "url", "URL", "website", "link", "file", "folder"])}
					on:input={(event) =>
						setTextFor(isCloseAction ? ["appPath", "path", "application", "app", "bundleId"] : ["path", "Path", "url", "URL", "website", "link", "file", "folder"], isCloseAction ? "appPath" : "path", event.currentTarget.value)}
					placeholder={isWebsiteAction ? "https://example.com" : isCloseAction ? "Safari" : "~/Documents/Report.pdf"}
				/>
				{#if isOpenAction}
					<button class="btn shrink-0" on:click={() => choosePath("path")}>File…</button>
					<button class="btn shrink-0" on:click={() => choosePath("path", [], true)}>Folder…</button>
				{/if}
			</div>
			<p class="hint">{isCloseAction ? "The app is asked to quit, exactly as if you chose Quit from its menu." : isWebsiteAction ? "Opens in your default browser." : "Opens with its default app."}</p>
		</div>
	{:else if isUdpAction}
		<div class="grid grid-cols-[1fr_7rem] gap-3">
			<div class="field">
				<label class="label" for="vsd-udp-host">Host or IP address</label>
				<input id="vsd-udp-host" class="input" value={textValue(settings, ["host", "Host", "ip", "IP", "address", "Address"], "127.0.0.1")} on:input={(event) => setTextFor(["host", "Host", "ip", "IP", "address", "Address"], "host", event.currentTarget.value)} />
			</div>
			<div class="field">
				<label class="label" for="vsd-udp-port">Port</label>
				<input id="vsd-udp-port" type="number" min="1" max="65535" class="input tabular-nums" value={settings.port ?? settings.Port ?? 5000} on:input={(event) => update({ [Object.prototype.hasOwnProperty.call(settings ?? {}, "Port") ? "Port" : "port"]: Number(event.currentTarget.value) }, false)} />
			</div>
		</div>
		<div class="field">
			<label class="label" for="vsd-udp-message">Message</label>
			<textarea id="vsd-udp-message" rows="3" class="textarea input-mono" value={textValue(settings, ["message", "Message", "data", "Data"])} on:input={(event) => setTextFor(["message", "Message", "data", "Data"], "message", event.currentTarget.value)}></textarea>
			<p class="hint">Sent as one UTF-8 datagram each time the key is pressed.</p>
		</div>
	{:else if isBrightnessAction}
		<div class="flex items-center justify-between gap-3">
			<span class="label">Each press</span>
			<div class="segmented">
				<button aria-pressed={Number(settings.actionIdx ?? 0) === 0} on:click={() => update({ actionIdx: 0 })}>Brighter</button>
				<button aria-pressed={Number(settings.actionIdx ?? 0) === 1} on:click={() => update({ actionIdx: 1 })}>Dimmer</button>
			</div>
		</div>
		<p class="hint">Changes the M18's own screen, not the Mac's display.</p>
	{:else if isMultimediaAction}
		<div class="field">
			<label class="label" for="vsd-multimedia-action">Media key</label>
			<select id="vsd-multimedia-action" class="select" value={settings.actionIdx ?? 1} on:change={(event) => update({ actionIdx: Number(event.currentTarget.value) })}>
				<option value="0">Previous track</option><option value="1">Play / Pause</option><option value="2">Next track</option><option value="3">Stop</option><option value="4">Mute</option><option value="5">Volume up</option><option value="6">Volume down</option>
			</select>
			{#if Number(settings.actionIdx ?? 1) === 3}
				<p class="hint">macOS has no system-wide Stop key; Stop pauses the Music app if it is running.</p>
			{/if}
		</div>
	{:else if isMouseEventAction}
		<div class="field">
			<span class="label">Action</span>
			<div class="segmented self-start">
				{#each mouseEvents as [value, text]}
					<button aria-pressed={(settings.eventType ?? "click") === value} on:click={() => update({ eventType: value })}>{text}</button>
				{/each}
			</div>
		</div>
		{#if ["click", "doubleClick", "drag"].includes(settings.eventType ?? "click")}
			<div class="field max-w-60">
				<label class="label" for="vsd-mouse-button">Button</label>
				<select id="vsd-mouse-button" class="select" value={settings.button ?? "left"} on:change={(event) => update({ button: event.currentTarget.value })}>
					<option value="left">Left</option><option value="right">Right</option><option value="middle">Middle</option><option value="side1">Back (side 1)</option><option value="side2">Forward (side 2)</option>
				</select>
			</div>
		{/if}
		{#if settings.eventType === "move" || settings.eventType === "drag"}
			<div class="grid grid-cols-2 gap-3">
				<div class="field">
					<label class="label" for="vsd-mouse-x">X</label>
					<input id="vsd-mouse-x" type="number" class="input tabular-nums" value={settings.x ?? 0} on:input={(event) => update({ x: Number(event.currentTarget.value) }, false)} />
				</div>
				<div class="field">
					<label class="label" for="vsd-mouse-y">Y</label>
					<input id="vsd-mouse-y" type="number" class="input tabular-nums" value={settings.y ?? 0} on:input={(event) => update({ y: Number(event.currentTarget.value) }, false)} />
				</div>
			</div>
			<div class="flex flex-wrap items-center gap-3">
				<div class="segmented">
					<button aria-pressed={(settings.coordinate ?? "absolute") === "absolute"} on:click={() => update({ coordinate: "absolute" })}>Screen position</button>
					<button aria-pressed={settings.coordinate === "relative"} on:click={() => update({ coordinate: "relative" })}>Relative to pointer</button>
				</div>
				<button class="btn btn-sm" on:click={captureMousePosition}><Crosshair size="14" /> Use current pointer position</button>
			</div>
			{#if mousePositionError}<p role="alert" class="text-xs text-danger">{mousePositionError}</p>{/if}
		{/if}
		{#if settings.eventType === "scroll"}
			<div class="grid grid-cols-2 gap-3">
				<div class="field">
					<label class="label" for="vsd-mouse-amount">Amount</label>
					<input id="vsd-mouse-amount" type="number" class="input tabular-nums" value={settings.amount ?? 3} on:input={(event) => update({ amount: Number(event.currentTarget.value) }, false)} />
				</div>
				<div class="field">
					<span class="label">Direction</span>
					<div class="segmented self-start">
						<button aria-pressed={(settings.axis ?? "vertical") === "vertical"} on:click={() => update({ axis: "vertical" })}>Vertical</button>
						<button aria-pressed={settings.axis === "horizontal"} on:click={() => update({ axis: "horizontal" })}>Horizontal</button>
					</div>
				</div>
			</div>
		{/if}
		<div class="field">
			<span class="label">Hold modifier keys</span>
			<div class="flex flex-wrap gap-4 text-[13px]">
				{#each ["Shift", "Control", "Alt", "Command"] as modifier}
					<label class="flex items-center gap-2">
						<input type="checkbox" class="switch" checked={(Array.isArray(settings.modifiers) ? settings.modifiers : []).includes(modifier.toLowerCase())} on:change={(event) => setMouseModifier(modifier, event.currentTarget.checked)} />
						{modifier === "Alt" ? "Option" : modifier}
					</label>
				{/each}
			</div>
		</div>
		<p class="hint">Simulating the mouse needs Accessibility access for OpenDeck VSD M18 in System Settings › Privacy & Security.</p>
	{:else if isMicrophoneAction}
		<p class="text-[13px] text-ink-muted">Mutes or unmutes the Mac's current default microphone. If that microphone has no mute control, the key shows an alert instead.</p>
	{:else if isEmojiAction}
		<div class="field">
			<label class="label" for="vsd-emoji">Emoji or text to type</label>
			<input id="vsd-emoji" class="input text-lg" value={textValue(settings, ["emoji", "Emoji", "text", "Text", "value"])} on:input={(event) => setTextFor(["emoji", "Emoji", "text", "Text", "value"], "emoji", event.currentTarget.value)} placeholder="👍" />
			<p class="hint">Leave empty to open the Emoji & Symbols picker instead.</p>
		</div>
	{:else if isSoundboardAction}
		<div class="field">
			<label class="label" for="vsd-sound-path">Sound file</label>
			<div class="flex gap-2">
				<input id="vsd-sound-path" class="input" value={textValue(settings, ["path", "filePath", "audio", "musicUrl"])} on:input={(event) => setTextFor(["path", "filePath", "audio", "musicUrl"], "path", event.currentTarget.value)} placeholder="Choose an audio file" />
				<button class="btn shrink-0" on:click={() => choosePath("path", [{ name: "Audio", extensions: ["mp3", "wav", "mp4", "m4a", "m4b", "m4p", "mov", "aiff", "aif", "flac", "ogg"] }])}><FolderOpen size="14" /> Choose…</button>
			</div>
		</div>
		<div class="field">
			<span class="label">Playback</span>
			<div class="segmented self-start">
				{#each soundModes as mode}<button aria-pressed={(settings.mode ?? "Play/Stop") === mode} on:click={() => update({ mode })}>{mode}</button>{/each}
			</div>
			<p class="hint">{soundModeHelp[settings.mode ?? "Play/Stop"] ?? ""}</p>
		</div>
		<div class="grid grid-cols-2 gap-4">
			<div class="field">
				<label class="label" for="vsd-sound-volume">Volume · {settings.volume ?? 100}%</label>
				<input id="vsd-sound-volume" type="range" min="0" max="100" class="range" style={`--range-fill: ${settings.volume ?? 100}%`} value={settings.volume ?? 100} on:input={(event) => update({ volume: Number(event.currentTarget.value) }, false)} />
			</div>
			<div class="field">
				<label class="label" for="vsd-sound-output">Output</label>
				<select id="vsd-sound-output" class="select" value={settings.outputDevice ?? settings.device?.description ?? settings.device?.name ?? settings.device?.id ?? "default"} on:change={(event) => update({ outputDevice: event.currentTarget.value })}>
					<option value="default">System default</option>
					{#each audioOutputDevices as outputDevice}<option value={outputDevice.id}>{outputDevice.name}</option>{/each}
				</select>
			</div>
		</div>
		<div class="grid grid-cols-[1fr_8rem] gap-4">
			<div class="field">
				<label class="label" for="vsd-sound-fade">Fade</label>
				<select id="vsd-sound-fade" class="select" value={settings.fadeType ?? "none"} on:change={(event) => update({ fadeType: event.currentTarget.value })}>
					<option value="none">None</option><option value="fadeIn">Fade in</option><option value="fadeOut">Fade out</option><option value="fadeInOut">Fade in and out</option>
				</select>
			</div>
			{#if (settings.fadeType ?? "none") !== "none"}
				<div class="field">
					<label class="label" for="vsd-sound-fade-duration">Seconds</label>
					<input id="vsd-sound-fade-duration" type="number" min="0" max="3600" class="input tabular-nums" value={settings.fadeDuration ?? 0} on:input={(event) => update({ fadeDuration: Number(event.currentTarget.value) }, false)} />
				</div>
			{/if}
		</div>
	{:else if isUnsupportedVsd}
		<div class="notice notice-warning">
			<Warning size="16" class="mt-px shrink-0" />
			<span>This key came from VSD Craft, but its action isn't supported yet. Its image, title, and settings are kept; pressing it shows an alert.</span>
		</div>
		<dl class="grid grid-cols-[7rem_1fr] gap-x-3 gap-y-1 text-xs">
			<dt class="text-ink-faint">Original action</dt>
			<dd class="text-ink">{settings.sourceName ?? "Unknown"}</dd>
			<dt class="text-ink-faint">Identifier</dt>
			<dd class="font-mono break-all text-ink-muted">{settings.sourceUuid ?? "Unknown"}</dd>
		</dl>
	{:else if isVsdCoreAction}
		{#if Object.keys(settings ?? {}).length > 0}
			<p class="hint">Settings imported from VSD Craft:</p>
			<div class="flex flex-col gap-3">
				{#each Object.keys(settings) as key}
					{#if typeof settings[key] === "boolean"}
						<label class="flex items-center justify-between gap-3 text-[13px]" for={`vsd-setting-${key}`}>
							{settingLabel(key)}
							<input id={`vsd-setting-${key}`} type="checkbox" class="switch" checked={settings[key]} on:change={(event) => update({ [key]: event.currentTarget.checked })} />
						</label>
					{:else if typeof settings[key] === "number"}
						<div class="field">
							<label class="label" for={`vsd-setting-${key}`}>{settingLabel(key)}</label>
							<input id={`vsd-setting-${key}`} type="number" class="input tabular-nums" value={settings[key]} on:input={(event) => update({ [key]: Number(event.currentTarget.value) }, false)} />
						</div>
					{:else if typeof settings[key] === "string"}
						<div class="field">
							<label class="label" for={`vsd-setting-${key}`}>{settingLabel(key)}</label>
							<input id={`vsd-setting-${key}`} class="input" value={settings[key]} on:input={(event) => update({ [key]: event.currentTarget.value }, false)} />
						</div>
					{:else}
						<div class="field">
							<label class="label" for={`vsd-setting-${key}`}>{settingLabel(key)} (JSON)</label>
							<textarea
								id={`vsd-setting-${key}`}
								rows="3"
								class="textarea input-mono"
								value={JSON.stringify(settings[key], null, 2)}
								on:change={(event) => {
									try {
										update({ [key]: JSON.parse(event.currentTarget.value) });
									} catch {
										// Keep the last valid value until the JSON parses.
									}
								}}
							></textarea>
						</div>
					{/if}
				{/each}
			</div>
		{:else}
			<p class="hint">{libraryEntry?.action.tooltip ?? instance.action.tooltip ?? "This action"} — nothing to set up.</p>
		{/if}
	{:else}
		<p class="hint">{libraryEntry?.action.tooltip ?? instance.action.tooltip ?? "This action"} — nothing to set up.</p>
	{/if}
</div>

<style>
	.next-shortcut {
		border-color: rgb(139 123 255 / 0.45);
		box-shadow: 0 0 0 1px rgb(139 123 255 / 0.18);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}
	details[open] summary {
		color: var(--color-ink);
	}
</style>
