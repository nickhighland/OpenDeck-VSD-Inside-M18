<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { ActionState } from "$lib/ActionState";
	import type { Context } from "$lib/Context";
	import type { CopiedItem } from "$lib/propertyInspector";

	import Clipboard from "phosphor-svelte/lib/Clipboard";
	import Copy from "phosphor-svelte/lib/Copy";
	import Pencil from "phosphor-svelte/lib/Pencil";
	import Trash from "phosphor-svelte/lib/Trash";
	import InstanceEditor from "./InstanceEditor.svelte";

	import { t } from "$lib/i18n";
	import { copiedItem, inspectedInstance, inspectedParentAction, openContextMenu } from "$lib/propertyInspector";
	import { CanvasLock, renderImage } from "$lib/rendererHelper";
	import { settings } from "$lib/settings";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount } from "svelte";
	import { tick } from "svelte";

	export let context: Context | null;
	export let label: string = "";
	export let tabindex: number = 0;
	export let role: string = "gridcell";

	// One-way binding for slot data.
	export let inslot: ActionInstance | null;
	let slot: ActionInstance | null;
	let lastInslot: ActionInstance | null | undefined;
	const update = (inslot: ActionInstance | null) => {
		if (inslot === lastInslot) return;
		if (inslot && context && inslot.context.split(".")[0] != context.device) return;
		lastInslot = inslot;
		slot = inslot;
	};
	$: update(inslot);

	export let active: boolean = true;
	export let scale: number = 1;
	export let isTouchPoint: boolean = false;
	// M18 bottom buttons are physical switches without an LCD. They remain
	// assignable in the editor, but image updates must never be sent for them.
	export let m18Bottom: boolean = false;
	let pressed: boolean = false;

	let state: ActionState | undefined;
	let m18PageSet: { pages: { profile: string }[]; selected: number } = { pages: [], selected: 0 };
	onMount(() => {
		const device = context?.device;
		if (!device?.startsWith("18-")) return;
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void (async () => {
			try {
				const [pageSet, stopListening] = await Promise.all([
					invoke<typeof m18PageSet>("get_m18_pages", { device }),
					listen("m18_pages_changed", ({ payload }: { payload: { device: string; pageSet: typeof m18PageSet } }) => {
						if (payload.device === device) m18PageSet = payload.pageSet;
					}),
				]);
				if (disposed) stopListening();
				else {
					m18PageSet = pageSet;
					unlisten = stopListening;
				}
			} catch {}
		})();
		return () => {
			disposed = true;
			unlisten?.();
		};
	});
	$: {
		if (!slot) {
			state = undefined;
		} else {
			const currentState = slot.states[slot.current_state];
			if (currentState && slot.action.uuid === "opendeck.m18.page-goto" && slot.settings?.showPageNumber !== false && !currentState.text.trim()) {
				state = { ...currentState, text: String(Number(slot.settings?.pageIndex ?? 0) + 1), show: true, alignment: "middle", size: 20, colour: "#ffffff", stroke_colour: "#000000", stroke_size: 1 };
			} else if (currentState && slot.action.uuid === "opendeck.m18.page-indicator") {
				const pageIndex = m18PageSet.pages.findIndex((page) => page.profile === context?.profile);
				const pageNumber = (pageIndex >= 0 ? pageIndex : m18PageSet.selected) + 1;
				state = { ...currentState, text: String(pageNumber), show: true, alignment: "middle", size: 20, colour: "#ffffff", stroke_colour: "#000000", stroke_size: 1 };
			} else {
				state = currentState;
			}
		}
	}

	listen("update_state", ({ payload }: { payload: { context: string; contents: ActionInstance | null } }) => {
		if (payload.context == slot?.context) slot = payload.contents;
	});

	listen("key_moved", ({ payload }: { payload: { context: Context; pressed: boolean } }) => {
		if (JSON.stringify(context) == JSON.stringify(payload.context)) pressed = payload.pressed;
	});

	function select(event: MouseEvent | KeyboardEvent) {
		if (event instanceof MouseEvent && event.ctrlKey) return;
		$openContextMenu = null;
		if (!slot) {
			$inspectedInstance = context;
			return;
		}
		if (slot.action.uuid == "opendeck.multiaction" || slot.action.uuid == "opendeck.toggleaction") {
			$inspectedParentAction = context;
		} else {
			$inspectedInstance = slot.context;
		}
	}

	function onfocus() {
		$openContextMenu = null;
		if (!slot) {
			$inspectedInstance = context;
			return;
		}
		if (slot.action.uuid != "opendeck.multiaction" && slot.action.uuid != "opendeck.toggleaction") {
			$inspectedInstance = slot.context;
		} else {
			$inspectedInstance = context;
		}
	}

	let contextMenuEl: HTMLDivElement;
	async function contextMenu(event: MouseEvent | KeyboardEvent) {
		event.preventDefault();
		if (!active || !context) return;
		const rect = canvas.getBoundingClientRect();
		let x = event instanceof MouseEvent && event.x ? event.x : rect.left;
		let y = event instanceof MouseEvent && event.y ? event.y : rect.bottom;
		$openContextMenu = { context, x, y };
		await tick();
		contextMenuEl?.querySelector("button")?.focus();
	}

	let showEditor = false;
	function edit() {
		$openContextMenu = null;
		showEditor = true;
	}

	function copy() {
		$openContextMenu = null;
		if (!context || !slot) return;
		copiedItem.set({ type: "instance", source: context });
	}

	export let handlePaste: ((item: CopiedItem, destination: Context) => Promise<void>) | undefined = undefined;
	async function paste() {
		$openContextMenu = null;
		if (!$copiedItem || !context || !handlePaste) return;
		await handlePaste($copiedItem, context);
		await tick();
		$inspectedInstance = `${context.device}.${context.profile}.${context.controller}.${context.position}.0`;
	}

	async function clear() {
		$openContextMenu = null;
		if (!slot) return;
		await invoke("remove_instance", { context: slot.context });
		showEditor = false;
		slot = null;
		inslot = slot;
		await tick();
		$inspectedInstance = context;
	}

	let showAlert: boolean = false;
	let showOk: boolean = false;
	let timeouts: number[] = [];
	listen("show_alert", ({ payload }: { payload: string }) => {
		if (!slot || payload != slot.context) return;
		timeouts.forEach(clearTimeout);
		showOk = false;
		showAlert = true;
		timeouts.push(setTimeout(() => (showAlert = false), 1.5e3));
	});
	listen("show_ok", ({ payload }: { payload: string }) => {
		if (!slot || payload != slot.context) return;
		timeouts.forEach(clearTimeout);
		showAlert = false;
		showOk = true;
		timeouts.push(setTimeout(() => (showOk = false), 1.5e3));
	});

	let canvas: HTMLCanvasElement;
	let lock = new CanvasLock();
	export let size = 144;
	// Canvas resolution defaults to a square `size`, but rectangular controllers (e.g. the Neo's infobar) can override this.
	export let width: number = size;
	export let height: number = size;
	$: (async () => {
		const sl = structuredClone(slot);
		if (m18Bottom) {
			const unlock = await lock.lock();
			try {
				const ctx = canvas?.getContext("2d");
				if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
			} finally {
				unlock();
			}
		} else if (!sl) {
			const unlock = await lock.lock();
			try {
				const ctx = canvas?.getContext("2d");
				if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
				if (active) await invoke("update_image", { context, image: null });
			} finally {
				unlock();
			}
		} else {
			const unlock = await lock.lock();
			try {
				let fallback = sl.action.states[sl.current_state]?.image ?? sl.action.icon;
				if (state) await renderImage(canvas, context, state, fallback, showOk, showAlert, true, active, pressed, $settings?.rotation);
			} finally {
				unlock();
			}
		}
	})();

	function clearAndRedraw() {
		canvas?.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
		slot = slot;
	}
	$: if ($settings?.rotation != undefined) {
		clearAndRedraw();
	}

	async function triggerVirtualPress() {
		if (!active || !context || !slot) return;
		await invoke("trigger_virtual_press", { context });
	}

	$: accessibleLabel = label + (slot ? ": " + slot.action.name + (state?.show && state?.text ? " - " + state.text : "") : "");
</script>

<div class="relative" style={`transform: scale(${(112 /* desired inner size */ / size) * scale});`}>
	<canvas
		bind:this={canvas}
		class="relative border-3 border-neutral-700 rounded-3xl outline-none outline-offset-2 outline-blue-500"
		style={`margin: ${-((size + 3 * 2 /* border */ - 132) /* desired outer size */ / 2)}px;`}
		class:outline-solid={active && ((slot && $inspectedInstance == slot.context) || (context && $inspectedInstance == context))}
		class:rounded-full!={context?.controller == "Encoder"}
		class:rounded-lg!={context?.controller == "Infobar"}
		class:bg-black={slot != null || m18Bottom}
		{width}
		{height}
		draggable={slot != null}
		{tabindex}
		{role}
		aria-label={accessibleLabel}
		on:dragstart
		on:dragover
		on:drop
		on:click|stopPropagation={select}
		on:dblclick|stopPropagation={triggerVirtualPress}
		on:keydown={(e) => {
			if (!active || !context) return;
			if (e.key == "Enter") select(e);
			else if (e.key == "F2") edit();
			else if ((e.ctrlKey || e.metaKey) && e.key == "c") copy();
			else if ((e.ctrlKey || e.metaKey) && e.key == "v") paste();
			else if (e.key == "Delete") clear();
			else if (e.key == "ContextMenu" || (e.shiftKey && e.key == "F10")) contextMenu(e);
		}}
		on:keyup|stopPropagation={(e) => {
			if (!active || !context) return;
			if (e.key == " ") select(e);
		}}
		on:focus={onfocus}
		on:contextmenu={contextMenu}
	/>
	{#if m18Bottom}
		<div class="m18-bottom-face absolute inset-0 flex items-center justify-center px-2 text-center text-xs text-neutral-300 pointer-events-none">
			<span class="line-clamp-3">{slot ? slot.action.name : label}</span>
		</div>
	{/if}
	{#if isTouchPoint && !slot}
		<div class="absolute left-1/4 top-1/2 w-1/2 border-t-4 border-neutral-700 pointer-events-none"></div>
	{/if}
</div>

{#if $openContextMenu && $openContextMenu?.context == context}
	<div
		bind:this={contextMenuEl}
		class="absolute w-32 font-semibold text-sm text-neutral-300 bg-neutral-700 border border-neutral-600 rounded-lg divide-y divide-neutral-600! z-10"
		style={`left: ${$openContextMenu.x}px; top: ${$openContextMenu.y}px;`}
	>
		{#if !slot}
			<button class="flex flex-row items-center w-full p-2 hover:bg-neutral-600 transition-colors rounded-lg cursor-pointer" on:click|stopPropagation={paste}>
				<Clipboard size="18" class="text-neutral-300" />
				<span class="ml-2">{$t("key.paste")}</span>
			</button>
		{:else}
			<button class="flex flex-row items-center w-full p-2 hover:bg-neutral-600 transition-colors rounded-t-lg cursor-pointer" on:click|stopPropagation={edit}>
				<Pencil size="18" class="text-neutral-300" />
				<span class="ml-2">{$t("key.edit")}</span>
			</button>
			<button class="flex flex-row items-center w-full p-2 hover:bg-neutral-600 transition-colors cursor-pointer" on:click|stopPropagation={copy}>
				<Copy size="18" class="text-neutral-300" />
				<span class="ml-2">{$t("key.copy")}</span>
			</button>
			<button class="flex flex-row items-center w-full p-2 hover:bg-neutral-600 transition-colors rounded-b-lg cursor-pointer" on:click|stopPropagation={clear}>
				<Trash size="18" class="text-red-400" />
				<span class="ml-2">{$t("key.delete")}</span>
			</button>
		{/if}
	</div>
{/if}

{#if slot && showEditor}
	<InstanceEditor bind:instance={slot} bind:showEditor />
{/if}

<style>
	.m18-bottom-face {
		border: 3px solid rgb(64 64 64);
		border-radius: 0.75rem;
		background: rgb(0 0 0 / 0.92);
	}
</style>
