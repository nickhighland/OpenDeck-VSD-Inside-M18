<script lang="ts">
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";

	import { t } from "$lib/i18n";
	import { refreshSavedPages } from "$lib/keyImages";
	import { loadPageSet, pageSets, redrawEpoch, setPageSet, type M18PageSet } from "$lib/pages";
	import { settings } from "$lib/settings";
	import { profileManager } from "$lib/singletons";

	import { invoke } from "@tauri-apps/api/core";
	import { listen, type UnlistenFn } from "@tauri-apps/api/event";
	import { onMount } from "svelte";

	export let devices: { [id: string]: DeviceInfo } = {};
	export let value: string;
	export let selectedProfiles: { [id: string]: Profile } = {};

	let registered: string[] = [];

	async function refreshProfile(device: string) {
		try {
			selectedProfiles[device] = await invoke<Profile>("get_selected_profile", { device });
		} catch {
			// The device disconnected; the device list update removes it.
		}
	}

	$: {
		if (!value || !devices[value]) value = Object.keys(devices).sort()[0];
		// A device that disconnected must be registered again when it returns,
		// so its current profile is fetched instead of showing a stale copy.
		registered = registered.filter((id) => devices[id]);
		for (const id of Object.keys(devices)) {
			if (!registered.includes(id)) {
				registered.push(id);
				(async () => {
					const profile: Profile = await invoke("get_selected_profile", { device: id });
					selectedProfiles[id] = profile;
					await invoke("set_selected_profile", { device: id, id: profile.id });
					if (id.startsWith("18-")) {
						await loadPageSet(id);
						// Prepare every inactive page immediately. The page-turn barrier
						// can then use finished images instead of waiting for the webview
						// to mount the target page after the switch.
						refreshSavedPages(id);
					}
				})().catch((error) => console.warn(`Failed to load device ${id}`, error));
			}
		}
	}

	export function reloadProfiles() {
		registered = [];
	}

	// Rotation and app icon size change every key's image on every page.
	let drawnWith = "";
	$: {
		const current = `${$settings?.rotation}:${$settings?.app_icon_scale}`;
		if ($settings && drawnWith && current !== drawnWith) Object.keys(devices).forEach((device) => refreshSavedPages(device));
		if ($settings) drawnWith = current;
	}

	onMount(() => {
		let disposed = false;
		const unlisteners: UnlistenFn[] = [];
		(async () => {
			const subscriptions = await Promise.all([
				listen<{ [id: string]: DeviceInfo }>("devices", ({ payload }) => (devices = payload)),
				// Page switches from M18 keys, app-based switching, and plugins are
				// performed by the core; follow them here for every device.
				listen<{ device: string; pageSet: M18PageSet }>("m18_pages_changed", ({ payload }) => {
					// Adding, removing, renaming, or reordering pages changes what
					// other pages show (page numbers, new pages); a page turn alone
					// does not.
					setPageSet(payload.device, payload.pageSet);
					// A turn also makes the former page inactive; keep it warm for
					// the next turn. Page-set edits use the same path.
					refreshSavedPages(payload.device);
					void refreshProfile(payload.device);
				}),
				listen("rerender_images", async () => {
					await Promise.all(Object.keys(devices).map(refreshProfile));
					redrawEpoch.update((epoch) => epoch + 1);
					Object.keys(devices).forEach((device) => refreshSavedPages(device));
				}),
				// Only non-M18 devices still switch profiles through the editor.
				listen<{ device: string; profile: string }>("switch_profile", async ({ payload }) => {
					if (payload.device == value && $profileManager) {
						$profileManager.setProfile(payload.profile);
					} else {
						await invoke("set_selected_profile", { device: payload.device, id: payload.profile });
						await refreshProfile(payload.device);
					}
				}),
			]);
			if (disposed) subscriptions.forEach((unlisten) => unlisten());
			else unlisteners.push(...subscriptions);
			devices = await invoke("get_devices");
		})();
		return () => {
			disposed = true;
			unlisteners.forEach((unlisten) => unlisten());
		};
	});

	$: deviceCount = Object.keys(devices).length;
</script>

{#if deviceCount > 0}
	<div class="flex h-8 items-center gap-2 rounded-full border border-line bg-raised pr-3 pl-2.5">
		<span class="relative flex size-2">
			<span class="absolute inline-flex size-full animate-ping rounded-full bg-success opacity-40"></span>
			<span class="relative inline-flex size-2 rounded-full bg-success"></span>
		</span>
		{#if deviceCount > 1}
			<select bind:value class="bg-transparent pr-1 text-[12.5px] font-medium text-ink outline-none" aria-label={$t("device_selector.device")}>
				{#each Object.entries(devices).sort() as [id, device]}
					<option value={id}>{device.name}</option>
				{/each}
			</select>
		{:else}
			<span class="text-[12.5px] font-medium text-ink">{devices[value]?.name ?? ""}</span>
		{/if}
		<span class="text-[11.5px] text-ink-faint">Connected</span>
	</div>
{/if}
