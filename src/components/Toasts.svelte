<script lang="ts">
	import CheckCircle from "phosphor-svelte/lib/CheckCircle";
	import Info from "phosphor-svelte/lib/Info";
	import WarningCircle from "phosphor-svelte/lib/WarningCircle";
	import X from "phosphor-svelte/lib/X";

	import { dismissToast, toasts } from "$lib/toast";
</script>

<div class="pointer-events-none fixed right-4 bottom-4 z-[60] flex w-80 flex-col gap-2" aria-live="polite">
	{#each $toasts as item (item.id)}
		<div class="pointer-events-auto flex animate-toast-in items-start gap-2.5 rounded-xl border border-line-strong bg-overlay/95 p-3 shadow-[var(--shadow-pop)] backdrop-blur-xl" role={item.kind === "error" ? "alert" : "status"}>
			{#if item.kind === "error"}
				<WarningCircle size="18" weight="fill" class="mt-px shrink-0 text-danger" />
			{:else if item.kind === "success"}
				<CheckCircle size="18" weight="fill" class="mt-px shrink-0 text-success" />
			{:else}
				<Info size="18" weight="fill" class="mt-px shrink-0 text-accent" />
			{/if}
			<div class="min-w-0 flex-1">
				<p class="text-[13px] font-semibold text-ink">{item.title}</p>
				{#if item.detail}
					<p class="mt-0.5 text-xs break-words text-ink-muted">{item.detail}</p>
				{/if}
			</div>
			<button class="btn-ghost btn btn-sm btn-icon -mt-0.5 -mr-1" on:click={() => dismissToast(item.id)} aria-label="Dismiss">
				<X size="14" />
			</button>
		</div>
	{/each}
</div>
