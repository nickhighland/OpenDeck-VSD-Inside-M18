<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";

	import { invoke } from "@tauri-apps/api/core";
	import { onDestroy } from "svelte";

	const LED_COUNT = 24;
	const DEFAULT_COLOR = "#780000";

	export let instance: ActionInstance;
	let palette: string[] = [];

	function normalise(value: unknown): string[] {
		if (!Array.isArray(value) || value.length !== LED_COUNT || value.some((color) => typeof color !== "string" || !/^#[0-9a-f]{6}$/i.test(color))) {
			return Array(LED_COUNT).fill(DEFAULT_COLOR);
		}
		return value.map((color) => color.toLowerCase());
	}

	$: {
		const next = normalise(instance?.settings?.ledColors);
		if (JSON.stringify(next) !== JSON.stringify(palette)) palette = next;
	}

	// Colour pickers fire continuously while dragging; send the palette at
	// most every 80 ms so the USB link is not flooded.
	let saveTimer: ReturnType<typeof setTimeout> | undefined;
	let lastSent = 0;
	function save() {
		clearTimeout(saveTimer);
		const wait = Math.max(0, 80 - (Date.now() - lastSent));
		saveTimer = setTimeout(async () => {
			lastSent = Date.now();
			if (!instance) return;
			instance.settings = { ...(instance.settings ?? {}), ledColors: [...palette] };
			try {
				await invoke("set_m18_led_palette", { context: instance.context, colors: palette });
			} catch (error) {
				console.warn("Failed to set the M18 LED colors", error);
			}
		}, wait);
	}
	onDestroy(() => clearTimeout(saveTimer));

	function setAll(color: string) {
		palette = Array(LED_COUNT).fill(color);
		save();
	}

	function setOne(index: number, color: string) {
		palette[index] = color;
		palette = palette;
		save();
	}

	function hsl(hue: number, saturation = 90, lightness = 55): string {
		const s = saturation / 100;
		const l = lightness / 100;
		const f = (n: number) => {
			const k = (n + hue / 30) % 12;
			const color = l - s * Math.min(l, 1 - l) * Math.max(-1, Math.min(k - 3, 9 - k, 1));
			return Math.round(255 * color)
				.toString(16)
				.padStart(2, "0");
		};
		return `#${f(0)}${f(8)}${f(4)}`;
	}

	function gradient(from: number, to: number): string[] {
		return Array.from({ length: LED_COUNT }, (_, index) => hsl(from + ((to - from) * index) / (LED_COUNT - 1)));
	}

	const presets: { name: string; colors: string[] }[] = [
		{ name: "Rainbow", colors: gradient(0, 330) },
		{ name: "Ocean", colors: gradient(170, 240) },
		{ name: "Sunset", colors: gradient(-20, 50) },
		{ name: "Violet", colors: gradient(250, 300) },
		{ name: "Warm white", colors: Array(LED_COUNT).fill("#ffb46b") },
		{ name: "White", colors: Array(LED_COUNT).fill("#ffffff") },
		{ name: "Red", colors: Array(LED_COUNT).fill(DEFAULT_COLOR) },
		{ name: "Off", colors: Array(LED_COUNT).fill("#000000") },
	];

	function applyPreset(colors: string[]) {
		palette = [...colors];
		save();
	}
</script>

<div class="mx-auto flex max-w-2xl flex-col gap-4 px-5 py-4">
	<div class="rounded-xl border border-line bg-black/40 p-3">
		<div class="grid grid-cols-12 gap-1.5">
			{#each palette as color, index}
				<label class="group relative flex flex-col items-center gap-1" title={`LED ${index + 1}`}>
					<span
						class="size-6 rounded-full border border-white/10 transition-transform group-hover:scale-110"
						style={`background: ${color}; box-shadow: 0 0 12px -1px ${color === "#000000" ? "transparent" : color};`}
					></span>
					<span class="text-[9.5px] text-ink-faint tabular-nums">{index + 1}</span>
					<input type="color" value={color} class="absolute inset-0 cursor-pointer opacity-0" on:input={(event) => setOne(index, event.currentTarget.value)} aria-label={`LED ${index + 1}`} />
				</label>
			{/each}
		</div>
	</div>

	<div class="flex items-center justify-between gap-3">
		<div>
			<p class="text-[13px] text-ink">Set every LED</p>
			<p class="hint">Changes apply to the M18 immediately. Click a single LED above to change just that one.</p>
		</div>
		<label class="btn relative shrink-0">
			<span class="size-4 rounded-full border border-white/20" style={`background: ${palette[0] ?? DEFAULT_COLOR}`}></span>
			Pick color
			<input type="color" value={palette[0] ?? DEFAULT_COLOR} class="absolute inset-0 cursor-pointer opacity-0" on:input={(event) => setAll(event.currentTarget.value)} aria-label="Set all LEDs" />
		</label>
	</div>

	<div>
		<p class="section-title mb-2">Presets</p>
		<div class="flex flex-wrap gap-2">
			{#each presets as preset}
				<button class="btn btn-sm" on:click={() => applyPreset(preset.colors)}>
					<span class="h-2.5 w-7 rounded-full border border-white/10" style={`background: linear-gradient(90deg, ${preset.colors[0]}, ${preset.colors[Math.floor(LED_COUNT / 2)]}, ${preset.colors[LED_COUNT - 1]});`}></span>
					{preset.name}
				</button>
			{/each}
		</div>
	</div>
</div>
