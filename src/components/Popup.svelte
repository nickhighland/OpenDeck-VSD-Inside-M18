<script lang="ts">
	import X from "phosphor-svelte/lib/X";

	import { createEventDispatcher, onDestroy, tick } from "svelte";

	export let show = false;
	export let label = "";
	/** When set, the dialog draws its own title bar with a close button. */
	export let title = "";
	export let subtitle = "";
	export let size: "sm" | "md" | "lg" | "xl" = "lg";

	const dispatch = createEventDispatcher<{ close: void }>();

	let dialogEl: HTMLDivElement;
	let previousFocus: HTMLElement | null = null;

	$: if (show) {
		previousFocus = document.activeElement as HTMLElement | null;
		tick().then(() => dialogEl?.focus());
	} else if (previousFocus) {
		previousFocus.focus();
		previousFocus = null;
	}

	onDestroy(() => previousFocus?.focus());

	function close() {
		show = false;
		dispatch("close");
	}

	const widths = { sm: "max-w-md", md: "max-w-xl", lg: "max-w-3xl", xl: "max-w-5xl" };
</script>

<svelte:window
	on:keydown={(event) => {
		if (show && event.key === "Escape") {
			event.stopPropagation();
			close();
		}
	}}
/>

{#if show}
	<div class="fixed inset-0 z-40 flex animate-fade-in items-center justify-center bg-black/55 p-6 backdrop-blur-[2px]" role="presentation" on:mousedown|self={close}>
		<div
			bind:this={dialogEl}
			class="flex max-h-full w-full {widths[size]} animate-pop-in flex-col overflow-hidden rounded-2xl border border-line-strong bg-panel shadow-[var(--shadow-pop)] outline-none"
			role="dialog"
			aria-modal="true"
			aria-label={label || title}
			tabindex="-1"
		>
			{#if title}
				<div class="flex shrink-0 items-start gap-3 border-b border-line px-5 py-4">
					<div class="min-w-0 flex-1">
						<h2 class="text-[15px] font-semibold text-ink">{title}</h2>
						{#if subtitle}<p class="mt-0.5 text-xs text-ink-muted">{subtitle}</p>{/if}
					</div>
					<slot name="actions" />
					<button class="btn btn-ghost btn-icon btn-sm" on:click={close} aria-label="Close">
						<X size="15" />
					</button>
				</div>
			{/if}
			{#if $$slots.header}
				<div class="shrink-0 px-5 pt-4">
					<slot name="header" />
				</div>
			{/if}
			<div class="min-h-0 flex-1 overflow-auto px-5 py-4">
				<slot />
			</div>
			{#if $$slots.footer}
				<div class="shrink-0 border-t border-line px-5 py-3">
					<slot name="footer" />
				</div>
			{/if}
		</div>
	</div>
{/if}
