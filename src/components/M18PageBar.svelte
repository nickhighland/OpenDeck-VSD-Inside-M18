<script lang="ts">
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";

	import ArrowLeft from "phosphor-svelte/lib/ArrowLeft";
	import ArrowRight from "phosphor-svelte/lib/ArrowRight";
	import Copy from "phosphor-svelte/lib/Copy";
	import DotsThree from "phosphor-svelte/lib/DotsThree";
	import Lightning from "phosphor-svelte/lib/Lightning";
	import PencilSimple from "phosphor-svelte/lib/PencilSimple";
	import Plus from "phosphor-svelte/lib/Plus";
	import Trash from "phosphor-svelte/lib/Trash";
	import AppSwitchDialog from "./AppSwitchDialog.svelte";

	import { inspectedInstance } from "$lib/propertyInspector";
	import { pageLabel, pageSets, setPageSet, type M18PageSet } from "$lib/pages";
	import { warmPage } from "$lib/keyImages";
	import { attempt, toast } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { ask } from "@tauri-apps/plugin-dialog";
	import { tick } from "svelte";

	export let device: DeviceInfo;
	export let profile: Profile;

	$: pageSet = $pageSets[device.id] ?? { pages: [], selected: 0 };
	$: selectedPage = pageSet.pages.findIndex((page) => page.profile == profile?.id);

	let busy = false;
	async function run<T>(title: string, action: () => Promise<T>) {
		if (busy) return;
		busy = true;
		try {
			return await attempt(title, action);
		} finally {
			busy = false;
		}
	}

	async function refreshProfile() {
		profile = await invoke("get_selected_profile", { device: device.id });
	}

	async function select(index: number) {
		if (index == selectedPage) return;
		$inspectedInstance = null;
		const target = pageSet.pages[index]?.profile;
		const started = performance.now();
		console.debug(`[M18 timing] page-tab ${index + 1} clicked`);
		if (target) void warmPage(device.id, target).catch((error) => console.debug("Failed to warm target M18 page", error));
		await run("Could not switch pages", async () => {
			await invoke("switch_m18_page_index", { device: device.id, index });
			console.debug(`[M18 timing] page-tab ${index + 1} backend returned in ${(performance.now() - started).toFixed(1)} ms`);
			await refreshProfile();
			console.debug(`[M18 timing] page-tab ${index + 1} editor refreshed in ${(performance.now() - started).toFixed(1)} ms`);
		});
	}

	async function update(command: string, args: Record<string, unknown>, failure: string) {
		const next = await run(failure, () => invoke<M18PageSet>(command, { device: device.id, ...args }));
		if (next) {
			setPageSet(device.id, next);
			await refreshProfile();
		}
		return next;
	}

	async function addPage() {
		$inspectedInstance = null;
		const next = await update("add_m18_page", {}, "Could not add a page");
		if (next) toast("success", `Added ${pageLabel(next.pages[next.pages.length - 1], next.pages.length - 1)}`);
	}

	// Renaming happens inline on the tab.
	let renaming: number | null = null;
	let renameValue = "";
	let renameInput: HTMLInputElement;
	async function startRename(index: number) {
		closeMenu();
		renaming = index;
		const name = pageSet.pages[index].name.trim();
		renameValue = /^\d+$/.test(name) ? "" : name;
		await tick();
		renameInput?.select();
	}
	async function commitRename() {
		if (renaming === null) return;
		const index = renaming;
		renaming = null;
		await update("rename_m18_page", { index, name: renameValue }, "Could not rename the page");
	}

	async function duplicatePage(index: number) {
		closeMenu();
		const next = await update("duplicate_m18_page", { index }, "Could not duplicate the page");
		if (next) toast("success", `Duplicated ${pageLabel(pageSet.pages[index] ?? next.pages[index], index)}`);
	}

	async function movePage(from: number, to: number) {
		closeMenu();
		if (to < 0 || to >= pageSet.pages.length || from === to) return;
		await update("move_m18_page", { from, to }, "Could not move the page");
	}

	async function deletePage(index: number) {
		closeMenu();
		const label = pageLabel(pageSet.pages[index], index);
		const confirmed = await ask(`Delete ${label} and every key on it? This cannot be undone.`, { title: `Delete ${label}?`, kind: "warning", okLabel: "Delete", cancelLabel: "Cancel" });
		if (!confirmed) return;
		$inspectedInstance = null;
		const next = await update("delete_m18_page", { index }, "Could not delete the page");
		if (next) toast("success", `Deleted ${label}`);
	}

	// Page menu (right-click or the ⋯ button on a tab).
	let menu: { index: number; x: number; y: number } | null = null;
	function openMenu(index: number, event: MouseEvent) {
		event.preventDefault();
		event.stopPropagation();
		const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
		menu = { index, x: event.type === "contextmenu" ? event.clientX : rect.left, y: event.type === "contextmenu" ? event.clientY : rect.bottom + 4 };
	}
	function closeMenu() {
		menu = null;
	}

	// Drag a tab to reorder pages. While dragging a key or an action, hovering a
	// tab switches to that page so it can be dropped there.
	let draggingPage: number | null = null;
	let dropTarget: number | null = null;
	let hoverTimer: ReturnType<typeof setTimeout> | undefined;
	function onTabDragOver(event: DragEvent, index: number) {
		const types = event.dataTransfer?.types ?? [];
		if (types.includes("m18-page")) {
			event.preventDefault();
			dropTarget = index;
		} else if ((types.includes("action") || types.includes("controller")) && index !== selectedPage && hoverTimer === undefined) {
			hoverTimer = setTimeout(() => {
				hoverTimer = undefined;
				void select(index);
			}, 450);
		}
	}
	function onTabDragLeave() {
		dropTarget = null;
		clearTimeout(hoverTimer);
		hoverTimer = undefined;
	}
	async function onTabDrop(event: DragEvent, index: number) {
		onTabDragLeave();
		if (draggingPage === null) return;
		event.preventDefault();
		const from = draggingPage;
		draggingPage = null;
		await movePage(from, index);
	}

	let showAutoSwitch = false;
</script>

<svelte:window on:click={closeMenu} on:blur={closeMenu} on:keydown={(event) => event.key === "Escape" && closeMenu()} />

<div class="flex min-w-0 items-center gap-2">
	<div class="flex min-w-0 items-center gap-1 overflow-x-auto py-1" role="tablist" aria-label="M18 pages">
		{#each pageSet.pages as page, index (page.id)}
			{@const active = index == selectedPage}
			{#if renaming === index}
				<input
					bind:this={renameInput}
					bind:value={renameValue}
					class="input h-8 w-36 min-h-0 py-0"
					placeholder={`Page ${index + 1}`}
					maxlength="40"
					aria-label="Page name"
					on:keydown={(event) => {
						if (event.key === "Enter") void commitRename();
						else if (event.key === "Escape") {
							event.stopPropagation();
							renaming = null;
						}
					}}
					on:blur={commitRename}
				/>
			{:else}
				<div
					class="group relative flex h-8 shrink-0 items-center rounded-lg border transition-colors {dropTarget === index
						? 'border-accent bg-accent-soft'
						: active
							? 'border-accent/35 bg-accent-soft'
							: 'border-transparent hover:bg-hover'}"
					role="presentation"
					draggable="true"
					on:dragstart={(event) => {
						draggingPage = index;
						event.dataTransfer?.setData("m18-page", String(index));
						if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
					}}
					on:dragend={() => (draggingPage = null)}
					on:dragover={(event) => onTabDragOver(event, index)}
					on:dragleave={onTabDragLeave}
					on:drop={(event) => onTabDrop(event, index)}
					on:contextmenu={(event) => openMenu(index, event)}
				>
					<button
						class="flex h-full items-center gap-2 pr-2 pl-2.5 text-[12.5px] font-medium"
						class:text-ink={active}
						class:text-ink-muted={!active}
						role="tab"
						aria-selected={active}
						on:click={() => select(index)}
						on:dblclick={() => startRename(index)}
						title="Double-click to rename · drag to reorder"
					>
						<span class="flex h-4.5 min-w-4.5 items-center justify-center rounded-[5px] px-1 text-[10.5px] font-bold tabular-nums" class:bg-accent-strong={active} class:text-white={active} class:bg-press={!active}>{index + 1}</span>
						{#if !/^Page \d+$/.test(pageLabel(page, index))}<span class="max-w-40 truncate">{pageLabel(page, index)}</span>{/if}
					</button>
					<button class="mr-1 flex size-5 items-center justify-center rounded-md text-ink-faint opacity-0 transition-opacity group-hover:opacity-100 hover:bg-press hover:text-ink focus-visible:opacity-100" class:opacity-100={menu?.index === index} on:click={(event) => openMenu(index, event)} aria-label={`Options for ${pageLabel(page, index)}`}>
						<DotsThree size="14" weight="bold" />
					</button>
				</div>
			{/if}
		{/each}
	</div>
	<button class="btn btn-ghost btn-sm btn-icon shrink-0" on:click={addPage} disabled={busy} title="Add a page" aria-label="Add a page">
		<Plus size="15" weight="bold" />
	</button>
	<div class="ml-auto shrink-0 pl-2">
		<button class="btn btn-ghost btn-sm" on:click={() => (showAutoSwitch = true)} title="Switch pages automatically when an app comes to the front">
			<Lightning size="14" weight="fill" class="text-warning" />
			Auto-switch
		</button>
	</div>
</div>

{#if menu}
	{@const index = menu.index}
	<div class="menu fixed z-50" style={`left: ${menu.x}px; top: ${menu.y}px;`} role="menu" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
		<button class="menu-item" role="menuitem" on:click={() => startRename(index)}><PencilSimple size="15" /> Rename…</button>
		<button class="menu-item" role="menuitem" on:click={() => duplicatePage(index)}><Copy size="15" /> Duplicate</button>
		<div class="menu-separator"></div>
		<button class="menu-item" role="menuitem" disabled={index === 0} on:click={() => movePage(index, index - 1)}><ArrowLeft size="15" /> Move left</button>
		<button class="menu-item" role="menuitem" disabled={index === pageSet.pages.length - 1} on:click={() => movePage(index, index + 1)}><ArrowRight size="15" /> Move right</button>
		<div class="menu-separator"></div>
		<button class="menu-item danger" role="menuitem" disabled={pageSet.pages.length <= 1} on:click={() => deletePage(index)}><Trash size="15" /> Delete page</button>
	</div>
{/if}

<AppSwitchDialog bind:show={showAutoSwitch} {device} pages={pageSet.pages} />
