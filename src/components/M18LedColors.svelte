<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";

	import { invoke } from "@tauri-apps/api/core";

	const LED_COUNT = 24;
	const DEFAULT_COLOR = "#780000";

	export let instance: ActionInstance;
	let palette: string[] = [];
	let saving = false;

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

	async function save() {
		if (!instance) return;
		saving = true;
		try {
			await invoke("set_m18_led_palette", { context: instance.context, colors: palette });
		} finally {
			saving = false;
		}
	}

	function setAll(event: Event) {
		const color = (event.currentTarget as HTMLInputElement).value;
		palette = Array(LED_COUNT).fill(color);
		void save();
	}

	function setOne(index: number, event: Event) {
		palette[index] = (event.currentTarget as HTMLInputElement).value;
		palette = palette;
		void save();
	}
</script>

<div class="h-full overflow-auto p-3 text-neutral-300">
	<div class="mb-3 flex items-center justify-between">
		<div>
			<h2 class="font-semibold">M18 LED colors</h2>
			<p class="text-xs text-neutral-400">24 LEDs. Changes apply immediately.</p>
		</div>
		<input type="color" value={palette[0] ?? DEFAULT_COLOR} on:input={setAll} disabled={saving} aria-label="Set all LEDs" />
	</div>
	<div class="grid grid-cols-8 gap-2">
		{#each palette as color, index}
			<label class="flex flex-col items-center gap-1 text-[10px] text-neutral-500">
				<span>{index + 1}</span>
				<input type="color" value={color} on:input={(event) => setOne(index, event)} disabled={saving} aria-label={`LED ${index + 1}`} />
			</label>
		{/each}
	</div>
</div>
