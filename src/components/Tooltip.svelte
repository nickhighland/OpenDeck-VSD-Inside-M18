<script lang="ts">
	import Info from "phosphor-svelte/lib/Info";

	let show = false;
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<span class="relative inline-flex align-middle" on:mouseenter={() => (show = true)} on:mouseleave={() => (show = false)}>
	<button type="button" class="inline-flex text-ink-faint transition-colors hover:text-ink-muted" aria-label="More information" on:focus={() => (show = true)} on:blur={() => (show = false)}>
		<Info size="15" />
	</button>
	{#if show}
		<span class="absolute bottom-full left-1/2 z-50 mb-2 w-72 -translate-x-1/2 animate-fade-in rounded-lg border border-line-strong bg-overlay px-3 py-2 text-xs leading-relaxed font-normal text-ink-muted shadow-[var(--shadow-pop)]" role="tooltip">
			<slot />
		</span>
	{/if}
</span>
