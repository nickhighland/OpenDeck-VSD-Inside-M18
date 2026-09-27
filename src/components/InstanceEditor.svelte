<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { ActionState } from "$lib/ActionState";

	import ArrowCounterClockwise from "phosphor-svelte/lib/ArrowCounterClockwise";
	import ImageSquare from "phosphor-svelte/lib/ImageSquare";
	import Minus from "phosphor-svelte/lib/Minus";
	import PaintBucket from "phosphor-svelte/lib/PaintBucket";
	import Plus from "phosphor-svelte/lib/Plus";
	import TextB from "phosphor-svelte/lib/TextB";
	import TextItalic from "phosphor-svelte/lib/TextItalic";
	import TextUnderline from "phosphor-svelte/lib/TextUnderline";

	import { iconSource, isDefaultArtwork, launchesSomething, resolveState } from "$lib/appIcons";
	import { t } from "$lib/i18n";
	import { CanvasLock, renderImage, resizeImage } from "$lib/rendererHelper";

	import { invoke } from "@tauri-apps/api/core";
	import { createEventDispatcher, onDestroy, onMount } from "svelte";

	export let instance: ActionInstance;

	// Edits change `instance` in place; this tells the inspector to redraw its header.
	const dispatch = createEventDispatcher<{ edit: void }>();

	let state: number = 0;
	$: if (state >= instance.states.length) state = 0;

	let fonts: string[] = [];
	onMount(async () => {
		try {
			fonts = await invoke("get_fonts");
		} catch {
			fonts = [];
		}
	});

	let fileInput: HTMLInputElement;
	let solidColourInput: HTMLInputElement;

	$: current = instance.states[state];
	$: bold = current?.style.includes("Bold") ?? false;
	$: italic = current?.style.includes("Italic") ?? false;

	function setStyle(nextBold: boolean, nextItalic: boolean) {
		instance.states[state].style = nextBold && nextItalic ? "Bold Italic" : nextBold ? "Bold" : nextItalic ? "Italic" : "Regular";
	}

	function adjustImageScale(delta: number) {
		const next = (instance.states[state].image_scale || 100) + delta;
		instance.states[state].image_scale = Math.max(10, Math.min(200, next));
	}

	// Keys that launch an app (or open a file) show its icon until an image is chosen.
	$: launches = launchesSomething(instance.action.uuid);
	$: automaticIcon = launches && isDefaultArtwork(current?.image);
	$: source = iconSource(instance.action.uuid);

	function resetImage() {
		const original = instance.action.states[state]?.image ?? instance.action.icon;
		// Launching keys go back to the app's own icon.
		instance.states[state].image = launches && !isDefaultArtwork(original) ? "" : original;
		instance.states[state].image_scale = 100;
	}

	async function useFile(file: File | undefined) {
		if (!file || !file.type.startsWith("image/")) return;
		const reader = new FileReader();
		reader.onload = async () => {
			const result = reader.result?.toString();
			if (!result) return;
			instance.states[state].image = (await resizeImage(result)) ?? result;
		};
		reader.readAsDataURL(file);
	}

	function useSolidColour() {
		const canvas = document.createElement("canvas");
		canvas.width = 1;
		canvas.height = 1;
		const context = canvas.getContext("2d");
		if (!context) return;
		context.fillStyle = solidColourInput.value;
		context.fillRect(0, 0, canvas.width, canvas.height);
		instance.states[state].image = canvas.toDataURL("image/png");
	}

	// Save shortly after the last change, not on every keystroke or colour-picker move.
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	let pending: { context: string; index: number; state: unknown } | undefined;
	function flush() {
		clearTimeout(saveTimer);
		saveTimer = undefined;
		if (!pending) return;
		const request = pending;
		pending = undefined;
		invoke("set_state", request).catch((error) => console.warn("Failed to save the key appearance", error));
	}
	let editing = "";
	let saved = "";
	$: {
		const key = `${instance.context}#${state}`;
		const snapshot = structuredClone(instance.states[state]);
		const serialised = JSON.stringify(snapshot ?? null);
		if (key !== editing) {
			// Another key or state was opened: save what is pending for the
			// previous one, but do not re-save the one that was just opened.
			flush();
			editing = key;
			saved = serialised;
		} else if (snapshot && serialised !== saved) {
			// Only real edits are saved, not redraws of unchanged content.
			saved = serialised;
			pending = { context: instance.context, index: state, state: snapshot };
			clearTimeout(saveTimer);
			saveTimer = setTimeout(flush, 150);
			dispatch("edit");
		}
	}
	onDestroy(flush);

	let canvas: HTMLCanvasElement;
	let showsAppIcon = false;
	let previewRequest = 0;
	const previewLock = new CanvasLock();
	async function drawPreview(shown: ActionState, settings: unknown) {
		const request = ++previewRequest;
		const resolved = await resolveState({ action: instance.action, settings }, shown);
		const unlock = await previewLock.lock();
		try {
			// A newer edit already started its own draw.
			if (request !== previewRequest) return;
			showsAppIcon = resolved.appIcon;
			await renderImage(canvas, null, resolved.state, instance.action.states[state]?.image ?? instance.action.icon, false, false, true, false, false, 0);
		} finally {
			unlock();
		}
	}
	$: if (canvas && current) drawPreview(current, instance.settings);

	let dragging = false;
	const alignments = ["top", "middle", "bottom"] as const;
</script>

<div class="flex gap-5">
	<div class="flex w-[8.5rem] shrink-0 flex-col items-center gap-2.5">
		<button
			class="group relative size-[8.5rem] overflow-hidden rounded-[22px] bg-black shadow-[inset_0_0_0_1px_rgb(255_255_255/0.08),0_10px_24px_-12px_black] transition-shadow"
			class:ring-2={dragging}
			class:ring-accent={dragging}
			on:click={() => fileInput.click()}
			on:dragover|preventDefault={(event) => {
				dragging = true;
				if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
			}}
			on:dragleave={() => (dragging = false)}
			on:drop|preventDefault={(event) => {
				dragging = false;
				useFile(event.dataTransfer?.files?.[0]);
			}}
			title={$t("instance_editor.image.hint")}
			aria-label={$t("instance_editor.image.hint")}
		>
			<canvas bind:this={canvas} class="size-full" width={144} height={144} aria-label={$t("instance_editor.image.n", { n: state + 1 })} />
			<span class="absolute inset-0 flex flex-col items-center justify-center gap-1 bg-black/60 text-[11px] font-medium text-white opacity-0 transition-opacity group-hover:opacity-100">
				<ImageSquare size="20" />
				Choose image
			</span>
		</button>
		<div class="flex items-center gap-1">
			<button class="btn btn-sm btn-icon" on:click={() => adjustImageScale(-10)} aria-label={$t("instance_editor.image.scale.decrease")}><Minus size="12" weight="bold" /></button>
			<span class="w-11 text-center text-[11px] text-ink-muted tabular-nums">{current?.image_scale || 100}%</span>
			<button class="btn btn-sm btn-icon" on:click={() => adjustImageScale(10)} aria-label={$t("instance_editor.image.scale.increase")}><Plus size="12" weight="bold" /></button>
		</div>
		<div class="flex w-full flex-col gap-1">
			<button class="btn btn-sm w-full" on:click={() => solidColourInput.click()}>
				<PaintBucket size="13" />
				{$t("instance_editor.solid_colour")}
			</button>
			{#if automaticIcon}
				<p class="hint text-center">{showsAppIcon ? `Showing the ${source}'s own icon` : `Shows the ${source}'s icon once one is chosen`}</p>
			{:else}
				<button class="btn btn-sm btn-ghost w-full" on:click={resetImage}><ArrowCounterClockwise size="13" /> {launches ? `Use ${source} icon` : "Reset image"}</button>
			{/if}
		</div>
		<input bind:this={solidColourInput} type="color" class="invisible absolute size-0" value="#7160fb" on:change={useSolidColour} />
		<input
			bind:this={fileInput}
			type="file"
			class="hidden"
			accept="image/*"
			on:change={() => {
				useFile(fileInput.files?.[0]);
				fileInput.value = "";
			}}
		/>
	</div>

	{#if current}
		<div class="flex min-w-0 flex-1 flex-col gap-3.5">
			{#if instance.states.length > 1}
				<div class="flex items-center gap-2">
					<span class="label">{$t("instance_editor.state")}</span>
					<div class="segmented" role="tablist">
						{#each instance.states as _, index}
							<button role="tab" aria-selected={state === index} on:click={() => (state = index)}>{$t("instance_editor.state.n", { n: index + 1 })}</button>
						{/each}
					</div>
				</div>
			{/if}

			<div class="grid grid-cols-[1fr_auto] items-end gap-3">
				<label>
					<span class="label mb-1.5">Title</span>
					<textarea bind:value={instance.states[state].text} placeholder={instance.action.states[state]?.text || "No title"} rows="2" class="textarea"></textarea>
				</label>
				<label class="flex flex-col items-center gap-1.5 pb-2">
					<span class="label">{$t("instance_editor.show")}</span>
					<input type="checkbox" class="switch" bind:checked={instance.states[state].show} />
				</label>
			</div>

			<div class="flex flex-wrap items-center gap-x-5 gap-y-3" class:opacity-40={!current.show}>
				<div class="flex items-center gap-2">
					<span class="label">Position</span>
					<div class="segmented">
						{#each alignments as alignment}
							<button aria-pressed={current.alignment === alignment} on:click={() => (instance.states[state].alignment = alignment)}>{$t(`instance_editor.alignment.${alignment}`)}</button>
						{/each}
					</div>
				</div>
				<div class="flex items-center gap-1">
					<button class="btn btn-sm btn-icon" class:btn-primary={bold} aria-pressed={bold} on:click={() => setStyle(!bold, italic)} aria-label="Bold"><TextB size="14" weight="bold" /></button>
					<button class="btn btn-sm btn-icon" class:btn-primary={italic} aria-pressed={italic} on:click={() => setStyle(bold, !italic)} aria-label="Italic"><TextItalic size="14" /></button>
					<button class="btn btn-sm btn-icon" class:btn-primary={current.underline} aria-pressed={current.underline} on:click={() => (instance.states[state].underline = !current.underline)} aria-label="Underline"><TextUnderline size="14" /></button>
				</div>
			</div>

			<div class="grid grid-cols-[minmax(0,1fr)_5rem] gap-3" class:opacity-40={!current.show}>
				<label>
					<span class="label mb-1.5">{$t("instance_editor.font")}</span>
					<input list="font-families" bind:value={instance.states[state].family} placeholder={$t("instance_editor.font.placeholder")} class="input" />
					<datalist id="font-families">
						<option value="Liberation Sans">Liberation Sans</option>
						<option value="Archivo Black">Archivo Black</option>
						<option value="Comic Neue">Comic Neue</option>
						<option value="Courier Prime">Courier Prime</option>
						<option value="Tinos">Tinos</option>
						<option value="Anton">Anton</option>
						<option value="Liberation Serif">Liberation Serif</option>
						<option value="Open Sans">Open Sans</option>
						<option value="Fira Sans">Fira Sans</option>
						{#each fonts as font}<option value={font}>{font}</option>{/each}
					</datalist>
				</label>
				<label>
					<span class="label mb-1.5">{$t("instance_editor.font.size")}</span>
					<input type="number" min="4" max="72" bind:value={instance.states[state].size} class="input tabular-nums" />
				</label>
			</div>

			<div class="flex flex-wrap items-center gap-x-5 gap-y-2" class:opacity-40={!current.show}>
				<label class="flex items-center gap-2">
					<input type="color" bind:value={instance.states[state].colour} class="color-swatch" />
					<span class="text-xs text-ink-muted">{$t("instance_editor.colour")}</span>
				</label>
				<label class="flex items-center gap-2">
					<input type="color" bind:value={instance.states[state].stroke_colour} class="color-swatch" />
					<span class="text-xs text-ink-muted">{$t("instance_editor.outline")}</span>
					<input type="number" min="0" max="20" bind:value={instance.states[state].stroke_size} class="input h-7 min-h-0 w-14 py-0 tabular-nums" aria-label="Outline width" />
				</label>
				<label class="flex items-center gap-2">
					<input type="color" bind:value={instance.states[state].background_colour} class="color-swatch" />
					<span class="text-xs text-ink-muted">Background</span>
				</label>
			</div>
		</div>
	{/if}
</div>

<style>
	.color-swatch {
		width: 1.75rem;
		height: 1.75rem;
		padding: 0;
		border: 1px solid var(--color-line-strong);
		border-radius: 0.45rem;
		background: none;
		overflow: hidden;
	}
	.color-swatch::-webkit-color-swatch-wrapper {
		padding: 0;
	}
	.color-swatch::-webkit-color-swatch {
		border: none;
		border-radius: 0.4rem;
	}
</style>
