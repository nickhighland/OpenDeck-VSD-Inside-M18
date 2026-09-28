<script lang="ts">
	import { COMMAND_PRESETS } from "$lib/commands";

	import { createEventDispatcher, onDestroy } from "svelte";

	/** A shell command, run with /bin/sh. */
	export let value = "";
	export let label = "Command";

	const dispatch = createEventDispatcher<{ change: string }>();

	// Typing saves shortly after the last keystroke; presets and leaving the
	// field save at once.
	let timer: ReturnType<typeof setTimeout> | undefined;
	let pending: string | undefined;
	function save(next: string, immediate = false) {
		value = next;
		clearTimeout(timer);
		pending = next;
		if (immediate) flush();
		else timer = setTimeout(flush, 300);
	}
	function flush() {
		clearTimeout(timer);
		if (pending === undefined) return;
		dispatch("change", pending);
		pending = undefined;
	}
	onDestroy(flush);
</script>

<div class="flex gap-2">
	<input
		class="input input-mono min-w-0 flex-1"
		{value}
		placeholder="m1ddc set input 17"
		aria-label={label}
		spellcheck="false"
		autocomplete="off"
		autocapitalize="off"
		on:input={(event) => save(event.currentTarget.value)}
		on:blur={flush}
	/>
	<select
		class="select w-auto shrink-0"
		aria-label="Ready-made commands"
		value=""
		on:change={(event) => {
			const command = event.currentTarget.value;
			if (command) save(command, true);
			event.currentTarget.value = "";
		}}
	>
		<option value="">Presets…</option>
		{#each COMMAND_PRESETS as group}
			<optgroup label={group.group}>
				{#each group.items as item}<option value={item.command}>{item.label}</option>{/each}
			</optgroup>
		{/each}
	</select>
</div>
