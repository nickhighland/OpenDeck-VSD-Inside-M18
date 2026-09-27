<script lang="ts">
	export let icon: string;
	export let name: string;
	export let subtitle: string;
	export let hidden: boolean = false;
	export let disconnected: boolean = false;
	export let action: () => void;
	export let actionLabel: string = "";
	export let secondaryAction: (() => void) | undefined = undefined;
	export let secondaryActionLabel: string = "";
</script>

<div class="card flex items-center gap-3 p-3 transition-colors hover:border-line-strong" class:hidden>
	<img src={icon} class="size-12 shrink-0 rounded-xl bg-black/30 object-cover" class:opacity-60={disconnected} alt="" loading="lazy" />
	<div class="min-w-0 flex-1" class:opacity-70={disconnected}>
		<p class="truncate text-[13px] font-semibold text-ink" title={name}>{name}</p>
		<div class="flex min-w-0 flex-wrap items-center text-xs text-ink-muted">
			<slot name="subtitle">{subtitle}</slot>
		</div>
	</div>

	<div class="flex shrink-0 items-center gap-0.5">
		{#if secondaryAction}
			<button class="btn btn-ghost btn-icon btn-sm" on:click={secondaryAction} aria-label={secondaryActionLabel} title={secondaryActionLabel}>
				<slot name="secondary" />
			</button>
		{/if}
		{#if $$slots.default}
			<button class="btn btn-ghost btn-icon btn-sm" on:click={action} aria-label={actionLabel} title={actionLabel}>
				<slot />
			</button>
		{/if}
	</div>
</div>
