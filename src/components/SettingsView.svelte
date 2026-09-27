<script lang="ts">
	import Archive from "phosphor-svelte/lib/Archive";
	import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
	import ClockClockwise from "phosphor-svelte/lib/ClockClockwise";
	import ClockCounterClockwise from "phosphor-svelte/lib/ClockCounterClockwise";
	import Code from "phosphor-svelte/lib/Code";
	import DownloadSimple from "phosphor-svelte/lib/DownloadSimple";
	import FolderOpen from "phosphor-svelte/lib/FolderOpen";
	import Gear from "phosphor-svelte/lib/Gear";
	import GearSix from "phosphor-svelte/lib/GearSix";
	import Info from "phosphor-svelte/lib/Info";
	import Monitor from "phosphor-svelte/lib/Monitor";
	import Scroll from "phosphor-svelte/lib/Scroll";
	import Popup from "./Popup.svelte";

	import { t } from "$lib/i18n";
	import { inputPermission, inputPermissionDialog } from "$lib/permissions";
	import { LANGUAGES, settings } from "$lib/settings";
	import { PRODUCT_NAME } from "$lib/singletons";
	import { attempt, toast } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";
	import { ask, message } from "@tauri-apps/plugin-dialog";

	export let showPopup = false;

	let buildInfo = "";
	(async () => {
		try {
			buildInfo = await invoke("get_build_info");
		} catch {}
	})();
	$: platform = buildInfo.split("</summary>")[0] ?? "";

	// macOS only: whether keystroke-sending keys are allowed to work.
	let keystrokes: boolean | null = null;
	$: if (showPopup && !$inputPermissionDialog) inputPermission().then((allowed) => (keystrokes = allowed));

	const sections = [
		{ id: "general", label: "General", icon: GearSix },
		{ id: "display", label: "M18 Display", icon: Monitor },
		{ id: "data", label: "Backup & Import", icon: Archive },
		{ id: "advanced", label: "Advanced", icon: Code },
		{ id: "about", label: "About", icon: Info },
	] as const;
	let section: (typeof sections)[number]["id"] = "general";

	let importingVsd = false;

	async function backupConfig() {
		const saved = await attempt("Backup failed", () => invoke<boolean>("backup_config_directory"));
		if (saved) toast("success", $t("settings.backup_config.success.title"), $t("settings.backup_config.success.prompt"));
	}

	async function restoreConfig() {
		const confirmed = await ask("Restoring replaces every page, key, and setting with the ones in the backup, then restarts the app. Back up the current configuration first if you might want it again.", {
			title: "Restore a backup?",
			kind: "warning",
			okLabel: "Choose backup…",
			cancelLabel: "Cancel",
		});
		if (!confirmed) return;
		await attempt("Restore failed", () => invoke("restore_config_directory"));
	}

	async function importVsdCraftProfile() {
		if (importingVsd) return;
		importingVsd = true;
		try {
			const summary: {
				device: string;
				profiles: string[];
				imported_actions: number;
				unsupported_actions: string[];
			} = await invoke("import_vsd_profile");
			const unsupported = summary.unsupported_actions.length
				? `\n\n${summary.unsupported_actions.length} action type(s) are not supported yet. Those keys keep their image and title but do nothing until they are implemented:\n${summary.unsupported_actions.join("\n")}`
				: "";
			await message(`Imported ${summary.imported_actions} keys on ${summary.profiles.length} page(s).${unsupported}\n\n${PRODUCT_NAME} will now restart to load them.`, {
				title: "VSD Craft import complete",
				buttons: { ok: $t("dialog.ok") },
			});
			await invoke("restart");
		} catch (error) {
			if (String(error).includes("No VSD Craft profile was selected")) return;
			await message(String(error), { title: "VSD Craft import", kind: "error", buttons: { ok: $t("dialog.ok") } });
		} finally {
			importingVsd = false;
		}
	}

	const rotations = [0, 90, 180, 270];
</script>

<button class="btn btn-ghost" on:click={() => (showPopup = true)} title={$t("settings.button")}>
	<Gear size="16" />
	<span>{$t("settings.button")}</span>
</button>

<Popup bind:show={showPopup} title={$t("settings.button")} size="lg">
	<div class="-mx-5 -my-4 flex min-h-[26rem]">
		<nav class="w-48 shrink-0 border-r border-line p-2" aria-label="Settings sections">
			{#each sections as item}
				<button
					class="flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-[13px] transition-colors"
					class:bg-accent-soft={section === item.id}
					class:text-ink={section === item.id}
					class:text-ink-muted={section !== item.id}
					class:hover:bg-hover={section !== item.id}
					aria-current={section === item.id}
					on:click={() => (section = item.id)}
				>
					<svelte:component this={item.icon} size="16" weight={section === item.id ? "fill" : "regular"} />
					{item.label}
				</button>
			{/each}
		</nav>

		<div class="min-w-0 flex-1 overflow-auto px-6 py-5">
			{#if $settings}
				{#if section === "general"}
					<div class="settings-list">
						<div class="setting">
							<div>
								<p class="setting-title">Language</p>
								<p class="hint">{$t("settings.language.tooltip", { PRODUCT_NAME })}</p>
							</div>
							<select class="select w-48" bind:value={$settings.language} aria-label="Language">
								{#each LANGUAGES as language}<option value={language.code}>{language.name}</option>{/each}
							</select>
						</div>
						<label class="setting">
							<div>
								<p class="setting-title">Start at login</p>
								<p class="hint">{$t("settings.autolaunch.tooltip.1", { PRODUCT_NAME })}</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.autolaunch} />
						</label>
						<label class="setting">
							<div>
								<p class="setting-title">Keep running when the window is closed</p>
								<p class="hint">The M18 keeps working from the menu bar. Recommended.</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.background} />
						</label>
						<label class="setting">
							<div>
								<p class="setting-title">Check for updates</p>
								<p class="hint">Look for new releases of {PRODUCT_NAME} on GitHub at startup.</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.updatecheck} />
						</label>
						{#if keystrokes !== null}
							<div class="setting">
								<div>
									<p class="setting-title">Keystrokes</p>
									<p class="hint">
										{keystrokes ? "Allowed by macOS: hotkeys, typed text, and media keys work." : "Blocked by macOS: hotkeys, typed text, and media keys do nothing until this is fixed."}
									</p>
								</div>
								{#if keystrokes}
									<span class="badge text-success">Allowed</span>
								{:else}
									<button
										class="btn btn-sm"
										on:click={() => {
											showPopup = false;
											$inputPermissionDialog = true;
										}}>Fix…</button
									>
								{/if}
							</div>
						{/if}
					</div>
				{:else if section === "display"}
					<div class="settings-list">
						<div class="setting flex-col items-stretch">
							<div class="flex items-center justify-between">
								<p class="setting-title">Brightness</p>
								<span class="text-xs text-ink-muted tabular-nums">{$settings.brightness}%</span>
							</div>
							<input type="range" min="0" max="100" class="range" style={`--range-fill: ${$settings.brightness}%`} bind:value={$settings.brightness} aria-label={$t("settings.brightness")} />
						</div>
						<div class="setting">
							<div>
								<p class="setting-title">Turn off the screen when the computer is idle</p>
								<p class="hint">The M18 stays on while you use the computer and turns off after this many minutes without keyboard, mouse, or trackpad input. It turns back on as soon as you use the computer. The first M18 press after that only wakes it. 0 keeps it on.</p>
							</div>
							<div class="flex shrink-0 items-center gap-2">
								<input type="number" min="0" max="1440" class="input w-20 text-right tabular-nums" bind:value={$settings.sleep_timeout_minutes} aria-label="Idle minutes before the M18 screen turns off" />
								<span class="text-xs text-ink-muted">min</span>
							</div>
						</div>
						<label class="setting">
							<div>
								<p class="setting-title">Turn off the screen while the computer is locked</p>
								<p class="hint">{$t("settings.sleep_when_computer_locked.tooltip")}</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.sleep_when_computer_locked} />
						</label>
						<div class="setting">
							<div>
								<p class="setting-title">Key image rotation</p>
								<p class="hint">Rotate every key image, for example when the M18 stands on its side.</p>
							</div>
							<div class="segmented shrink-0">
								{#each rotations as rotation}
									<button aria-pressed={$settings.rotation === rotation} on:click={() => $settings && ($settings.rotation = rotation)}>{rotation}°</button>
								{/each}
							</div>
						</div>
					</div>
				{:else if section === "data"}
					<div class="settings-list">
						<div class="setting">
							<div>
								<p class="setting-title">Import from VSD Craft</p>
								<p class="hint">Choose a VSD Craft profile's <code class="font-mono">manifest.json</code>. Its pages, keys, images, and titles are added as new pages; VSD Craft itself is not changed.</p>
							</div>
							<button class="btn btn-primary shrink-0" disabled={importingVsd} on:click={importVsdCraftProfile}>
								<DownloadSimple size="14" />
								{importingVsd ? "Importing…" : "Import…"}
							</button>
						</div>
						<div class="setting">
							<div>
								<p class="setting-title">Back up</p>
								<p class="hint">Save every page, key, image, and setting to a ZIP file.</p>
							</div>
							<button class="btn shrink-0" on:click={backupConfig}><ClockCounterClockwise size="14" /> Back up…</button>
						</div>
						<div class="setting">
							<div>
								<p class="setting-title">Restore</p>
								<p class="hint">Replace everything with a backup, then restart.</p>
							</div>
							<button class="btn shrink-0" on:click={restoreConfig}><ClockClockwise size="14" /> Restore…</button>
						</div>
						<div class="setting">
							<div>
								<p class="setting-title">Files</p>
								<p class="hint">Where configuration and logs are stored. Logs help when reporting a problem.</p>
							</div>
							<div class="flex shrink-0 gap-2">
								<button class="btn btn-sm" on:click={() => invoke("open_config_directory")}><FolderOpen size="13" /> {$t("settings.open_config")}</button>
								<button class="btn btn-sm" on:click={() => invoke("open_log_directory")}><Scroll size="13" /> {$t("settings.open_logs")}</button>
							</div>
						</div>
					</div>
				{:else if section === "advanced"}
					<div class="settings-list">
						<label class="setting">
							<div>
								<p class="setting-title">Developer mode</p>
								<p class="hint">{$t("settings.developer.tooltip")}</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.developer} />
						</label>
						<label class="setting">
							<div>
								<p class="setting-title">Send usage statistics</p>
								<p class="hint">Anonymous statistics (operating system, installed plugins, device model) are sent to the upstream OpenDeck project's analytics service when the app starts.</p>
							</div>
							<input type="checkbox" class="switch" bind:checked={$settings.statistics} />
						</label>
						{#if !platform.includes("windows")}
							<label class="setting">
								<div>
									<p class="setting-title">Separate Wine prefixes</p>
									<p class="hint">{$t("settings.separatewine.tooltip", { PRODUCT_NAME })}</p>
								</div>
								<input type="checkbox" class="switch" bind:checked={$settings.separatewine} />
							</label>
						{/if}
					</div>
				{:else}
					<div class="flex flex-col items-center gap-3 py-4 text-center">
						<img src="/app-icon.png" alt="" class="size-16 drop-shadow-[0_8px_20px_rgb(0_0_0/0.5)]" />
						<div>
							<p class="text-base font-semibold text-ink">{PRODUCT_NAME}</p>
							<p class="hint">Control software for the VSD Inside M18</p>
						</div>
						<div class="build-info w-full max-w-md rounded-lg border border-line bg-black/20 px-3 py-2 text-left text-xs text-ink-muted">
							<!-- eslint-disable-next-line svelte/no-at-html-tags -->
							{@html buildInfo}
						</div>
						<div class="flex gap-2">
							<button class="btn btn-sm" on:click={() => invoke("open_url", { url: "https://github.com/nickhighland/OpenDeck-VSD-Inside-M18" })}><ArrowSquareOut size="13" /> Project on GitHub</button>
							<button class="btn btn-sm" on:click={() => invoke("open_url", { url: "https://github.com/nekename/OpenDeck" })}><ArrowSquareOut size="13" /> Built on OpenDeck</button>
						</div>
						<p class="hint max-w-md">Licensed under the GNU GPL v3 or later. Built on OpenDeck by nekename and the M18 protocol work in opendeck-m18.</p>
					</div>
				{/if}
			{/if}
		</div>
	</div>
</Popup>

<style>
	.settings-list {
		display: flex;
		flex-direction: column;
	}
	.setting {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1.5rem;
		padding: 0.9rem 0;
		border-bottom: 1px solid var(--color-line);
	}
	.setting:first-child {
		padding-top: 0.2rem;
	}
	.setting:last-child {
		border-bottom: none;
	}
	.setting.flex-col {
		flex-direction: column;
		align-items: stretch;
		gap: 0.6rem;
	}
	.setting-title {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-ink);
		margin-bottom: 0.15rem;
	}
	.build-info :global(summary) {
		cursor: pointer;
		color: var(--color-ink);
	}
	.build-info :global(details[open] summary) {
		margin-bottom: 0.4rem;
	}
</style>
