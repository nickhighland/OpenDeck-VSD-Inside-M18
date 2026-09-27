<script lang="ts">
	import Keyboard from "phosphor-svelte/lib/Keyboard";
	import PencilSimple from "phosphor-svelte/lib/PencilSimple";
	import Record from "phosphor-svelte/lib/Record";
	import X from "phosphor-svelte/lib/X";

	import {
		describeSequence,
		formatShortcut,
		isMac,
		keyLabel,
		MODIFIER_NAMES,
		MODIFIER_SYMBOLS,
		MODIFIERS,
		modifiersFromEvent,
		PICKABLE_KEYS,
		shortcutFromEvent,
		shortcutHoldSequences,
		shortcutSequence,
		type Modifier,
		type Shortcut,
	} from "$lib/shortcuts";

	import { createEventDispatcher, onDestroy } from "svelte";

	/** The saved Enigo sequence. */
	export let value = "";
	/** A readable label saved with the sequence (VSD Craft imports provide one). */
	export let display = "";
	/** "tap" records a complete keystroke; "hold" presses on key-down and releases on key-up. */
	export let mode: "tap" | "hold" = "tap";
	export let label = "Shortcut";
	export let id = `shortcut-${Math.random().toString(36).slice(2)}`;

	const dispatch = createEventDispatcher<{ change: { down: string; up?: string; display: string } }>();

	let recording = false;
	let liveModifiers: Modifier[] = [];
	let showManual = false;

	$: described = describeSequence(value);
	$: keycaps = described ? [...described.modifiers.map((modifier) => MODIFIER_SYMBOLS[modifier]), described.key] : [];

	function commit(shortcut: Shortcut) {
		const text = formatShortcut(shortcut);
		if (mode === "hold") {
			const { down, up } = shortcutHoldSequences(shortcut);
			dispatch("change", { down, up, display: text });
		} else {
			dispatch("change", { down: shortcutSequence(shortcut), display: text });
		}
	}

	function onKeydown(event: KeyboardEvent) {
		if (!recording) return;
		event.preventDefault();
		event.stopPropagation();
		if (event.key === "Escape" && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
			stopRecording();
			return;
		}
		liveModifiers = modifiersFromEvent(event);
		const shortcut = shortcutFromEvent(event);
		if (!shortcut) return;
		stopRecording();
		commit(shortcut);
	}

	function onKeyup(event: KeyboardEvent) {
		if (!recording) return;
		event.preventDefault();
		liveModifiers = modifiersFromEvent(event);
	}

	function startRecording() {
		recording = true;
		liveModifiers = [];
		window.addEventListener("keydown", onKeydown, true);
		window.addEventListener("keyup", onKeyup, true);
	}

	function stopRecording() {
		recording = false;
		liveModifiers = [];
		window.removeEventListener("keydown", onKeydown, true);
		window.removeEventListener("keyup", onKeyup, true);
	}

	onDestroy(stopRecording);

	// Manual picker, for keys the keyboard lacks (F13–F20) or shortcuts the
	// system intercepts before the editor sees them (⌘Tab, ⌘Space).
	let manualModifiers: Modifier[] = [];
	let manualKey = "";
	function toggleManualModifier(modifier: Modifier) {
		manualModifiers = manualModifiers.includes(modifier) ? manualModifiers.filter((existing) => existing !== modifier) : [...manualModifiers, modifier];
	}
	function applyManual() {
		if (!manualKey) return;
		commit({ modifiers: manualModifiers, code: manualKey });
		showManual = false;
	}

	let rawValue = "";
	$: if (showManual) rawValue = value;
</script>

<div class="flex flex-col gap-2">
	<div class="flex items-center gap-2">
		<div
			{id}
			class="flex h-10 min-w-0 flex-1 items-center gap-1.5 rounded-lg border bg-black/25 px-2.5 transition-colors {recording ? 'recording border-accent' : 'border-line-strong'}"
			role="status"
			aria-label={label}
		>
			{#if recording}
				<span class="relative mr-1 flex size-2.5">
					<span class="absolute inline-flex size-full animate-ping rounded-full bg-danger opacity-60"></span>
					<span class="relative inline-flex size-2.5 rounded-full bg-danger"></span>
				</span>
				{#each liveModifiers as modifier}<span class="kbd">{MODIFIER_SYMBOLS[modifier]}</span>{/each}
				<span class="truncate text-xs text-ink-muted">{liveModifiers.length ? "…now press a key" : "Press the shortcut · Esc to cancel"}</span>
			{:else if keycaps.length}
				{#each keycaps as keycap}<span class="kbd">{keycap}</span>{/each}
			{:else if value.trim()}
				<Keyboard size="15" class="shrink-0 text-ink-faint" />
				<span class="truncate font-mono text-[11.5px] text-ink-muted" title={value}>{display || value}</span>
			{:else}
				<span class="text-xs text-ink-faint">No shortcut</span>
			{/if}
		</div>
		{#if recording}
			<button class="btn" on:click={stopRecording}>Cancel</button>
		{:else}
			<button class="btn btn-primary" on:click={startRecording}><Record size="14" weight="fill" /> Record</button>
		{/if}
		<button class="btn btn-icon" class:bg-overlay={showManual} on:click={() => (showManual = !showManual)} title="Choose keys manually or edit the raw sequence" aria-expanded={showManual} aria-label="Edit manually">
			<PencilSimple size="15" />
		</button>
		{#if value.trim() && !recording}
			<button class="btn btn-ghost btn-icon" on:click={() => dispatch("change", { down: "", up: mode === "hold" ? "" : undefined, display: "" })} title="Remove the shortcut" aria-label="Remove the shortcut">
				<X size="15" />
			</button>
		{/if}
	</div>

	{#if showManual}
		<div class="card flex flex-col gap-3 p-3">
			<div class="flex flex-wrap items-center gap-2">
				{#each MODIFIERS as modifier}
					<button class="btn btn-sm" class:btn-primary={manualModifiers.includes(modifier)} aria-pressed={manualModifiers.includes(modifier)} on:click={() => toggleManualModifier(modifier)}>
						{MODIFIER_SYMBOLS[modifier]}
						{#if isMac}<span class="text-[11px] opacity-80">{MODIFIER_NAMES[modifier]}</span>{/if}
					</button>
				{/each}
				<span class="text-ink-faint">+</span>
				<select class="select w-40" bind:value={manualKey} aria-label="Key">
					<option value="">Choose a key…</option>
					{#each PICKABLE_KEYS as group}
						<optgroup label={group.group}>
							{#each group.keys as key}<option value={key.code}>{keyLabel(key.code)}</option>{/each}
						</optgroup>
					{/each}
				</select>
				<button class="btn btn-sm btn-primary" disabled={!manualKey} on:click={applyManual}>Use</button>
			</div>
			<label class="flex flex-col gap-1.5">
				<span class="label">Raw sequence (advanced)</span>
				<input
					class="input input-mono"
					bind:value={rawValue}
					spellcheck="false"
					placeholder="[k(Meta,Press),r(40),k(Meta,Release)]"
					on:change={() => dispatch("change", { down: rawValue.trim(), display: "" })}
				/>
				<span class="hint">Enigo tokens, for sequences the recorder cannot express, like typed text <code class="font-mono">t("hi")</code> or several keys in a row.</span>
			</label>
		</div>
	{/if}
</div>

<style>
	.recording {
		box-shadow: 0 0 0 3px var(--color-accent-soft);
	}
</style>
