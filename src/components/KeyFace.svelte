<script lang="ts">
	import type { ActionState } from "$lib/ActionState";

	import { CanvasLock, renderImage } from "$lib/rendererHelper";

	/** The appearance to draw, as the M18 would show it. */
	export let state: ActionState | undefined;
	/** Artwork used when the state has no image of its own. */
	export let fallback: string | undefined = undefined;

	let canvas: HTMLCanvasElement;
	const lock = new CanvasLock();

	async function draw(shown: ActionState | undefined) {
		const unlock = await lock.lock();
		try {
			if (!canvas) return;
			if (!shown) canvas.getContext("2d")?.clearRect(0, 0, canvas.width, canvas.height);
			else await renderImage(canvas, null, shown, fallback, false, false, true, false, false, 0);
		} finally {
			unlock();
		}
	}
	$: if (canvas) draw(state ? structuredClone(state) : undefined);
</script>

<canvas bind:this={canvas} width="144" height="144" class="size-full rounded-[22%] bg-black" aria-hidden="true" />
