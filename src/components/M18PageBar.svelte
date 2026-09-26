<script lang="ts">
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onMount } from "svelte";

	export let device: DeviceInfo;
	export let profile: Profile;

	let pageSet: { pages: { id: string; name: string; profile: string }[]; selected: number } = { pages: [], selected: 0 };

	async function refresh() {
		if (!device?.id) return;
		try {
			pageSet = await invoke("get_m18_pages", { device: device.id });
		} catch {}
	}

	onMount(async () => {
		await refresh();
		await listen("m18_pages_changed", async ({ payload }: { payload: { device: string; pageSet: typeof pageSet } }) => {
			if (payload.device == device.id) {
				pageSet = payload.pageSet;
				profile = await invoke("get_selected_profile", { device: device.id });
			}
		});
	});

	$: selectedPage = pageSet.pages.findIndex((page) => page.profile == profile?.id);

	async function select(index: number) {
		if (index == selectedPage) return;
		await invoke("switch_m18_page_index", { device: device.id, index });
		profile = await invoke("get_selected_profile", { device: device.id });
	}
</script>

<div class="flex flex-row items-center gap-1" aria-label="M18 pages">
	<span class="mr-1 text-xs text-neutral-500">Pages</span>
	{#each pageSet.pages as page, index}
		<button
			class="rounded border px-2 py-1 text-xs transition-colors"
			class:border-blue-400={index == selectedPage}
			class:bg-blue-900={index == selectedPage}
			class:border-neutral-600={index != selectedPage}
			class:hover:bg-neutral-700={index != selectedPage}
			on:click={() => select(index)}
			aria-label={`Select M18 page ${page.name}`}
		>
			{page.name}
		</button>
	{/each}
</div>
