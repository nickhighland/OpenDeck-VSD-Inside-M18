<script lang="ts">
	import type { Action } from "$lib/Action";

	import CaretRight from "phosphor-svelte/lib/CaretRight";
	import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
	import X from "phosphor-svelte/lib/X";
	import GroupIcon from "./GroupIcon.svelte";

	import { COMING_SOON, groupColor, libraryGroup } from "$lib/actionLibrary";
	import { actionName, actionTooltip, filterActionCategories, isBuiltInGroup, shouldOpenActionCategory, type ActionLocalisations } from "$lib/actionSearch";
	import { categories, plugins, reloadCatalog } from "$lib/catalog";
	import { t } from "$lib/i18n";
	import { getWebserverUrl } from "$lib/ports";
	import { copiedItem } from "$lib/propertyInspector";
	import { localisations } from "$lib/settings";

	import { onMount } from "svelte";

	/** Called when an action is double-clicked or chosen with Enter. */
	export let onChoose: ((action: Action) => void) | undefined = undefined;

	export async function reload() {
		await reloadCatalog();
	}

	const DEFAULT_OPEN = ["Apps & Websites", "Keyboard & Text", "Media & Audio"];
	const STORAGE_KEY = "opendeck-m18.library";

	let query = "";
	let searchInput: HTMLInputElement;
	let showComingSoon = false;
	let openOverrides = new Map<string, boolean>();

	onMount(() => {
		reloadCatalog().catch((error) => console.error("Failed to load the action library", error));
		try {
			const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
			openOverrides = new Map(Object.entries(saved.open ?? {}));
			showComingSoon = saved.showComingSoon === true;
		} catch {
			// Preferences are a convenience; the defaults apply.
		}
	});

	function persist() {
		try {
			localStorage.setItem(STORAGE_KEY, JSON.stringify({ open: Object.fromEntries(openOverrides), showComingSoon }));
		} catch {
			// Ignore: storage may be unavailable.
		}
	}

	function toggleGroup(name: string, open: boolean) {
		openOverrides = new Map(openOverrides).set(name, !open);
		persist();
	}

	$: localisationsMap = ($localisations ?? {}) as ActionLocalisations;
	$: filtered = filterActionCategories($categories, query, localisationsMap, { includeComingSoon: showComingSoon });
	$: resultCount = filtered.reduce((count, [, category]) => count + category.actions.length, 0);
	$: comingSoonCount = ($categories[COMING_SOON]?.actions ?? []).filter((action) => action.visible_in_action_list !== false).length;

	function iconUrl(icon: string): string {
		return icon.startsWith("opendeck/") ? icon.replace("opendeck", "") : getWebserverUrl(icon);
	}

	function categoryIcon(name: string, icon: string | null | undefined, actions: Action[]): string | undefined {
		if (icon) return iconUrl(icon);
		const plugin = actions[0] && $plugins.find((candidate) => candidate.id == actions[0].plugin);
		return plugin?.icon ? getWebserverUrl(plugin.icon) : undefined;
	}

	function pluginName(name: string, actions: Action[]): string {
		const plugin = actions[0] && $plugins.find((candidate) => candidate.id == actions[0].plugin);
		return plugin?.name ?? name;
	}

	function handleListKeydown(event: KeyboardEvent) {
		const list = event.currentTarget as HTMLElement;
		const items = Array.from(list.querySelectorAll<HTMLElement>("[role='option']"));
		const currentIndex = items.indexOf(event.target as HTMLElement);
		if (currentIndex == -1) return;
		let newIndex = currentIndex;
		switch (event.key) {
			case "ArrowDown":
				newIndex = Math.min(currentIndex + 1, items.length - 1);
				break;
			case "ArrowUp":
				newIndex = Math.max(currentIndex - 1, 0);
				break;
			case "Home":
				newIndex = 0;
				break;
			case "End":
				newIndex = items.length - 1;
				break;
			default:
				return;
		}
		event.preventDefault();
		items[newIndex]?.focus();
	}
</script>

<svelte:window
	on:keydown={(event) => {
		// ⌘F / Ctrl+F jumps to the action search.
		if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
			event.preventDefault();
			searchInput?.focus();
			searchInput?.select();
		}
	}}
/>

<aside class="flex h-full w-[19.5rem] shrink-0 flex-col border-l border-line bg-panel">
	<div class="shrink-0 px-3 pt-3 pb-2">
		<div class="mb-2.5 flex items-baseline justify-between px-1">
			<h2 class="text-[13px] font-semibold text-ink">Actions</h2>
			<span class="text-[11px] text-ink-faint">Drag onto a key · double-click to add</span>
		</div>
		<div class="relative">
			<MagnifyingGlass size="14" class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-faint" />
			<input
				bind:this={searchInput}
				bind:value={query}
				class="input h-8 pr-8 pl-8"
				placeholder={$t("action_list.search_placeholder")}
				aria-label={$t("action_list.search_placeholder")}
				type="text"
				spellcheck="false"
				on:keydown={(event) => {
					if (event.key === "Escape") {
						query = "";
						searchInput.blur();
					}
				}}
			/>
			{#if query}
				<button class="btn btn-ghost btn-sm btn-icon absolute top-1/2 right-1 -translate-y-1/2" on:click={() => (query = "")} aria-label="Clear search"><X size="13" /></button>
			{:else}
				<span class="kbd pointer-events-none absolute top-1/2 right-1.5 h-5 min-w-5 -translate-y-1/2 border-b text-[10px] text-ink-faint">⌘F</span>
			{/if}
		</div>
	</div>

	<span id="action-list-hint" class="sr-only">{$t("action_list.hint")}</span>
	<div class="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
		{#if query.trim() && resultCount === 0}
			<div class="px-4 py-10 text-center" role="status">
				<MagnifyingGlass size="28" class="mx-auto mb-2 text-ink-faint" />
				<p class="text-[13px] text-ink-muted">No actions match “{query.trim()}”</p>
				<p class="hint mt-1">Try a shorter word, like “volume” or “page”.</p>
			</div>
		{/if}
		{#each filtered as [name, { icon, actions }] (name)}
			{@const open = shouldOpenActionCategory(name, query, openOverrides, DEFAULT_OPEN)}
			{@const builtIn = isBuiltInGroup(name)}
			{@const soon = name === COMING_SOON}
			<section class="mb-1">
				<button
					class="group flex w-full items-center gap-2.5 rounded-lg px-2 py-2 text-left transition-colors hover:bg-hover"
					on:click={() => toggleGroup(name, open)}
					aria-expanded={open}
					title={libraryGroup(name)?.blurb ?? ""}
				>
					<CaretRight size="11" weight="bold" class="shrink-0 text-ink-faint transition-transform duration-150 {open ? 'rotate-90' : ''}" />
					{#if builtIn || soon}
						<span class="flex size-6 shrink-0 items-center justify-center rounded-md text-white" style={`background: ${groupColor(name)}; box-shadow: 0 2px 8px -2px ${groupColor(name)};`}>
							<GroupIcon group={name} size="14" />
						</span>
					{:else if categoryIcon(name, icon, actions)}
						<img src={categoryIcon(name, icon, actions)} alt="" class="size-6 shrink-0 rounded-md" />
					{:else}
						<span class="flex size-6 shrink-0 items-center justify-center rounded-md bg-overlay text-ink-muted"><GroupIcon group={name} size="14" /></span>
					{/if}
					<span class="min-w-0 flex-1 truncate text-[12.5px] font-semibold" class:text-ink={!soon} class:text-ink-muted={soon}>{builtIn || soon ? name : pluginName(name, actions)}</span>
					{#if !builtIn && !soon}<span class="badge">Plugin</span>{/if}
					<span class="text-[11px] text-ink-faint tabular-nums">{actions.length}</span>
				</button>

				{#if open}
					{#if soon}
						<p class="mx-2 mb-1.5 rounded-md bg-hover px-2.5 py-1.5 text-[11px] text-ink-faint">Not working yet. Kept so imported VSD Craft keys stay in place.</p>
					{/if}
					<div class="flex flex-col gap-px pb-1" role="listbox" aria-label={name} aria-describedby="action-list-hint" tabindex="-1" on:keydown={handleListKeydown}>
						{#each actions as action (action.plugin + action.uuid)}
							{@const description = actionTooltip(action, localisationsMap)}
							<div
								class="action-row group/row flex cursor-grab items-center gap-2.5 rounded-lg px-2 py-1.5 transition-colors hover:bg-hover focus-visible:bg-hover active:cursor-grabbing"
								class:opacity-60={soon}
								draggable="true"
								title={description || actionName(action, localisationsMap)}
								role="option"
								aria-selected="false"
								tabindex="0"
								aria-label={actionName(action, localisationsMap)}
								on:dragstart={(event) => {
									if (!event.dataTransfer) return;
									event.dataTransfer.effectAllowed = "copy";
									event.dataTransfer.setData("action", JSON.stringify(action));
								}}
								on:dblclick={() => onChoose?.(action)}
								on:keydown={(event) => {
									if (event.key === "Enter") onChoose?.(action);
									else if ((event.ctrlKey || event.metaKey) && event.key == "c") copiedItem.set({ type: "action", action });
								}}
							>
								<img src={iconUrl(action.icon)} alt="" class="pointer-events-none size-8 shrink-0 rounded-lg shadow-[0_2px_6px_-2px_rgb(0_0_0/0.8)]" draggable="false" />
								<div class="min-w-0 flex-1">
									<p class="truncate text-[12.5px] font-medium text-ink">{actionName(action, localisationsMap)}</p>
									{#if description}<p class="truncate text-[11px] text-ink-faint">{description}</p>{/if}
								</div>
								{#if soon}<span class="badge shrink-0">Soon</span>{/if}
							</div>
						{/each}
					</div>
				{/if}
			</section>
		{/each}
	</div>

	{#if comingSoonCount > 0}
		<label class="flex shrink-0 cursor-pointer items-center gap-2.5 border-t border-line px-4 py-2.5 text-xs text-ink-muted">
			<input
				type="checkbox"
				class="switch"
				bind:checked={showComingSoon}
				on:change={() => {
					persist();
				}}
			/>
			Show {comingSoonCount} upcoming actions
		</label>
	{/if}
</aside>
