<script lang="ts">
	import Popup from "./Popup.svelte";
	import Tooltip from "./Tooltip.svelte";

	import { t } from "$lib/i18n";
	import { settings } from "$lib/settings";

	import { invoke } from "@tauri-apps/api/core";
	import { open } from "@tauri-apps/plugin-dialog";
	import { message } from "@tauri-apps/plugin-dialog";

	let showPopup = false;
	let importing = false;

	function filename(path: string): string {
		return path.split(/[\\/]/).pop() ?? path;
	}

	async function chooseVideo() {
		const selected = await open({ multiple: false, directory: false });
		if (typeof selected !== "string" || !$settings) return;

		importing = true;
		try {
			$settings.screensaver_video_path = await invoke<string>("import_screensaver_video", { path: selected });
			$settings.screensaver_mode = "video";
		} catch (error) {
			await message(String(error), { title: "M18 screensaver", buttons: { ok: $t("dialog.ok") } });
		} finally {
			importing = false;
		}
	}

	async function choosePhotos() {
		const selected = await open({ multiple: true, directory: false });
		const paths = Array.isArray(selected) ? selected : typeof selected === "string" ? [selected] : [];
		if (!$settings || paths.length === 0) return;

		importing = true;
		try {
			$settings.screensaver_photo_paths = await invoke<string[]>("import_screensaver_photos", { paths });
			$settings.screensaver_mode = "slideshow";
		} catch (error) {
			await message(String(error), { title: "M18 screensaver", buttons: { ok: $t("dialog.ok") } });
		} finally {
			importing = false;
		}
	}
</script>

<button
	class="px-3 py-1 text-sm text-neutral-300 bg-neutral-700 hover:bg-neutral-600 transition-colors border border-neutral-600 rounded-lg"
	on:click={() => (showPopup = true)}
>
	{$t("settings.screensaver")}
</button>

<svelte:window
	on:keydown={(event) => {
		if (event.key == "Escape") showPopup = false;
	}}
/>

<Popup show={showPopup} label={$t("settings.screensaver")}>
	<svelte:fragment slot="header">
		<button class="mr-2 my-1 float-right text-xl text-neutral-300" on:click={() => (showPopup = false)} aria-label={$t("settings.close")}>✕</button>
		<h2 class="m-2 font-semibold text-xl text-neutral-300">{$t("settings.screensaver")}</h2>
	</svelte:fragment>

	{#if $settings}
		<div class="flex flex-row items-center m-2 space-x-2">
			<label for="screensaver-enabled" class="text-neutral-400">{$t("settings.screensaver.enabled")}</label>
			<input type="checkbox" bind:checked={$settings.screensaver_enabled} id="screensaver-enabled" />
			<Tooltip>{$t("settings.screensaver.tooltip")}</Tooltip>
		</div>

		<div class="flex flex-row items-center m-2 space-x-2">
			<label for="screensaver-timeout" class="text-neutral-400">{$t("settings.screensaver.timeout")}</label>
			<input type="number" min="1" max="1440" bind:value={$settings.screensaver_timeout_minutes} class="w-16 px-1 text-neutral-300 border border-neutral-600 rounded-lg" id="screensaver-timeout" />
			<span class="text-neutral-400">{$t("settings.screensaver.minutes")}</span>
		</div>

		<div class="flex flex-row items-center m-2 space-x-2">
			<label for="screensaver-mode" class="text-neutral-400">{$t("settings.screensaver.mode")}</label>
			<div class="select-wrapper">
				<select bind:value={$settings.screensaver_mode} id="screensaver-mode" class="w-auto pr-10!">
					<option value="video">{$t("settings.screensaver.video")}</option>
					<option value="slideshow">{$t("settings.screensaver.slideshow")}</option>
				</select>
			</div>
		</div>

		{#if $settings.screensaver_mode == "video"}
			<div class="flex flex-row items-center m-2 space-x-2">
				<button class="px-2 py-1 text-sm text-neutral-300 bg-neutral-700 hover:bg-neutral-600 border border-neutral-600 rounded-lg disabled:opacity-50" on:click={chooseVideo} disabled={importing}>
					{$t("settings.screensaver.choose_video")}
				</button>
				<span class="max-w-xs truncate text-xs text-neutral-400" title={$settings.screensaver_video_path}>
					{$settings.screensaver_video_path ? $t("settings.screensaver.video_selected", { name: filename($settings.screensaver_video_path) }) : "No video selected"}
				</span>
			</div>
		{:else}
			<div class="flex flex-row items-center m-2 space-x-2">
				<button class="px-2 py-1 text-sm text-neutral-300 bg-neutral-700 hover:bg-neutral-600 border border-neutral-600 rounded-lg disabled:opacity-50" on:click={choosePhotos} disabled={importing}>
					{$t("settings.screensaver.choose_photos")}
				</button>
				<span class="text-xs text-neutral-400">{$t("settings.screensaver.photos_selected", { count: $settings.screensaver_photo_paths.length })}</span>
			</div>

			<div class="flex flex-row items-center m-2 space-x-2">
				<label for="screensaver-slide-seconds" class="text-neutral-400">{$t("settings.screensaver.slide_seconds")}</label>
				<input type="number" min="1" max="3600" bind:value={$settings.screensaver_slide_seconds} class="w-16 px-1 text-neutral-300 border border-neutral-600 rounded-lg" id="screensaver-slide-seconds" />
				<span class="text-neutral-400">{$t("settings.screensaver.seconds")}</span>
			</div>
		{/if}
	{/if}
</Popup>
