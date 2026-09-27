<script lang="ts">
	import ArrowSquareOut from "phosphor-svelte/lib/ArrowSquareOut";
	import DownloadSimple from "phosphor-svelte/lib/DownloadSimple";
	import Popup from "./Popup.svelte";

	import { t } from "$lib/i18n.ts";
	import "$lib/shims.ts";

	import { invoke } from "@tauri-apps/api/core";
	import DOMPurify from "dompurify";
	import { marked } from "marked";
	import markedAlert from "marked-alert";
	import { baseUrl } from "marked-base-url";
	import { onMount } from "svelte";

	export let id: string;
	export let details: { repository: string; name: string; author: string; download_url: string | undefined };
	let readme = `<strong>${$t("plugin_details.loading")}</strong>`;
	let downloadCount = 0;

	export let install: () => void;
	export let close: () => void;

	// @ts-expect-error
	const fetch = window.fetchNative ?? window.fetch;

	async function getReadme(repo: string): Promise<string> {
		const renderer = new marked.Renderer();
		renderer.link = function (token) {
			const rendered = marked.Renderer.prototype.link.call(this, token);
			return rendered.replace("<a", `<a target="_blank" `);
		};
		marked.use({ renderer });
		const urls = [
			"https://raw.githubusercontent.com/" + repo + "/main/README.md",
			"https://raw.githubusercontent.com/" + repo + "/main/readme.md",
			"https://raw.githubusercontent.com/" + repo + "/master/README.md",
			"https://raw.githubusercontent.com/" + repo + "/master/readme.md",
		];
		for (const url of urls) {
			const response = await fetch(url);
			if (response.ok) {
				marked.use(markedAlert());
				marked.use(baseUrl(url));
				return await marked.parse(DOMPurify.sanitize(await response.text()).replace(/<a/g, '<a target="_blank" '));
			}
		}
		return await marked.parse($t("plugin_details.readme.not_found", { repo }));
	}

	function handleReadmeClick(event: MouseEvent | KeyboardEvent) {
		const link = (event.target as HTMLElement).closest("a");
		if (link && link.href) {
			event.preventDefault();
			window.open(link.href);
		}
	}

	onMount(async () => {
		const repo = details.repository.split("/")[3] + "/" + details.repository.split("/")[4];

		try {
			readme = await getReadme(repo);
		} catch {
			readme = await marked.parse($t("plugin_details.readme.not_found", { repo }));
		}

		try {
			const releasesResponse = await fetch("https://api.github.com/repos/" + repo + "/releases");
			const releases = await releasesResponse.json();
			// GitHub answers with an error object when rate limited.
			if (!Array.isArray(releases)) return;
			for (const release of releases) {
				for (const asset of release.assets ?? []) {
					downloadCount += asset.download_count ?? 0;
				}
			}
		} catch {
			// The download count is optional.
		}
	});
</script>

<Popup show title={details.name} subtitle={`${$t("plugin_details.by")} ${details.author}`} size="lg" on:close={close}>
	<div class="flex items-start gap-6">
		<img src={"https://openactionapi.github.io/plugins/icons/" + id + ".png"} alt="" class="size-28 shrink-0 rounded-2xl shadow-[0_10px_30px_-12px_black]" />
		<div class="flex min-w-0 flex-col gap-3 pt-1">
			<div class="flex items-center gap-2 text-[13px] text-ink-muted">
				<img src={"https://avatars.githubusercontent.com/" + details.repository.split("/")[3]} alt="" class="size-6 rounded-full" />
				<button class="underline decoration-line-strong underline-offset-2 hover:text-ink" on:click={() => window.open("https://github.com/" + details.repository.split("/")[3])}>
					{details.author}
					{#if details.repository.split("/")[3] != details.author}
						({details.repository.split("/")[3]})
					{/if}
				</button>
				{#if downloadCount}
					<span class="flex items-center gap-1 text-ink-faint"><DownloadSimple size="14" /> {downloadCount.toLocaleString()}</span>
				{/if}
			</div>
			<div class="flex items-center gap-2">
				<button on:click={install} class="btn btn-primary h-9 px-5">{$t("plugin_details.install")}</button>
				<button
					on:click={() => invoke("open_url", { url: details.download_url ?? details.repository + "/releases/latest" })}
					class="btn h-9"
					aria-label={$t("plugin_details.download_latest")}
					title={$t("plugin_details.download_latest")}
				>
					<ArrowSquareOut size="15" /> Releases
				</button>
			</div>
		</div>
	</div>

	<!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
	<div class="plugin-readme mt-5 rounded-xl border border-line bg-black/20 p-5" on:click={handleReadmeClick} on:keyup={handleReadmeClick} role="region">
		{@html readme}
	</div>
</Popup>
