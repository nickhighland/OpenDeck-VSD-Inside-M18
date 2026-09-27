<script lang="ts">
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { Context } from "$lib/Context";
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { Profile } from "$lib/Profile";
	import type { CopiedItem } from "$lib/propertyInspector";

	import Key from "./Key.svelte";

	import { t } from "$lib/i18n";
	import { inspectedInstance, inspectedParentAction } from "$lib/propertyInspector";
	import { attempt } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";

	export let device: DeviceInfo;
	export let profile: Profile;

	export let selectedDevice: string;

	// The M18 has 15 LCD keys in three rows and a separate row of three
	// physical buttons. The backend keeps OpenDeck's 4x5 position numbering
	// for plugin compatibility, but the editor must show the real hardware.
	const M18_LCD_ROWS = 3;
	const M18_LCD_COLUMNS = 5;
	const M18_BOTTOM_KEYS = 3;
	$: isM18 = device.id.startsWith("18-");

	/** Key size adapts to the space available, within comfortable limits. */
	let areaWidth = 900;
	let areaHeight = 600;
	$: keySize = Math.round(Math.max(64, Math.min(104, (areaWidth - 150) / 5.9, (areaHeight - 170) / 4.35)));
	$: keyGap = Math.round(keySize * 0.17);

	function handleDragStart({ dataTransfer }: DragEvent, controller: string, position: number) {
		if (!dataTransfer) return;
		dataTransfer.effectAllowed = "move";
		dataTransfer.setData("controller", controller);
		dataTransfer.setData("position", position.toString());
		dataTransfer.setData("profile", profile.id);
	}

	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		if (!event.dataTransfer) return;
		if (event.dataTransfer.types.includes("action")) event.dataTransfer.dropEffect = "copy";
		else if (event.dataTransfer.types.includes("controller")) event.dataTransfer.dropEffect = "move";
	}

	async function handleDrop({ dataTransfer }: DragEvent, controller: string, position: number) {
		let context = { device: device.id, profile: profile.id, controller, position };
		let array = controller == "Encoder" ? profile.sliders : controller == "Infobar" ? profile.infobars : profile.keys;
		if (dataTransfer?.getData("action")) {
			let action = JSON.parse(dataTransfer.getData("action"));
			if (array[position]) {
				return;
			}
			const created = await attempt("Could not add the action", () => invoke<ActionInstance | null>("create_instance", { context, action }));
			if (created === undefined) return;
			array[position] = created;
			profile = profile;
			if (created) $inspectedInstance = created.context;
		} else if (dataTransfer?.getData("controller")) {
			let oldController = dataTransfer.getData("controller");
			let oldArray = oldController == "Encoder" ? profile.sliders : oldController == "Infobar" ? profile.infobars : profile.keys;
			let oldPosition = parseInt(dataTransfer.getData("position"));
			// A key dragged in from another page (by hovering a page tab) keeps
			// its original page as the source.
			let oldProfile = dataTransfer.getData("profile") || profile.id;
			if (oldController == controller && oldPosition == position && oldProfile == profile.id) return;
			const source = { device: device.id, profile: oldProfile, controller: oldController, position: oldPosition };
			if (isM18 && controller == "Keypad" && oldController == "Keypad" && array[position]) {
				if (oldProfile != profile.id) return;
				const swapped = await attempt("Could not swap the keys", () =>
					invoke<{ source: ActionInstance; destination: ActionInstance }>("swap_m18_instances", { source, destination: context }),
				);
				if (!swapped) return;
				oldArray[oldPosition] = swapped.source;
				array[position] = swapped.destination;
				profile = profile;
				return;
			}
			const moved = await attempt("Could not move the key", () => invoke<ActionInstance | null>("move_instance", { source, destination: context, retain: false }));
			if (moved) {
				array[position] = moved;
				if (oldProfile == profile.id) oldArray[oldPosition] = null;
				profile = profile;
			}
		}
	}

	async function handlePaste(item: CopiedItem, destination: Context) {
		let array = destination.controller == "Encoder" ? profile.sliders : destination.controller == "Infobar" ? profile.infobars : profile.keys;

		if (item.type == "action") {
			if (array[destination.position]) return;
			const created = await attempt("Could not paste the action", () => invoke<ActionInstance | null>("create_instance", { context: destination, action: item.action }));
			if (created === undefined) return;
			array[destination.position] = created;
			profile = profile;
			return;
		}

		const copied = await attempt("Could not paste the key", () => invoke<ActionInstance | null>("move_instance", { source: item.source, destination, retain: true }));
		if (copied) {
			array[destination.position] = copied;
			profile = profile;
		}
	}

	// Grid navigation: track focused cell and compute row lengths for arrow key movement.
	let focusedRow = 0;
	let focusedCol = 0;

	$: gridRowLengths = isM18
		? [...Array(M18_LCD_ROWS).fill(M18_LCD_COLUMNS), M18_BOTTOM_KEYS]
		: [
				...Array(device.rows).fill(device.columns),
				...(device.encoders > 0 ? [device.encoders] : []),
				...(device.touchpoints > 0 || device.infobars > 0 ? [device.touchpoints + device.infobars] : []),
			];
	$: encoderRowIndex = device.rows;
	$: touchpointRowIndex = device.rows + (device.encoders > 0 ? 1 : 0);

	function flatIndexFromRowCol(row: number, col: number): number {
		let index = 0;
		for (let r = 0; r < row; r++) index += gridRowLengths[r];
		return index + col;
	}

	function rowColFromFlatIndex(flatIndex: number): [number, number] {
		let remaining = flatIndex;
		for (let r = 0; r < gridRowLengths.length; r++) {
			if (remaining < gridRowLengths[r]) return [r, remaining];
			remaining -= gridRowLengths[r];
		}
		return [0, 0];
	}

	function handleGridKeydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement;
		if (target.getAttribute("role") !== "gridcell") return;
		if (!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;

		event.preventDefault();
		event.stopPropagation();

		let newRow = focusedRow;
		let newCol = focusedCol;

		switch (event.key) {
			case "ArrowRight":
				newCol = Math.min(focusedCol + 1, gridRowLengths[focusedRow] - 1);
				break;
			case "ArrowLeft":
				newCol = Math.max(focusedCol - 1, 0);
				break;
			case "ArrowDown":
				newRow = Math.min(focusedRow + 1, gridRowLengths.length - 1);
				newCol = Math.min(focusedCol, gridRowLengths[newRow] - 1);
				break;
			case "ArrowUp":
				newRow = Math.max(focusedRow - 1, 0);
				newCol = Math.min(focusedCol, gridRowLengths[newRow] - 1);
				break;
			case "Home":
				newCol = 0;
				break;
			case "End":
				newCol = gridRowLengths[focusedRow] - 1;
				break;
		}

		if (newRow === focusedRow && newCol === focusedCol) return;

		focusedRow = newRow;
		focusedCol = newCol;

		const grid = event.currentTarget as HTMLElement;
		const cells = grid.querySelectorAll("[role='gridcell']");
		(cells[flatIndexFromRowCol(newRow, newCol)] as HTMLElement)?.focus();
	}

	function handleGridFocusin(event: FocusEvent) {
		const grid = event.currentTarget as HTMLElement;
		const cells = Array.from(grid.querySelectorAll("[role='gridcell']"));
		const index = cells.indexOf(event.target as Element);
		if (index === -1) return;
		[focusedRow, focusedCol] = rowColFromFlatIndex(index);
	}
</script>

{#key device}
	<span id="grid-description" class="sr-only">{$t("device_view.grid_description")}</span>
	<div
		class="flex min-h-0 flex-1 items-center justify-center overflow-auto px-8 py-6"
		class:hidden={$inspectedParentAction || selectedDevice != device.id}
		bind:clientWidth={areaWidth}
		bind:clientHeight={areaHeight}
		role="grid"
		aria-label={device.name}
		aria-describedby="grid-description"
		tabindex="-1"
		on:click={() => inspectedInstance.set(null)}
		on:keyup={(event) => {
			if (event.key === "Escape") inspectedInstance.set(null);
		}}
		on:keydown|capture={handleGridKeydown}
		on:focusin={handleGridFocusin}
	>
		{#if isM18}
			<div class="m18-body relative" style={`--key-size: ${keySize}px; --key-gap: ${keyGap}px;`}>
				<div class="m18-lcd" role="rowgroup">
					{#each { length: M18_LCD_ROWS } as _, r}
						<div class="flex flex-row" style={`gap: ${keyGap}px;`} role="row">
							{#each { length: M18_LCD_COLUMNS } as _, c}
								<Key
									context={{ device: device.id, profile: profile.id, controller: "Keypad", position: r * M18_LCD_COLUMNS + c }}
									bind:inslot={profile.keys[r * M18_LCD_COLUMNS + c]}
									on:dragover={handleDragOver}
									on:drop={(event) => handleDrop(event, "Keypad", r * M18_LCD_COLUMNS + c)}
									on:dragstart={(event) => handleDragStart(event, "Keypad", r * M18_LCD_COLUMNS + c)}
									{handlePaste}
									displaySize={keySize}
									label={`Key ${r * M18_LCD_COLUMNS + c + 1}`}
									tabindex={focusedRow === r && focusedCol === c ? 0 : -1}
								/>
							{/each}
						</div>
					{/each}
				</div>

				<div class="m18-bottom-row flex flex-row justify-center" style={`gap: ${Math.round(keySize * 0.9)}px;`} role="row" aria-label="M18 bottom buttons">
					{#each { length: M18_BOTTOM_KEYS } as _, i}
						<Key
							context={{ device: device.id, profile: profile.id, controller: "Keypad", position: M18_LCD_ROWS * M18_LCD_COLUMNS + i }}
							bind:inslot={profile.keys[M18_LCD_ROWS * M18_LCD_COLUMNS + i]}
							on:dragover={handleDragOver}
							on:drop={(event) => handleDrop(event, "Keypad", M18_LCD_ROWS * M18_LCD_COLUMNS + i)}
							on:dragstart={(event) => handleDragStart(event, "Keypad", M18_LCD_ROWS * M18_LCD_COLUMNS + i)}
							{handlePaste}
							m18Bottom
							displaySize={keySize}
							label={`Button ${i + 1}`}
							tabindex={focusedRow === M18_LCD_ROWS && focusedCol === i ? 0 : -1}
						/>
					{/each}
				</div>
			</div>
		{:else}
			<div class="flex flex-col" style={`gap: ${keyGap}px;`} role="rowgroup">
				{#each { length: device.rows } as _, r}
					<div class="flex flex-row" style={`gap: ${keyGap}px;`} role="row">
						{#each { length: device.columns } as _, c}
							<Key
								context={{ device: device.id, profile: profile.id, controller: "Keypad", position: r * device.columns + c }}
								bind:inslot={profile.keys[r * device.columns + c]}
								on:dragover={handleDragOver}
								on:drop={(event) => handleDrop(event, "Keypad", r * device.columns + c)}
								on:dragstart={(event) => handleDragStart(event, "Keypad", r * device.columns + c)}
								{handlePaste}
								size={device.id.startsWith("sd-") && device.rows == 4 && device.columns == 8 ? 192 : 144}
								displaySize={keySize}
								label="{$t('device_view.key')} {String.fromCharCode(65 + r)}{c + 1}"
								tabindex={focusedRow === r && focusedCol === c ? 0 : -1}
							/>
						{/each}
					</div>
				{/each}

				<div class="flex flex-row justify-between" role="row">
					{#each { length: device.encoders } as _, i}
						<Key
							context={{ device: device.id, profile: profile.id, controller: "Encoder", position: i }}
							bind:inslot={profile.sliders[i]}
							on:dragover={handleDragOver}
							on:drop={(event) => handleDrop(event, "Encoder", i)}
							on:dragstart={(event) => handleDragStart(event, "Encoder", i)}
							{handlePaste}
							displaySize={keySize}
							label="{$t('device_view.encoder')} {i + 1}"
							tabindex={focusedRow === encoderRowIndex && focusedCol === i ? 0 : -1}
						/>
					{/each}
				</div>

				<div class="flex flex-row items-center" style={`gap: ${keyGap}px;`} role="row">
					{#each { length: device.touchpoints } as _, i}
						<!-- On the Stream Deck Neo, the infobar display sits physically between the two touchpoints. -->
						{#if device.infobars > 0 && i === 1}
							{#each { length: device.infobars } as _, j}
								<Key
									context={{ device: device.id, profile: profile.id, controller: "Infobar", position: j }}
									bind:inslot={profile.infobars[j]}
									on:dragover={handleDragOver}
									on:drop={(event) => handleDrop(event, "Infobar", j)}
									on:dragstart={(event) => handleDragStart(event, "Infobar", j)}
									{handlePaste}
									displaySize={keySize}
									width={248}
									height={58}
								/>
							{/each}
						{/if}
						<Key
							context={{ device: device.id, profile: profile.id, controller: "Keypad", position: device.rows * device.columns + i }}
							bind:inslot={profile.keys[device.rows * device.columns + i]}
							on:dragover={handleDragOver}
							on:drop={(event) => handleDrop(event, "Keypad", device.rows * device.columns + i)}
							on:dragstart={(event) => handleDragStart(event, "Keypad", device.rows * device.columns + i)}
							{handlePaste}
							displaySize={keySize}
							isTouchPoint
							label="{$t('device_view.touchpoint')} {i + 1}"
							tabindex={focusedRow === touchpointRowIndex && focusedCol === i ? 0 : -1}
						/>
					{/each}
				</div>
			</div>
		{/if}
	</div>
{/key}

<style>
	/* The M18: a dark machined body with the LCD field set into it. */
	.m18-body {
		padding: calc(var(--key-gap) * 1.6) calc(var(--key-gap) * 1.8) calc(var(--key-gap) * 1.4);
		border-radius: calc(var(--key-size) * 0.42);
		background:
			radial-gradient(120% 90% at 30% 0%, rgb(255 255 255 / 0.07), transparent 55%),
			linear-gradient(180deg, #23262d 0%, #17191e 55%, #111317 100%);
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.12),
			inset 0 -1px 0 rgb(0 0 0 / 0.6),
			0 0 0 1px rgb(0 0 0 / 0.6),
			0 30px 60px -20px rgb(0 0 0 / 0.85),
			0 12px 24px -12px rgb(0 0 0 / 0.7);
	}
	.m18-lcd {
		display: flex;
		flex-direction: column;
		gap: var(--key-gap);
		padding: calc(var(--key-gap) * 0.9);
		border-radius: calc(var(--key-size) * 0.3);
		background: linear-gradient(180deg, #07080a, #0b0c0f);
		box-shadow:
			inset 0 2px 6px rgb(0 0 0 / 0.9),
			inset 0 0 0 1px rgb(0 0 0 / 0.8),
			0 1px 0 rgb(255 255 255 / 0.06);
	}
	.m18-bottom-row {
		margin-top: calc(var(--key-gap) * 1.3);
		padding-top: calc(var(--key-gap) * 0.4);
	}
</style>
