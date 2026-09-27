<script lang="ts">
	import type { Action } from "$lib/Action";
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";

	import ActionList from "../components/ActionList.svelte";
	import DeviceSelector from "../components/DeviceSelector.svelte";
	import DeviceView from "../components/DeviceView.svelte";
	import M18PageBar from "../components/M18PageBar.svelte";
	import NoDevicesDetected from "../components/NoDevicesDetected.svelte";
	import ParentActionView from "../components/ParentActionView.svelte";
	import PluginManager from "../components/PluginManager.svelte";
	import ProfileManager from "../components/ProfileManager.svelte";
	import PropertyInspectorView from "../components/PropertyInspectorView.svelte";
	import SettingsView from "../components/SettingsView.svelte";
	import InputPermissionDialog from "../components/InputPermissionDialog.svelte";
	import Toasts from "../components/Toasts.svelte";

	import { initPortBase } from "$lib/ports";
	import { inspectedInstance, inspectedParentAction } from "$lib/propertyInspector";
	import { actionList, deviceSelector, PRODUCT_NAME, profileManager } from "$lib/singletons";
	import { attempt, toast } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";

	let devices: { [id: string]: DeviceInfo } = {};
	let selectedDevice: string;
	let selectedProfiles: { [id: string]: Profile } = {};
	let parentView: ParentActionView;

	initPortBase();

	$: hasDevices = Object.keys(devices).length > 0 && selectedProfiles;
	$: isM18 = selectedDevice?.startsWith("18-");

	/** Place an action chosen in the library: into the open flow, the selected empty key, or the first free key. */
	async function placeAction(action: Action) {
		if ($inspectedParentAction && parentView) {
			await parentView.addAction(action);
			return;
		}
		const device = devices[selectedDevice];
		const profile = selectedProfiles[selectedDevice];
		if (!device || !profile) return;
		const selected = $inspectedInstance;
		let position: number | undefined;
		if (selected && typeof selected === "object" && selected.device === device.id && selected.profile === profile.id && selected.controller === "Keypad" && !profile.keys[selected.position]) {
			position = selected.position;
		} else {
			const slots = device.id.startsWith("18-") ? 18 : profile.keys.length;
			position = Array.from({ length: slots }, (_, index) => index).find((index) => !profile.keys[index]);
		}
		if (position === undefined) {
			toast("info", "This page is full", "Clear a key or add a page first.");
			return;
		}
		const context = { device: device.id, profile: profile.id, controller: "Keypad", position };
		const created = await attempt("Could not add the action", () => invoke<ActionInstance | null>("create_instance", { context, action }));
		if (!created) return;
		profile.keys[position] = created;
		selectedProfiles = selectedProfiles;
		$inspectedInstance = created.context;
	}
</script>

<svelte:window on:dragover={(event) => event.preventDefault()} on:drop={(event) => event.preventDefault()} />

<div class="flex h-screen flex-col bg-canvas text-ink">
	<header class="flex h-12 shrink-0 items-center gap-4 border-b border-line bg-panel px-4">
		<div class="flex items-center gap-2.5">
			<img src="/app-icon.png" alt="" class="size-7 drop-shadow-[0_4px_10px_rgb(0_0_0/0.45)]" />
			<span class="text-[13px] font-semibold tracking-tight">{PRODUCT_NAME}</span>
		</div>
		<DeviceSelector bind:devices bind:value={selectedDevice} bind:selectedProfiles bind:this={$deviceSelector} />
		<div class="ml-auto flex items-center gap-1">
			<PluginManager />
			<SettingsView />
		</div>
	</header>

	<div class="flex min-h-0 flex-1">
		<main class="flex min-w-0 flex-1 flex-col">
			{#if hasDevices}
				<div class="flex min-h-0 flex-1 flex-col">
					{#if !$inspectedParentAction}
						<div class="flex h-12 shrink-0 items-center border-b border-line px-4">
							{#key selectedDevice}
								{#if selectedDevice && devices[selectedDevice] && selectedProfiles[selectedDevice]}
									{#if isM18}
										<M18PageBar device={devices[selectedDevice]} bind:profile={selectedProfiles[selectedDevice]} />
									{:else}
										<ProfileManager bind:device={devices[selectedDevice]} bind:profile={selectedProfiles[selectedDevice]} bind:this={$profileManager} />
									{/if}
								{/if}
							{/key}
						</div>
					{/if}

					{#if $inspectedParentAction && selectedProfiles[selectedDevice]}
						<ParentActionView bind:this={parentView} bind:profile={selectedProfiles[selectedDevice]} />
					{/if}

					{#each Object.entries(devices) as [id, device] (id)}
						{#if device && selectedProfiles[id]}
							<DeviceView bind:device bind:profile={selectedProfiles[id]} bind:selectedDevice />
						{/if}
					{/each}
				</div>

				{#if selectedProfiles[selectedDevice]}
					<PropertyInspectorView bind:device={devices[selectedDevice]} bind:profile={selectedProfiles[selectedDevice]} />
				{/if}
			{:else}
				<NoDevicesDetected />
			{/if}
		</main>

		<ActionList bind:this={$actionList} onChoose={placeAction} />
	</div>
</div>

<InputPermissionDialog />
<Toasts />
