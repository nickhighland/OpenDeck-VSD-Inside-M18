<script lang="ts">
	import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
	import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
	import CloudArrowDown from "phosphor-svelte/lib/CloudArrowDown";
	import FileArrowUp from "phosphor-svelte/lib/FileArrowUp";
	import Gear from "phosphor-svelte/lib/Gear";
	import MagnifyingGlass from "phosphor-svelte/lib/MagnifyingGlass";
	import Trash from "phosphor-svelte/lib/Trash";
	import WarningCircle from "phosphor-svelte/lib/WarningCircle";
	import ListedPlugin from "./ListedPlugin.svelte";
	import PluginDetails from "./PluginDetails.svelte";
	import Popup from "./Popup.svelte";
	import Tooltip from "./Tooltip.svelte";

	import { t } from "$lib/i18n";
	import { getWebserverUrl } from "$lib/ports";
	import { localisations, settings } from "$lib/settings";
	import { reloadCatalog } from "$lib/catalog";
	import { deviceSelector, PRODUCT_NAME } from "$lib/singletons";
	import PuzzlePiece from "phosphor-svelte/lib/PuzzlePiece";

	import { invoke } from "@tauri-apps/api/core";
	import { onDestroy } from "svelte";
	import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
	import { ask, message, open } from "@tauri-apps/plugin-dialog";

	// @ts-expect-error
	const fetch = window.fetchNative ?? window.fetch;

	let showPopup: boolean;
	let pollTimer: ReturnType<typeof setInterval> | undefined;
	$: {
		clearInterval(pollTimer);
		pollTimer = showPopup ? setInterval(async () => (installed = await invoke("list_plugins")), 1e3) : undefined;
	}
	onDestroy(() => clearInterval(pollTimer));

	async function installPlugin(name: string, url: string | null, file: string | null, fallback_id: string | null) {
		if (
			!file &&
			!(await ask($t("plugin_manager.install.prompt"), {
				title: $t("plugin_manager.install.title", { name }),
				okLabel: $t("dialog.yes"),
				cancelLabel: $t("dialog.no"),
			}))
		)
			return;
		try {
			await invoke("install_plugin", { url, file, fallback_id });
			message($t("plugin_manager.install.success", { name }), {
				title: $t("plugin_manager.install.success.title", { name }),
				buttons: { ok: $t("dialog.ok") },
			});
			await reloadCatalog();
			installed = await invoke("list_plugins");
		} catch (error: any) {
			message(error, { title: $t("plugin_manager.install.error", { name }), buttons: { ok: $t("dialog.ok") } });
		}
	}

	let choices: any[] | undefined;
	let choice: number;
	let finishChoice = (_: unknown) => {};
	let cancelChoice = () => {};
	async function chooseAsset(assets: any[]): Promise<any> {
		choices = assets;
		try {
			await new Promise((resolve, reject) => {
				finishChoice = resolve;
				cancelChoice = reject;
			});
		} catch (e) {
			throw e;
		} finally {
			choices = undefined;
			finishChoice = (_: unknown) => {};
			cancelChoice = () => {};
		}
		return assets[choice];
	}

	let openDetailsView: string | null = null;
	type GitHubPlugin = {
		name: string;
		author: string;
		repository: string;
		download_url: string | undefined;
	};
	async function installPluginGitHub(id: string, plugin: GitHubPlugin) {
		if (plugin.download_url) {
			await installPlugin(plugin.name, plugin.download_url, null, id);
			return;
		}

		let endpoint = new URL(plugin.repository);
		endpoint.hostname = "api." + endpoint.hostname;
		endpoint.pathname = "/repos" + endpoint.pathname + "/releases";

		let res;
		try {
			res = await (await fetch(endpoint)).json();
		} catch (error: any) {
			message(error, { title: $t("plugin_manager.install.error", { name: plugin.name }), buttons: { ok: $t("dialog.ok") } });
			return;
		}

		let release = res[0];
		if (release.prerelease && res.find((r: any) => !r.prerelease)) release = res.find((r: any) => !r.prerelease);

		let assets = [];
		for (const asset of release.assets) {
			if (asset.name.toLowerCase().endsWith(".streamdeckplugin") || asset.name.toLowerCase().endsWith(".zip")) {
				assets.push(asset);
			}
		}
		let selected;
		if (assets.length == 1) selected = assets[0];
		else {
			try {
				selected = await chooseAsset(assets);
			} catch {
				return;
			}
		}

		await installPlugin(plugin.name, selected.browser_download_url, null, id);
	}

	async function installPluginFile() {
		const path = await open({ multiple: false, directory: false });
		if (!path) return;
		await installPlugin(path.split(/[\/\\]/).at(-1) ?? path, null, path, null);
	}

	async function removePlugin(plugin: any) {
		if (
			!(await ask($t("plugin_manager.remove.prompt", { name: plugin.name }), {
				title: $t("plugin_manager.remove.title", { name: plugin.name }),
				okLabel: $t("dialog.yes"),
				cancelLabel: $t("dialog.no"),
			}))
		)
			return;
		try {
			await invoke("remove_plugin", { id: plugin.id });
			message($t("plugin_manager.remove.success", { name: plugin.name }), {
				title: $t("plugin_manager.remove.success.title", { name: plugin.name }),
				buttons: { ok: $t("dialog.ok") },
			});
			await reloadCatalog();
			$deviceSelector?.reloadProfiles();
			installed = await invoke("list_plugins");
		} catch (error: any) {
			message(error, { title: $t("plugin_manager.remove.error", { name: plugin.name }), buttons: { ok: $t("dialog.ok") } });
		}
	}

	async function isUpdateAvailable(plugin: any): Promise<string | false> {
		const id = plugin.id.endsWith(".sdPlugin") ? plugin.id.slice(0, -9) : plugin.id;
		const cataloguePlugin = plugins[id];
		if (!cataloguePlugin || cataloguePlugin.download_url) return false;

		try {
			const endpoint = new URL(cataloguePlugin.repository);
			endpoint.hostname = "api." + endpoint.hostname;
			endpoint.pathname = "/repos" + endpoint.pathname + "/releases/latest";

			const res = await fetch(endpoint);
			if (!res.ok) return false;
			const release = await res.json();

			const normalizeVersion = (v: string) => v.replace(/^v/, "").replace(/^(\d+\.\d+\.\d+)\.\d+$/, "$1");
			if (normalizeVersion(release.tag_name) != normalizeVersion(plugin.version)) {
				return release.tag_name.replace(/^v/, "");
			} else {
				return false;
			}
		} catch (error) {
			console.warn("Failed to check for plugin update:", error);
			return false;
		}
	}

	let installed: any[] = [];
	(async () => (installed = await invoke("list_plugins")))();

	let plugins: { [id: string]: GitHubPlugin };
	let catalogueError = false;
	(async () => {
		try {
			plugins = await (await fetch("https://openactionapi.github.io/plugins/catalogue.json")).json();
		} catch {
			catalogueError = true;
		}
	})();

	let availableUpdates: { [id: string]: string | false } = {};
	let checkedPlugins = new Set<string>();
	$: if (showPopup) {
		for (const plugin of installed) {
			if (!checkedPlugins.has(plugin.id)) {
				checkedPlugins.add(plugin.id);
				isUpdateAvailable(plugin).then((version) => (availableUpdates = { ...availableUpdates, [plugin.id]: version }));
			}
		}
	}

	let pluginVersions: { [id: string]: string } = {};
	$: for (const plugin of installed) {
		if (pluginVersions[plugin.id] != plugin.version) {
			checkedPlugins.delete(plugin.id);
			delete availableUpdates[plugin.id];
			availableUpdates = availableUpdates;
			pluginVersions[plugin.id] = plugin.version;
		}
	}

	let query: string = "";

	onOpenUrl((urls: string[]) => {
		if (!urls[0].includes("installPlugin/")) return;
		let id = urls[0].split("installPlugin/")[1];
		if (!plugins[id]) return;
		installPluginGitHub(id, plugins[id]);
	});
</script>

<button class="btn btn-ghost" on:click={() => (showPopup = true)} title={$t("plugin_manager.title")}>
	<PuzzlePiece size="16" />
	<span>{$t("plugin_manager.button")}</span>
</button>

<svelte:window
	on:keydown={(event) => {
		if (event.key == "Escape") {
			if (choices) cancelChoice();
			else if (openDetailsView) openDetailsView = null;
		}
	}}
/>

<Popup bind:show={showPopup} title={$t("plugin_manager.title")} subtitle="Plugins add more actions to the library. The M18 itself is built in and needs no plugin." size="xl">
	<svelte:fragment slot="actions">
		<button class="btn btn-sm" on:click={installPluginFile}>
			<FileArrowUp size="14" />
			{$t("plugin_manager.install_from_file")}
		</button>
	</svelte:fragment>

	<h3 class="section-title mb-3">{$t("plugin_manager.installed")}</h3>
	<div class="grid grid-cols-2 gap-2 lg:grid-cols-3">
		<!-- prettier-ignore -->
		{#each installed.sort((a, b) =>
			(a.builtin && !b.builtin) ? -1 :
			(b.builtin && !a.builtin) ? 1 :
			(a.has_settings_interface && !b.has_settings_interface) ? -1 :
			(b.has_settings_interface && !a.has_settings_interface) ? 1 :
			a.id.localeCompare(b.id)
		) as plugin}
			<ListedPlugin
				icon={getWebserverUrl(plugin.icon)}
				name={($localisations && $localisations[plugin.id] && $localisations[plugin.id].Name) ? $localisations[plugin.id].Name : plugin.name}
				subtitle={plugin.version}
				disconnected={!plugin.registered}
				action={() => {
					if ($settings?.developer) invoke("reload_plugin", { id: plugin.id });
					else removePlugin(plugin);
				}}
				actionLabel={$settings?.developer ? $t("plugin_manager.reload") : $t("plugin_manager.remove")}
				secondaryAction={!plugin.registered ? () => invoke("open_log_directory") : plugin.has_settings_interface ? () => invoke("show_settings_interface", { plugin: plugin.id }) : undefined}
				secondaryActionLabel={!plugin.registered ? $t("plugin_manager.view_logs") : $t("plugin_manager.plugin_settings")}
			>
				<svelte:fragment slot="subtitle">
					<span class="tabular-nums">{plugin.version}</span>
					{#if plugin.builtin}<span class="badge ml-1">Built in</span>{/if}
					{#if !plugin.registered}<span class="badge ml-1 text-warning">Not running</span>{/if}
					{#if availableUpdates[plugin.id]}
						<button
							class="badge ml-1 text-success"
							on:click={() => openDetailsView = plugin.id.endsWith(".sdPlugin") ? plugin.id.slice(0, -9) : plugin.id}
						>
							{$t("plugin_manager.available")} {availableUpdates[plugin.id]}
						</button>
					{/if}
				</svelte:fragment>

				<svelte:fragment slot="secondary">
					{#if !plugin.registered}
						<WarningCircle size="18" class="text-warning" />
					{:else if plugin.has_settings_interface}
						<Gear size="18" class="text-ink-muted" />
					{/if}
				</svelte:fragment>

				{#if $settings?.developer}
					<ArrowClockwise size="18" class="text-ink-muted" />
				{:else if !plugin.builtin}
					<Trash size="18" class="text-ink-muted" />
				{/if}
			</ListedPlugin>
		{/each}
	</div>

	<div class="mt-7 mb-3 flex items-center justify-between gap-4">
		<h3 class="section-title">{$t("plugin_manager.store")}</h3>
		<div class="relative w-72">
			<MagnifyingGlass size="14" class="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-faint" />
			<input bind:value={query} class="input h-8 pl-8" placeholder={$t("plugin_manager.search")} aria-label={$t("plugin_manager.search")} type="search" spellcheck="false" />
		</div>
	</div>

	<div class="notice notice-warning mb-4">
		<WarningCircle size="16" class="mt-px shrink-0" />
		<span>{$t("plugin_manager.warning", { PRODUCT_NAME })}</span>
	</div>

	{#if catalogueError}
		<p class="py-6 text-center text-xs text-ink-faint">The plugin catalogue could not be loaded. Check your internet connection and reopen this window.</p>
	{:else if !plugins}
		<p class="py-6 text-center text-xs text-ink-faint">{$t("plugin_manager.loading.open_source")}</p>
	{:else}
		<div class="mb-2 flex items-center gap-2">
			<h4 class="text-[13px] font-semibold text-ink">{$t("plugin_manager.open_source")}</h4>
			<Tooltip>{$t("plugin_manager.open_source.tooltip")}</Tooltip>
		</div>
		<div class="grid grid-cols-2 gap-2 lg:grid-cols-3">
			{#each Object.entries(plugins) as [id, plugin]}
				<ListedPlugin
					icon="https://openactionapi.github.io/plugins/icons/{id}.png"
					name={plugin.name}
					subtitle={plugin.author}
					hidden={!plugin.name.toLowerCase().includes(query.toLowerCase())}
					action={() => (openDetailsView = id)}
					actionLabel={$t("plugin_manager.view_details")}
				>
					<ArrowSquareOut size="18" class="text-ink-muted" />
				</ListedPlugin>
			{/each}
		</div>
	{/if}

	{#if "Tacto Connect".toLowerCase().includes(query.toLowerCase())}
		<div class="mt-6 mb-2 flex items-center gap-2">
			<h4 class="text-[13px] font-semibold text-ink">Tacto</h4>
			<Tooltip>{$t("plugin_manager.tacto.tooltip")}</Tooltip>
		</div>
		<div class="grid grid-cols-2 gap-2 lg:grid-cols-3">
			<ListedPlugin
				icon="https://tacto.live/icon-192.png"
				name="Tacto Connect"
				subtitle="Rivulus"
				action={() => {
					installPluginGitHub("us.rivul.tacto", {
						name: "Tacto Connect",
						author: "Rivulus",
						repository: "https://github.com/RivulusLive/tacto-desktop",
						download_url: undefined,
					});
				}}
				actionLabel={$t("plugin_details.install")}
				secondaryAction={() => window.open("https://tacto.live")}
				secondaryActionLabel={$t("plugin_manager.visit_website")}
			>
				<svelte:fragment slot="secondary">
					<ArrowSquareOut size="18" class="text-ink-muted" />
				</svelte:fragment>

				<CloudArrowDown size="18" class="text-ink-muted" />
			</ListedPlugin>
		</div>
	{/if}
</Popup>

{#if openDetailsView}
	<PluginDetails
		id={openDetailsView}
		details={plugins[openDetailsView]}
		install={() => {
			// @ts-expect-error
			installPluginGitHub(openDetailsView, plugins[openDetailsView]);
		}}
		close={() => (openDetailsView = null)}
	/>
{/if}

{#if choices}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="w-96 animate-pop-in rounded-xl border border-line-strong bg-panel p-4 shadow-[var(--shadow-pop)]">
			<h3 class="mb-3 text-[14px] font-semibold text-ink">{$t("plugin_manager.choose_asset")}</h3>
			<select class="select" bind:value={choice} aria-label={$t("plugin_manager.choose_asset.label")}>
				{#each choices as choice, i}
					<option value={i}>{choice.name}</option>
				{/each}
			</select>
			<div class="mt-3 flex justify-end gap-2">
				<button class="btn" on:click={cancelChoice}>Cancel</button>
				<button class="btn btn-primary" on:click={finishChoice}>{$t("plugin_details.install")}</button>
			</div>
		</div>
	</div>
{/if}
