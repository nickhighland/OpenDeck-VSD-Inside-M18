<script lang="ts">
	import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
	import CheckCircle from "phosphor-svelte/lib/CheckCircle";
	import Warning from "phosphor-svelte/lib/Warning";
	import Popup from "./Popup.svelte";

	import { ACCESSIBILITY_SETTINGS, inputPermission, inputPermissionDialog } from "$lib/permissions";
	import { PRODUCT_NAME } from "$lib/singletons";
	import { attempt } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { onDestroy, onMount } from "svelte";

	let allowed: boolean | null = null;
	let closedAt = 0;

	onMount(() => {
		let disposed = false;
		let unlisten: (() => void) | undefined;
		// A key that sends keystrokes was refused by macOS. After the dialog is
		// closed, further presses wait a while before opening it again.
		listen("input_permission_missing", () => {
			if (!$inputPermissionDialog && Date.now() - closedAt > 30_000) $inputPermissionDialog = true;
		}).then((stop) => (disposed ? stop() : (unlisten = stop)));
		return () => {
			disposed = true;
			unlisten?.();
		};
	});

	// While the dialog is open, notice as soon as the permission is granted.
	let timer: ReturnType<typeof setInterval> | undefined;
	async function refresh() {
		allowed = await inputPermission();
	}
	$: {
		clearInterval(timer);
		if ($inputPermissionDialog) {
			refresh();
			timer = setInterval(refresh, 1500);
		}
	}
	onDestroy(() => clearInterval(timer));

	function close() {
		$inputPermissionDialog = false;
		closedAt = Date.now();
	}
</script>

<Popup show={$inputPermissionDialog} title="Allow keystrokes" subtitle="Hotkeys, typed text, and media keys need macOS's Accessibility permission" size="sm" on:close={close}>
	<div class="flex flex-col gap-3 text-[13px] leading-relaxed text-ink-muted">
		{#if allowed}
			<div class="notice notice-info">
				<CheckCircle size="16" weight="fill" class="mt-px shrink-0 text-success" />
				<span>{PRODUCT_NAME} can send keystrokes. Press the key again.</span>
			</div>
		{:else}
			<div class="notice notice-warning">
				<Warning size="16" class="mt-px shrink-0" />
				<span>macOS is blocking this version of {PRODUCT_NAME} from sending keystrokes.</span>
			</div>
			<p>
				macOS ties the permission to each build of the app. After an update, {PRODUCT_NAME} can still look switched on in System Settings while the new version is blocked.
			</p>
			<ol class="list-decimal space-y-1 pl-5 marker:text-ink-faint">
				<li>Open <span class="text-ink">Privacy &amp; Security › Accessibility</span>.</li>
				<li>If {PRODUCT_NAME} is listed, select it and remove it with <span class="kbd">−</span>.</li>
				<li>Add it back with <span class="kbd">+</span> from Applications, and switch it on.</li>
			</ol>
			<p class="hint">This dialog notices the change by itself.</p>
		{/if}
	</div>
	<div slot="footer" class="flex justify-end gap-2">
		{#if allowed}
			<button class="btn btn-primary" on:click={close}>Done</button>
		{:else}
			<button class="btn btn-ghost" on:click={close}>Not now</button>
			<button class="btn btn-primary" on:click={() => attempt("Could not open System Settings", () => invoke("open_url", { url: ACCESSIBILITY_SETTINGS }))}>
				<ArrowSquareOut size="14" />
				Open Accessibility Settings
			</button>
		{/if}
	</div>
</Popup>
