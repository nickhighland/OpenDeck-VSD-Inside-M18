<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { Context } from "$lib/Context";
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";

	import CursorClick from "phosphor-svelte/lib/CursorClick";
	import FlowArrow from "phosphor-svelte/lib/FlowArrow";
	import HandPointing from "phosphor-svelte/lib/HandPointing";
	import Play from "phosphor-svelte/lib/Play";
	import Trash from "phosphor-svelte/lib/Trash";
	import X from "phosphor-svelte/lib/X";
	import InstanceEditor from "./InstanceEditor.svelte";
	import M18ActionInspector from "./M18ActionInspector.svelte";
	import M18LedColors from "./M18LedColors.svelte";

	import { COMING_SOON, groupColor, isBuiltIn, isFlowParent } from "$lib/actionLibrary";
	import { actionIndex, plugins } from "$lib/catalog";
	import { t } from "$lib/i18n";
	import { getWebserverUrl, getWebSocketPort } from "$lib/ports";
	import { findInstance, parseActionContext, removeInstance, inspectedInstance, inspectedParentAction, inspectorTab } from "$lib/propertyInspector";
	import { resolveState } from "$lib/appIcons";
	import { getImage } from "$lib/rendererHelper";
	import { attempt } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount } from "svelte";

	let iframes: { [context: string]: HTMLIFrameElement } = {};
	let iframeContainer: HTMLDivElement;
	let iframeClosePopup: HTMLButtonElement;
	let iframePopupsOpen: string[] = [];

	export let device: DeviceInfo;
	export let profile: Profile;

	async function iframeOnLoad(event: Event, instance: ActionInstance) {
		const iframe = iframes[instance.context] ?? event.target;
		const parsedContext = parseActionContext(instance.context);

		const position = parsedContext.position;
		let coordinates: { row: number; column: number };
		if (parsedContext.controller == "Encoder") {
			coordinates = { row: 0, column: position };
		} else {
			coordinates = { row: Math.floor(position / device.columns), column: position % device.columns };
		}

		if (instance == null || !iframe?.src || !iframe.src.startsWith(getWebserverUrl())) return;
		const info = JSON.stringify(await invoke("make_info", { plugin: instance.action.plugin }));

		iframe?.contentWindow?.postMessage(
			{
				event: "connect",
				payload: [
					getWebSocketPort(),
					instance.context,
					"registerPropertyInspector",
					info,
					JSON.stringify({
						action: instance.action.uuid,
						context: instance.context,
						device: parsedContext.device,
						payload: {
							settings: instance.settings,
							coordinates,
							controller: parsedContext.controller,
							state: instance.current_state,
							isInMultiAction: !parsedContext.root,
						},
					}),
				],
			},
			getWebserverUrl(),
		);
	}

	const closePopup = (context: string) => {
		const iframe = iframes[context];
		if (iframe) {
			iframe.style.position = "";
			iframe.style.left = "";
			iframe.style.top = "";
			iframe.style.width = "100%";
			iframe.style.height = "100%";
			iframe.style.display = $inspectedInstance == context ? "block" : "none";
			iframe.contentWindow?.postMessage({ event: "windowClosed" }, getWebserverUrl());
		}

		iframePopupsOpen = iframePopupsOpen.filter((e) => e != context);

		if (iframePopupsOpen.length == 0) {
			iframeContainer.style.position = "";
			iframeContainer.style.inset = "";
			iframeContainer.style.padding = "";
			iframeContainer.style.zIndex = "";
			iframeContainer.style.background = "";
			iframeClosePopup.style.display = "none";
		}
	};

	function handleMessage({ data, origin }: MessageEvent) {
		// Only property inspectors (served by the plugin webserver) may drive the editor.
		if (origin !== getWebserverUrl().replace(/\/$/, "")) return;
		if (data.event == "windowOpened") {
			const iframe = iframes[data.payload];
			if (!iframe) return;
			iframe.style.position = "absolute";
			iframe.style.left = "36px";
			iframe.style.top = "36px";
			iframe.style.width = "calc(100% - 72px)";
			iframe.style.height = "calc(100% - 72px)";
			iframe.style.display = "block";

			iframePopupsOpen.push(data.payload);

			iframeContainer.style.position = "fixed";
			iframeContainer.style.inset = "0";
			iframeContainer.style.padding = "36px";
			iframeContainer.style.zIndex = "45";
			iframeContainer.style.background = "rgb(0 0 0 / 0.6)";

			iframeClosePopup.style.display = "flex";
		} else if (data.event == "windowClosed") {
			closePopup(data.payload);
		} else if (data.event == "openUrl") {
			invoke("open_url", { url: data.payload });
		} else if (data.event == "fetch") {
			function combineUint8Arrays(arrays: Uint8Array[]): Uint8Array {
				const totalLength = arrays.reduce((acc, curr) => acc + curr.length, 0);
				let mergedArray = new Uint8Array(totalLength);
				let offset = 0;

				arrays.forEach((item) => {
					mergedArray.set(item, offset);
					offset += item.length;
				});

				return mergedArray;
			}

			window
				// @ts-expect-error
				.fetchCORS(...data.payload.args)
				.then(async (response: Response) => {
					const chunks = [];
					if (response.body) {
						const reader = response.body.getReader();
						while (true) {
							const { done, value } = await reader.read();
							if (done) break;
							chunks.push(value);
						}
					}
					const body = combineUint8Arrays(chunks);

					iframes[data.payload.context]?.contentWindow?.postMessage(
						{
							event: "fetchResponse",
							payload: {
								id: data.payload.id,
								response: {
									url: response.url,
									body,
									headers: response.headers.entries().toArray(),
									status: response.status,
									statusText: response.statusText,
								},
							},
						},
						getWebserverUrl(),
					);
				})
				.catch((error: any) => {
					iframes[data.payload.context]?.contentWindow?.postMessage({ event: "fetchError", payload: { id: data.payload.id, error } }, getWebserverUrl());
				});
		}
	}

	const nonNull = <T,>(o: T | null): o is T => o != null;
	function flatten(instance: ActionInstance, result: ActionInstance[]) {
		result.push(instance);
		for (const child of instance.children ?? []) flatten(child, result);
	}
	$: instances = [...profile.keys, ...profile.sliders, ...profile.infobars].filter(nonNull).reduce((all, instance) => {
		flatten(instance, all);
		return all;
	}, [] as ActionInstance[]);

	onMount(() => {
		window.addEventListener("message", handleMessage);
		let unlisten: (() => void) | undefined;
		let disposed = false;
		listen<string>("plugin_reloaded", ({ payload }) => {
			for (const instance of instances) {
				if (instance.action.plugin == payload && iframes[instance.context]) {
					iframes[instance.context].src += "";
					if ($inspectedInstance == instance.context) {
						invoke("switch_property_inspector", { new: instance.context });
					}
				}
			}
		}).then((stop) => (disposed ? stop() : (unlisten = stop)));
		return () => {
			disposed = true;
			window.removeEventListener("message", handleMessage);
			unlisten?.();
		};
	});

	// What the inspector is showing.
	$: inspectedContext = typeof $inspectedInstance === "string" ? $inspectedInstance : null;
	$: inspected = inspectedContext ? findInstance(profile, inspectedContext) : undefined;
	$: emptySlot = $inspectedInstance && typeof $inspectedInstance === "object" ? ($inspectedInstance as Context) : null;
	$: slotInstance = emptySlot && emptySlot.device === device.id && emptySlot.profile === profile.id ? profile.keys[emptySlot.position] : null;

	function slotLabel(position: number): string {
		return position >= 15 ? `Button ${position - 14}` : `Key ${position + 1}`;
	}

	$: parsed = inspected ? parseActionContext(inspected.context) : null;
	$: entry = inspected ? $actionIndex.get(inspected.action.uuid) : undefined;
	$: builtIn = inspected ? isBuiltIn(inspected.action) : false;
	$: title = inspected ? (entry && builtIn ? entry.action.name : inspected.action.name) : "";
	$: group = entry && builtIn ? entry.category : undefined;
	$: pluginInfo = inspected && !builtIn ? $plugins.find((plugin) => plugin.id === inspected?.action.plugin) : undefined;
	let faceUrl = "";
	let faceRequest = 0;
	// The appearance editor changes the inspected key in place. Counting its
	// edits redraws the header without invalidating the bound profile, which
	// would redraw every key on the device.
	let edits = 0;
	async function showFace(shown: ActionInstance | null | undefined, _edits: number) {
		const request = ++faceRequest;
		const current = shown?.states[shown.current_state];
		if (!shown || !current) {
			faceUrl = shown ? getImage(undefined, shown.action.icon) : "";
			return;
		}
		// Launching keys without a chosen image show the app's icon.
		const { state } = await resolveState(shown, current);
		if (request === faceRequest) faceUrl = getImage(state.image, shown.action.icon);
	}
	$: showFace(inspected, edits);
	$: description = entry?.action.tooltip || inspected?.action.tooltip || "";
	$: hasBehavior = inspected
		? inspected.action.uuid == "opendeck.m18.led-colors" || inspected.action.uuid.startsWith("opendeck.m18.") || /^com\.(hotspot|mirabox)\.streamdock\.|^com\.streamdock\./.test(inspected.action.uuid) || !!inspected.action.property_inspector
		: false;

	async function testPress() {
		if (!parsed) return;
		const { indices: _indices, index: _index, root: _root, ...slot } = parsed;
		await attempt("Could not run the key", () => invoke("trigger_virtual_press", { context: slot }));
	}

	async function clearKey() {
		if (!inspected || !parsed) return;
		const context = inspected.context;
		const removed = await attempt("Could not clear the key", () => invoke("remove_instance", { context }));
		if (removed === undefined) return;
		removeInstance(profile, context);
		profile = profile;
		$inspectedInstance = null;
	}
</script>

<svelte:window
	on:keydown={(event) => {
		if (event.key == "Escape" && iframePopupsOpen.length > 0) {
			closePopup(iframePopupsOpen[iframePopupsOpen.length - 1]);
		}
	}}
/>

<section class="flex h-[42%] min-h-60 shrink-0 flex-col border-t border-line bg-panel" aria-label="Key settings">
	{#if inspected && parsed}
		<header class="flex shrink-0 items-center gap-3 border-b border-line px-5 py-3">
			<img src={faceUrl} alt="" class="size-10 shrink-0 rounded-[11px] bg-black object-cover shadow-[0_4px_10px_-4px_black]" />
			<div class="min-w-0 flex-1">
				<div class="flex items-center gap-2">
					<h2 class="truncate text-[14px] font-semibold text-ink">{title}</h2>
					{#if group}
						<span class="badge" style={`background: color-mix(in srgb, ${groupColor(group)} 18%, transparent); color: color-mix(in srgb, ${groupColor(group)} 75%, white);`}>{group}</span>
					{:else if pluginInfo}
						<span class="badge">{pluginInfo.name}</span>
					{/if}
					{#if group === COMING_SOON}<span class="badge text-warning">Not available yet</span>{/if}
				</div>
				<p class="truncate text-xs text-ink-faint">
					{!parsed.root ? `Step ${parsed.index} of the flow on ${slotLabel(parsed.position)}` : slotLabel(parsed.position)}{description ? ` · ${description}` : ""}
				</p>
			</div>
			<div class="segmented shrink-0">
				<button aria-pressed={$inspectorTab === "behavior"} on:click={() => ($inspectorTab = "behavior")}>Behavior</button>
				<button aria-pressed={$inspectorTab === "appearance"} on:click={() => ($inspectorTab = "appearance")}>Appearance</button>
			</div>
			{#if parsed.root}
				<button class="btn btn-sm shrink-0" on:click={testPress} title="Run this key now (or double-click it)"><Play size="13" weight="fill" /> Test</button>
			{/if}
			<button class="btn btn-sm btn-danger btn-icon shrink-0" on:click={clearKey} title={!parsed.root ? "Remove this step" : "Clear this key"} aria-label={!parsed.root ? "Remove this step" : "Clear this key"}><Trash size="14" /></button>
		</header>
	{:else if slotInstance && isFlowParent(slotInstance.action.uuid)}
		<div class="flex flex-1 items-center justify-center gap-4 px-6">
			<FlowArrow size="28" class="text-group-flows" />
			<div>
				<p class="text-[13px] font-semibold text-ink">{slotInstance.action.name} · {slotInstance.children?.length ?? 0} steps</p>
				<p class="hint">Press Enter or click the key to edit its steps.</p>
			</div>
			<button class="btn btn-primary btn-sm" on:click={() => ($inspectedParentAction = emptySlot)}>Edit steps</button>
		</div>
	{:else if emptySlot}
		<div class="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
			<HandPointing size="26" class="text-ink-faint" />
			<p class="text-[13px] font-semibold text-ink">{slotLabel(emptySlot.position)} is empty</p>
			<p class="hint max-w-sm">Drag an action here from the library, or double-click an action to place it on this key.</p>
		</div>
	{:else}
		<div class="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
			<CursorClick size="26" class="text-ink-faint" />
			<p class="text-[13px] font-semibold text-ink">Select a key to set it up</p>
			<p class="hint max-w-md">
				Click a key to change what it does and how it looks. Double-click a key to try it. Right-click for copy, paste, and more.
			</p>
		</div>
	{/if}

	<div class="relative min-h-0 flex-1 overflow-auto" class:hidden={!inspected || $inspectorTab !== "behavior"} bind:this={iframeContainer}>
		<button
			bind:this={iframeClosePopup}
			on:click={() => closePopup(iframePopupsOpen[iframePopupsOpen.length - 1])}
			class="btn btn-icon absolute top-3 right-3 z-10 hidden"
			aria-label={$t("settings.close")}
		>
			<X size="16" />
		</button>
		{#each instances as instance (instance.context)}
			{#if instance.action.uuid == "opendeck.m18.led-colors"}
				<div class="h-full w-full" class:hidden={$inspectedInstance != instance.context}>
					<M18LedColors {instance} />
				</div>
			{:else if instance.action.uuid.startsWith("opendeck.m18.") || /^com\.(hotspot|mirabox)\.streamdock\.|^com\.streamdock\./.test(instance.action.uuid)}
				<!-- Every built-in M18 and VSD Craft action is configured by the M18 inspector. -->
				<div class="h-full w-full" class:hidden={$inspectedInstance != instance.context}>
					<M18ActionInspector {instance} {device} on:edit={() => (edits += 1)} />
				</div>
			{:else if instance.action.property_inspector}
				<iframe
					title={$t("property_inspector.title")}
					class="h-full w-full rounded-xl bg-white/[0.02]"
					class:hidden={$inspectedInstance != instance.context}
					src={getWebserverUrl(instance.action.property_inspector + "|opendeck_property_inspector")}
					name={instance.context}
					bind:this={iframes[instance.context]}
					on:load={(event) => iframeOnLoad(event, instance)}
				/>
			{/if}
		{/each}
		{#if inspected && !hasBehavior}
			<div class="flex h-full items-center justify-center p-6 text-center">
				<p class="hint">{title} has no settings. Use the Appearance tab to change how it looks.</p>
			</div>
		{/if}
	</div>

	{#if inspected && $inspectorTab === "appearance"}
		<div class="min-h-0 flex-1 overflow-auto px-5 py-4">
			<InstanceEditor instance={inspected} on:edit={() => (edits += 1)} />
		</div>
	{/if}
</section>
