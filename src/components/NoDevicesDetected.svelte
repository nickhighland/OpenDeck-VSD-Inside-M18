<script lang="ts">
	import ArrowClockwise from "phosphor-svelte/lib/ArrowClockwise";
	import Plug from "phosphor-svelte/lib/Plug";
	import ProhibitInset from "phosphor-svelte/lib/ProhibitInset";
	import TerminalWindow from "phosphor-svelte/lib/TerminalWindow";
	import Usb from "phosphor-svelte/lib/Usb";

	import { t } from "$lib/i18n";
	import { PRODUCT_NAME } from "$lib/singletons";

	import { invoke } from "@tauri-apps/api/core";

	let buildInfo = "";
	(async () => {
		try {
			buildInfo = await invoke("get_build_info");
		} catch {}
	})();
	$: isLinux = buildInfo.split("</summary>")[0]?.includes("linux") ?? false;
</script>

<div class="flex h-full w-full flex-col items-center justify-center px-8 text-center">
	<div class="relative mb-6">
		<div class="absolute inset-0 animate-ping rounded-[22%] bg-accent/20 [animation-duration:2.4s]"></div>
		<img src="/app-icon.png" alt="" class="relative size-20 drop-shadow-[0_12px_30px_rgb(0_0_0/0.55)]" />
	</div>
	<h2 class="text-lg font-semibold text-ink">Looking for your VSD Inside M18…</h2>
	<p class="mt-1 max-w-md text-[13px] text-ink-muted">The M18 appears here as soon as it is connected. {PRODUCT_NAME} keeps checking in the background.</p>

	<ul class="mt-7 grid w-full max-w-lg gap-2 text-left">
		<li class="card flex items-start gap-3 p-3">
			<Usb size="18" class="mt-0.5 shrink-0 text-accent" />
			<div>
				<p class="text-[13px] font-medium text-ink">Check the cable</p>
				<p class="hint">Use a USB cable that carries data, not only power, and try another port.</p>
			</div>
		</li>
		<li class="card flex items-start gap-3 p-3">
			<ProhibitInset size="18" class="mt-0.5 shrink-0 text-accent" />
			<div>
				<p class="text-[13px] font-medium text-ink">Quit VSD Craft</p>
				<p class="hint">Only one app can control the M18 at a time.</p>
			</div>
		</li>
		{#if isLinux}
			<li class="card flex items-start gap-3 p-3">
				<TerminalWindow size="18" class="mt-0.5 shrink-0 text-accent" />
				<div>
					<p class="text-[13px] font-medium text-ink">Install the udev rule</p>
					<p class="hint">{$t("no_devices_detected.check_udev")}</p>
				</div>
			</li>
		{/if}
		<li class="card flex items-start gap-3 p-3">
			<Plug size="18" class="mt-0.5 shrink-0 text-accent" />
			<div>
				<p class="text-[13px] font-medium text-ink">Unplug it and plug it back in</p>
				<p class="hint">If it still does not appear, restart the app, then check the logs in Settings › Backup & Import.</p>
			</div>
		</li>
	</ul>

	<button class="btn mt-6" on:click={() => invoke("restart")}>
		<ArrowClockwise size="14" />
		{$t("no_devices_detected.restart", { PRODUCT_NAME })}
	</button>
</div>
