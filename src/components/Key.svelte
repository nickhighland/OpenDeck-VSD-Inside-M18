<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { ActionState } from "$lib/ActionState";
	import type { Context } from "$lib/Context";
	import type { CopiedItem } from "$lib/propertyInspector";

	import ClipboardText from "phosphor-svelte/lib/ClipboardText";
	import Copy from "phosphor-svelte/lib/Copy";
	import PaintBrush from "phosphor-svelte/lib/PaintBrush";
	import Play from "phosphor-svelte/lib/Play";
	import Plus from "phosphor-svelte/lib/Plus";
	import Trash from "phosphor-svelte/lib/Trash";

	import { isFlowParent } from "$lib/actionLibrary";
	import { resolveState } from "$lib/appIcons";
	import { actionIndex } from "$lib/catalog";
	import { displayState } from "$lib/keyImages";
	import { pageSets, redrawEpoch } from "$lib/pages";
	import { contextKey, copiedItem, inspectedInstance, inspectedParentAction, inspectorTab, openContextMenu } from "$lib/propertyInspector";
	import { CanvasLock, getImage, renderImage } from "$lib/rendererHelper";
	import { settings } from "$lib/settings";
	import { attempt } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { listen, type UnlistenFn } from "@tauri-apps/api/event";
	import { onMount, tick } from "svelte";

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

	/** Interactive keys render to the device; previews (flow steps) do not. */
	export let active: boolean = true;
	export let isTouchPoint: boolean = false;
	// M18 bottom buttons are physical switches without an LCD. They remain
	// assignable in the editor, but image updates must never be sent for them.
	export let m18Bottom: boolean = false;
	/** On-screen size of the key in CSS pixels. */
	export let displaySize: number = 88;
	export let size = 144;
	// Canvas resolution defaults to a square `size`, but rectangular controllers (e.g. the Neo's infobar) can override this.
	export let width: number = size;
	export let height: number = size;
	export let handlePaste: ((item: CopiedItem, destination: Context) => Promise<void>) | undefined = undefined;

	let pressed = false;
	let dropTarget = false;
	let dragDepth = 0;

	$: libraryEntry = slot ? $actionIndex.get(slot.action.uuid) : undefined;
	// Built-in actions show their current library name; plugins keep their own.
	$: displayName = slot ? (libraryEntry && (slot.action.plugin === "" || slot.action.plugin === "opendeck") ? libraryEntry.action.name : slot.action.name) : "";

	let state: ActionState | undefined;
	$: state = slot ? displayState(slot, $pageSets[context?.device ?? ""], context?.profile) : undefined;

	let showAlert: boolean = false;
	let showOk: boolean = false;
	let timeouts: ReturnType<typeof setTimeout>[] = [];

	onMount(() => {
		let disposed = false;
		const unlisteners: UnlistenFn[] = [];
		Promise.all([
			listen<{ context: string; contents: ActionInstance | null }>("update_state", ({ payload }) => {
				if (payload.context == slot?.context) slot = payload.contents;
			}),
			listen<{ context: Context; pressed: boolean }>("key_moved", ({ payload }) => {
				if (context && contextKey(context) === contextKey(payload.context)) pressed = payload.pressed;
			}),
			listen<string>("show_alert", ({ payload }) => {
				if (!slot || payload != slot.context) return;
				timeouts.forEach(clearTimeout);
				showOk = false;
				showAlert = true;
				timeouts.push(setTimeout(() => (showAlert = false), 1.5e3));
			}),
			listen<string>("show_ok", ({ payload }) => {
				if (!slot || payload != slot.context) return;
				timeouts.forEach(clearTimeout);
				showAlert = false;
				showOk = true;
				timeouts.push(setTimeout(() => (showOk = false), 1.5e3));
			}),
		]).then((subscriptions) => {
			if (disposed) subscriptions.forEach((unlisten) => unlisten());
			else unlisteners.push(...subscriptions);
		});
		return () => {
			disposed = true;
			unlisteners.forEach((unlisten) => unlisten());
			timeouts.forEach(clearTimeout);
		};
	});

	$: selected = active && $inspectedInstance != null && ((slot != null && $inspectedInstance === slot.context) || (context != null && contextKey($inspectedInstance) === contextKey(context)));

	function select(event: MouseEvent | KeyboardEvent) {
		if (event instanceof MouseEvent && event.ctrlKey) return;
		$openContextMenu = null;
		if (!slot) {
			$inspectedInstance = context;
			return;
		}
		if (isFlowParent(slot.action.uuid)) {
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
		$inspectedInstance = isFlowParent(slot.action.uuid) ? context : slot.context;
	}

	let contextMenuEl: HTMLDivElement;
	let keyEl: HTMLElement;
	async function contextMenu(event: MouseEvent | KeyboardEvent) {
		event.preventDefault();
		if (!active || !context) return;
		const rect = keyEl.getBoundingClientRect();
		let x = event instanceof MouseEvent && event.clientX ? event.clientX : rect.left;
		let y = event instanceof MouseEvent && event.clientY ? event.clientY : rect.bottom;
		// Keep the menu inside the window.
		x = Math.min(x, window.innerWidth - 200);
		y = Math.min(y, window.innerHeight - 190);
		$openContextMenu = { context, x, y };
		await tick();
		contextMenuEl?.querySelector("button")?.focus();
	}

	function editAppearance() {
		$openContextMenu = null;
		if (!slot) return;
		$inspectedParentAction = null;
		$inspectedInstance = slot.context;
		$inspectorTab = "appearance";
	}

	function copy() {
		$openContextMenu = null;
		if (!context || !slot) return;
		copiedItem.set({ type: "instance", source: context });
	}

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
		const removed = await attempt("Could not clear the key", () => invoke("remove_instance", { context: slot!.context }));
		if (removed === undefined) return;
		slot = null;
		inslot = slot;
		await tick();
		$inspectedInstance = context;
	}

	let canvas: HTMLCanvasElement;
	let lock = new CanvasLock();
	$: (async () => {
		// Dependencies that should trigger a redraw of the key.
		void $redrawEpoch;
		void $settings?.app_icon_scale;
		const sl = structuredClone(slot);
		if (m18Bottom) return;
		if (!sl) {
			const unlock = await lock.lock();
			try {
				const ctx = canvas?.getContext("2d");
				if (ctx) ctx.clearRect(0, 0, canvas.width, canvas.height);
				if (active && context) await invoke("update_image", { context, image: null });
			} finally {
				unlock();
			}
		} else {
			const unlock = await lock.lock();
			try {
				let fallback = sl.action.states[sl.current_state]?.image ?? sl.action.icon;
				if (state) {
					// Launching keys without a chosen image show the app's icon.
					const resolved = await resolveState(sl, state);
					await renderImage(canvas, context, resolved.state, fallback, showOk, showAlert, true, active, pressed, $settings?.rotation);
				}
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
		$openContextMenu = null;
		if (!active || !context || !slot) return;
		await attempt("Could not run the key", () => invoke("trigger_virtual_press", { context }));
	}

	function acceptsDrop(event: DragEvent) {
		const types = event.dataTransfer?.types ?? [];
		return active && (types.includes("action") || types.includes("controller"));
	}

	$: accessibleLabel = label + (slot ? ": " + displayName + (state?.show && state?.text ? " - " + state.text : "") : ", empty");
	let bottomFace = "";
	let bottomFaceIsIcon = false;
	let bottomFaceRequest = 0;
	async function showBottomFace(sl: ActionInstance | null, shown: ActionState | undefined) {
		const request = ++bottomFaceRequest;
		if (!sl || !shown) {
			bottomFace = "";
			return;
		}
		const resolved = await resolveState(sl, shown);
		if (request !== bottomFaceRequest) return;
		bottomFaceIsIcon = resolved.appIcon;
		bottomFace = getImage(resolved.state.image, sl.action.states[sl.current_state]?.image ?? sl.action.icon);
	}
	$: if (m18Bottom) showBottomFace(slot, state);

	function handleKeydown(e: KeyboardEvent) {
		if (!active || !context) return;
		if (e.key == "Enter") select(e);
		else if (e.key == "F2") editAppearance();
		else if ((e.ctrlKey || e.metaKey) && e.key == "c") copy();
		else if ((e.ctrlKey || e.metaKey) && e.key == "v") paste();
		else if (e.key == "Delete" || e.key == "Backspace") clear();
		else if (e.key == "ContextMenu" || (e.shiftKey && e.key == "F10")) contextMenu(e);
	}
</script>

{#if m18Bottom}
	<div class="flex flex-col items-center gap-2" style={`width: ${displaySize}px;`}>
		<!-- The bottom buttons are grid cells like the LCD keys; the role is passed in. -->
		<!-- svelte-ignore a11y-no-noninteractive-tabindex -->
		<div
			bind:this={keyEl}
			class="bottom-button relative flex items-center justify-center overflow-hidden rounded-full"
			class:selected
			class:pressed
			class:drop-target={dropTarget}
			class:empty={!slot}
			style={`width: ${Math.round(displaySize * 0.62)}px; height: ${Math.round(displaySize * 0.62)}px;`}
			draggable={slot != null}
			{tabindex}
			{role}
			aria-label={accessibleLabel}
			on:dragstart
			on:dragover
			on:drop={(event) => {
				dragDepth = 0;
				dropTarget = false;
			}}
			on:drop
			on:dragenter={(event) => {
				if (!acceptsDrop(event)) return;
				dragDepth += 1;
				dropTarget = true;
			}}
			on:dragleave={() => {
				dragDepth = Math.max(0, dragDepth - 1);
				if (dragDepth === 0) dropTarget = false;
			}}
			on:click|stopPropagation={select}
			on:dblclick|stopPropagation={triggerVirtualPress}
			on:keydown={handleKeydown}
			on:keyup|stopPropagation={(e) => {
				if (active && context && e.key == " ") select(e);
			}}
			on:focus={onfocus}
			on:contextmenu={contextMenu}
		>
			{#if slot}
				<img src={bottomFace} alt="" class={`pointer-events-none size-full ${bottomFaceIsIcon ? "object-contain p-[9%]" : "object-cover"}`} draggable="false" />
			{:else}
				<Plus size="16" weight="bold" class="text-ink-faint" />
			{/if}
		</div>
		<span class="max-w-full truncate text-center text-[11px] leading-tight" class:text-ink-muted={slot} class:text-ink-faint={!slot} title={slot ? displayName : label}>{slot ? displayName : label}</span>
	</div>
{:else}
	<div
		bind:this={keyEl}
		class="key relative"
		class:selected
		class:pressed
		class:drop-target={dropTarget}
		class:empty={!slot}
		class:inactive={!active}
		class:touchpoint={isTouchPoint}
		class:rounded-full!={context?.controller == "Encoder"}
		style={`width: ${displaySize * (width / size)}px; height: ${displaySize * (height / size)}px;`}
	>
		<canvas
			bind:this={canvas}
			class="absolute inset-0 size-full outline-none"
			{width}
			{height}
			draggable={slot != null}
			{tabindex}
			{role}
			aria-label={accessibleLabel}
			title={slot ? displayName : undefined}
			on:dragstart
			on:dragover
			on:drop={() => {
				dragDepth = 0;
				dropTarget = false;
			}}
			on:drop
			on:dragenter={(event) => {
				if (!acceptsDrop(event)) return;
				dragDepth += 1;
				dropTarget = true;
			}}
			on:dragleave={() => {
				dragDepth = Math.max(0, dragDepth - 1);
				if (dragDepth === 0) dropTarget = false;
			}}
			on:click|stopPropagation={select}
			on:dblclick|stopPropagation={triggerVirtualPress}
			on:keydown={handleKeydown}
			on:keyup|stopPropagation={(e) => {
				if (active && context && e.key == " ") select(e);
			}}
			on:focus={onfocus}
			on:contextmenu={contextMenu}
		/>
		{#if !slot && active}
			<div class="empty-glyph pointer-events-none absolute inset-0 flex items-center justify-center">
				<Plus size={Math.round(displaySize * 0.2)} weight="bold" />
			</div>
		{/if}
	</div>
{/if}

{#if $openContextMenu && context && contextKey($openContextMenu.context) === contextKey(context)}
	<div bind:this={contextMenuEl} class="menu fixed z-50" style={`left: ${$openContextMenu.x}px; top: ${$openContextMenu.y}px;`} role="menu">
		{#if slot}
			<button class="menu-item" role="menuitem" on:click|stopPropagation={editAppearance}>
				<PaintBrush size="15" /> Edit appearance
			</button>
			<button class="menu-item" role="menuitem" on:click|stopPropagation={triggerVirtualPress}>
				<Play size="15" /> Test press
			</button>
			<div class="menu-separator"></div>
			<button class="menu-item" role="menuitem" on:click|stopPropagation={copy}>
				<Copy size="15" /> Copy <span class="ml-auto text-[11px] opacity-60">⌘C</span>
			</button>
		{/if}
		<button class="menu-item" role="menuitem" disabled={!$copiedItem || !handlePaste} on:click|stopPropagation={paste}>
			<ClipboardText size="15" /> Paste <span class="ml-auto text-[11px] opacity-60">⌘V</span>
		</button>
		{#if slot}
			<div class="menu-separator"></div>
			<button class="menu-item danger" role="menuitem" on:click|stopPropagation={clear}>
				<Trash size="15" /> Clear key <span class="ml-auto text-[11px] opacity-60">⌫</span>
			</button>
		{/if}
	</div>
{/if}

<style>
	.key {
		border-radius: var(--radius-key);
		background: #030405;
		box-shadow:
			inset 0 0 0 1px rgb(255 255 255 / 0.06),
			0 1px 0 rgb(255 255 255 / 0.05),
			0 6px 14px -8px rgb(0 0 0 / 0.9);
		overflow: hidden;
		transition:
			transform 120ms ease,
			box-shadow 140ms ease;
	}
	.key canvas {
		border-radius: inherit;
	}
	.key:not(.inactive):hover {
		transform: translateY(-1px);
		box-shadow:
			inset 0 0 0 1px rgb(255 255 255 / 0.12),
			0 10px 20px -10px rgb(0 0 0 / 0.95);
	}
	.key.empty {
		background: rgb(255 255 255 / 0.015);
		box-shadow: inset 0 0 0 1.5px rgb(255 255 255 / 0.07);
	}
	.key.empty:not(.inactive):hover {
		box-shadow: inset 0 0 0 1.5px rgb(255 255 255 / 0.16);
	}
	.empty-glyph {
		color: rgb(255 255 255 / 0.1);
		transition: color 120ms;
	}
	.key.empty:hover .empty-glyph {
		color: rgb(255 255 255 / 0.35);
	}
	.key.selected,
	.bottom-button.selected {
		box-shadow:
			0 0 0 2px var(--color-accent),
			0 0 22px -4px rgb(139 123 255 / 0.7);
	}
	.key.drop-target,
	.bottom-button.drop-target {
		transform: scale(1.04);
		box-shadow:
			0 0 0 2px var(--color-accent),
			0 0 28px rgb(139 123 255 / 0.55);
	}
	.key.drop-target .empty-glyph {
		color: var(--color-accent);
	}
	.key.pressed,
	.bottom-button.pressed {
		transform: scale(0.93);
		transition-duration: 60ms;
	}
	.key.touchpoint.empty::after {
		content: "";
		position: absolute;
		left: 25%;
		top: 50%;
		width: 50%;
		border-top: 4px solid rgb(255 255 255 / 0.12);
	}

	.bottom-button {
		background: radial-gradient(circle at 35% 30%, #2c313b, #15181e 70%);
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.12),
			inset 0 -2px 4px rgb(0 0 0 / 0.5),
			0 4px 10px -4px rgb(0 0 0 / 0.9);
		transition:
			transform 120ms ease,
			box-shadow 140ms ease;
	}
	.bottom-button:hover {
		transform: translateY(-1px);
	}
	.bottom-button.empty {
		background: radial-gradient(circle at 35% 30%, #22262e, #121418 70%);
	}
	.bottom-button:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 3px;
	}
</style>
