<script lang="ts">
	import type { DeviceInfo } from "$lib/DeviceInfo";

	import AppWindow from "phosphor-svelte/lib/AppWindow";
	import ArrowRight from "phosphor-svelte/lib/ArrowRight";
	import Plus from "phosphor-svelte/lib/Plus";
	import Trash from "phosphor-svelte/lib/Trash";
	import Popup from "./Popup.svelte";

	import { pageLabel, type M18Page } from "$lib/pages";
	import { attempt } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount } from "svelte";

	export let show = false;
	export let device: DeviceInfo;
	export let pages: M18Page[] = [];

	const DEFAULT_RULE = "opendeck_default";

	type ApplicationProfiles = { [app: string]: { [device: string]: string } };
	let applicationProfiles: ApplicationProfiles = {};
	let applications: string[] = [];

	async function load() {
		[applications, applicationProfiles] = await Promise.all([invoke<string[]>("get_applications"), invoke<ApplicationProfiles>("get_application_profiles")]);
	}

	onMount(() => {
		let unlisten: (() => void) | undefined;
		let disposed = false;
		listen<string[]>("applications", ({ payload }) => (applications = payload)).then((stop) => (disposed ? stop() : (unlisten = stop)));
		return () => {
			disposed = true;
			unlisten?.();
		};
	});

	$: if (show) void load();

	$: rules = Object.entries(applicationProfiles)
		.filter(([app, devices]) => app !== DEFAULT_RULE && devices[device.id])
		.sort(([left], [right]) => left.localeCompare(right));
	$: fallback = applicationProfiles[DEFAULT_RULE]?.[device.id] ?? "";
	$: suggestions = applications.filter((app) => !applicationProfiles[app]?.[device.id]).sort((left, right) => left.localeCompare(right));

	async function save(next: ApplicationProfiles) {
		// Drop apps that no longer have a page on any device.
		const cleaned = Object.fromEntries(Object.entries(next).filter(([, devices]) => Object.values(devices).some(Boolean)));
		const saved = await attempt("Could not save auto-switch rules", () => invoke("set_application_profiles", { value: cleaned }));
		if (saved !== undefined) applicationProfiles = cleaned;
	}

	function setRule(app: string, profile: string) {
		const next = structuredClone(applicationProfiles);
		next[app] = { ...(next[app] ?? {}) };
		if (profile) next[app][device.id] = profile;
		else delete next[app][device.id];
		void save(next);
	}

	let newApp = "";
	let newPage = "";
	$: if (!newPage && pages.length) newPage = pages[0].profile;
	function addRule() {
		const app = newApp.trim();
		if (!app || !newPage) return;
		setRule(app, newPage);
		newApp = "";
	}

	function labelFor(profile: string) {
		const index = pages.findIndex((page) => page.profile === profile);
		return index >= 0 ? pageLabel(pages[index], index) : profile;
	}
</script>

<Popup bind:show title="Auto-switch pages" subtitle="Show a page automatically when an app comes to the front." size="md">
	<div class="space-y-5">
		<div class="card flex items-center gap-3 p-3">
			<div class="min-w-0 flex-1">
				<p class="text-[13px] font-medium text-ink">When no rule matches</p>
				<p class="hint">Used for every app that has no rule of its own.</p>
			</div>
			<select class="select w-44" value={fallback} on:change={(event) => setRule(DEFAULT_RULE, event.currentTarget.value)} aria-label="Page when no rule matches">
				<option value="">Stay on the current page</option>
				{#each pages as page, index}<option value={page.profile}>{pageLabel(page, index)}</option>{/each}
			</select>
		</div>

		<div>
			<p class="section-title mb-2">Rules</p>
			{#if rules.length === 0}
				<p class="rounded-lg border border-dashed border-line-strong px-4 py-6 text-center text-xs text-ink-faint">No rules yet. Add one below.</p>
			{:else}
				<ul class="divide-y divide-line overflow-hidden rounded-xl border border-line">
					{#each rules as [app, devices] (app)}
						<li class="flex items-center gap-3 bg-raised px-3 py-2">
							<AppWindow size="16" class="shrink-0 text-ink-faint" />
							<span class="min-w-0 flex-1 truncate text-[13px] text-ink">{app}</span>
							<ArrowRight size="14" class="shrink-0 text-ink-faint" />
							<select class="select w-44" value={devices[device.id]} on:change={(event) => setRule(app, event.currentTarget.value)} aria-label={`Page for ${app}`}>
								{#each pages as page, index}<option value={page.profile}>{pageLabel(page, index)}</option>{/each}
								{#if !pages.some((page) => page.profile === devices[device.id])}<option value={devices[device.id]}>{labelFor(devices[device.id])}</option>{/if}
							</select>
							<button class="btn btn-ghost btn-sm btn-icon" on:click={() => setRule(app, "")} aria-label={`Remove the rule for ${app}`}><Trash size="14" /></button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>

		<form class="flex items-end gap-2" on:submit|preventDefault={addRule}>
			<label class="min-w-0 flex-1">
				<span class="label mb-1.5">App</span>
				<input class="input" list="auto-switch-apps" bind:value={newApp} placeholder="Choose or type an app name" />
				<datalist id="auto-switch-apps">
					{#each suggestions as app}<option value={app}></option>{/each}
				</datalist>
			</label>
			<label class="w-44">
				<span class="label mb-1.5">Page</span>
				<select class="select" bind:value={newPage}>
					{#each pages as page, index}<option value={page.profile}>{pageLabel(page, index)}</option>{/each}
				</select>
			</label>
			<button class="btn btn-primary" type="submit" disabled={!newApp.trim()}><Plus size="14" weight="bold" /> Add rule</button>
		</form>
		<p class="hint">Apps you switch to while {device.name} is connected are suggested here. The name must match the app's name exactly.</p>
	</div>
</Popup>
