<script lang="ts">
	import type { Action } from "$lib/Action";
	import type { ActionInstance } from "$lib/ActionInstance";
	import type { Profile } from "$lib/Profile";

	import ArrowLeft from "phosphor-svelte/lib/ArrowLeft";
	import Clock from "phosphor-svelte/lib/Clock";
	import DownloadSimple from "phosphor-svelte/lib/DownloadSimple";
	import Trash from "phosphor-svelte/lib/Trash";
	import Key from "./Key.svelte";

	import { isBuiltIn, isFlowParent } from "$lib/actionLibrary";
	import { actionIndex } from "$lib/catalog";
	import { t } from "$lib/i18n";
	import { actionContextString, findInstance, parseActionContext, removeInstance as removeNestedInstance, copiedItem, inspectedInstance, inspectedParentAction } from "$lib/propertyInspector";
	import { attempt, toast } from "$lib/toast";

	import { invoke } from "@tauri-apps/api/core";

	export let profile: Profile;

	$: parentContext = typeof $inspectedParentAction === "string" ? $inspectedParentAction : actionContextString($inspectedParentAction!);
	$: parsedParent = parseActionContext(parentContext);
	$: parent = findInstance(profile, parentContext)!;
	$: children = parent?.children ?? [];
	$: parentUuid = parent?.action.uuid ?? "";
	$: isMultiAction = parentUuid == "opendeck.multiaction";
	$: parentTitle = isMultiAction ? "Multi Action" : parentUuid == "opendeck.toggleaction" ? "Action Cycle" : "Action Carousel";
	$: parentHelp = isMultiAction
		? "One press runs every step below, top to bottom, with the waits in between."
		: "Each press runs one step and then moves to the next, looping back to the first. The highlighted step runs next.";
	$: position = parsedParent.position;
	$: slotName = position >= 15 ? `Button ${position - 14}` : `Key ${position + 1}`;
	let history: string[] = [];

	function stepName(instance: ActionInstance): string {
		const entry = $actionIndex.get(instance.action.uuid);
		return entry && isBuiltIn(instance.action) ? entry.action.name : instance.action.name;
	}

	let dropping = false;
	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		if (event.dataTransfer?.types.includes("action")) {
			event.dataTransfer.dropEffect = "copy";
			dropping = true;
		}
	}

	export async function addAction(action: Action) {
		if (!action.supported_in_multi_actions && !isFlowParent(action.uuid)) {
			toast("error", `${action.name} can't be a step`, "This action is not supported inside a flow.");
			return;
		}
		const response = await attempt("Could not add the step", () => invoke<ActionInstance | null>("create_child_instance", { parentContext, action }));
		if (response) profile.keys[position] = response;
	}

	async function handleDrop({ dataTransfer }: DragEvent) {
		dropping = false;
		const data = dataTransfer?.getData("action");
		if (data) await addAction(JSON.parse(data));
	}

	async function handlePaste() {
		if (!$copiedItem || $copiedItem.type != "action") return;
		await addAction($copiedItem.action);
	}

	async function removeInstance(index: number) {
		const context = children[index].context;
		const removed = await attempt("Could not remove the step", () => invoke("remove_instance", { context }));
		if (removed === undefined) return;
		removeNestedInstance(profile, context);
		if (index == 0) {
			parent.settings?.delays?.splice(0, 1);
		} else {
			parent.settings?.delays?.splice(index - 1, 1);
		}
		if (isFlowParent(parent.action.uuid) && parent.action.uuid !== "opendeck.multiaction") {
			const remaining = parent.children?.length ?? 0;
			parent.current_state = Math.min(parent.current_state, Math.max(0, remaining - 1));
			parent.states = parent.states.slice(0, Math.max(1, remaining));
		}
		profile = profile;
		if ($inspectedInstance === context) $inspectedInstance = null;
	}

	async function setDelay(index: number, event: Event) {
		const target = event.currentTarget as HTMLInputElement;
		const value = Math.max(0, Math.min(300000, parseInt(target.value) || 0));
		const settings = await attempt("Could not save the wait", () => invoke<any>("set_child_delay", { parentContext, index, delayMs: value }));
		if (settings) {
			parent.settings = settings;
			profile = profile;
		}
	}

	function openChild(instance: ActionInstance) {
		if (isFlowParent(instance.action.uuid)) {
			history = [...history, parentContext];
			$inspectedParentAction = instance.context;
			$inspectedInstance = null;
		} else {
			$inspectedInstance = instance.context;
		}
	}

	function close() {
		const previous = history.pop();
		if (previous) {
			history = history;
			$inspectedParentAction = previous;
		} else {
			$inspectedParentAction = null;
			$inspectedInstance = null;
		}
	}
</script>

<svelte:window
	on:keydown={(event) => {
		if (event.key == "Escape" && !(event.target instanceof HTMLInputElement)) close();
	}}
/>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex shrink-0 items-center gap-3 border-b border-line px-5 py-3">
		<button class="btn btn-sm" on:click={close}><ArrowLeft size="13" weight="bold" /> {history.length ? "Back to flow" : "Back to M18"}</button>
		<div class="min-w-0 flex-1">
			<h1 class="text-[15px] font-semibold text-ink">{parentTitle} <span class="font-normal text-ink-faint">on {slotName}</span></h1>
			<p class="truncate text-xs text-ink-muted">{parentHelp}</p>
		</div>
		<span class="badge">{children.length} {children.length === 1 ? "step" : "steps"}</span>
	</div>

	<div class="min-h-0 flex-1 overflow-auto px-6 py-5" role="list" aria-label="{parentTitle} {$t('parent_action_view.children')}">
		<div class="mx-auto flex max-w-xl flex-col">
			{#each children as instance, index (instance.context)}
				{@const isNext = !isMultiAction && index === Math.min(parent.current_state, children.length - 1)}
				<!-- svelte-ignore a11y-no-noninteractive-tabindex a11y-no-noninteractive-element-interactions a11y-click-events-have-key-events -->
				<div
					class="card group flex items-center gap-3 p-2.5 pr-3 transition-colors hover:border-line-strong"
					class:ring-2={$inspectedInstance === instance.context || (isFlowParent(instance.action.uuid) && $inspectedParentAction === instance.context)}
					class:ring-accent={$inspectedInstance === instance.context}
					class:border-accent={isNext}
					on:click|stopPropagation={() => openChild(instance)}
					on:keydown={(event) => {
						if (event.key == "Enter") openChild(instance);
						else if (event.key == "Delete" || event.key == "Backspace") removeInstance(index);
					}}
					role="listitem"
					tabindex="0"
				>
					<span class="flex size-6 shrink-0 items-center justify-center rounded-md bg-press text-[11px] font-bold text-ink-muted tabular-nums">{index + 1}</span>
					<div class="pointer-events-none shrink-0">
						<Key inslot={instance} context={null} active={false} displaySize={46} role="presentation" tabindex={-1} label={`${parentTitle} step ${index + 1}`} />
					</div>
					<div class="min-w-0 flex-1">
						<p class="truncate text-[13px] font-medium text-ink">{stepName(instance)}</p>
						<p class="truncate text-[11.5px] text-ink-faint">{isFlowParent(instance.action.uuid) ? "Open nested steps" : isNext ? "Runs on the next press" : $actionIndex.get(instance.action.uuid)?.action.tooltip ?? instance.action.tooltip}</p>
					</div>
					<button class="btn btn-ghost btn-icon btn-sm opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100" on:click|stopPropagation={() => removeInstance(index)} aria-label={$t("parent_action_view.remove", { name: stepName(instance) })}>
						<Trash size="14" />
					</button>
				</div>

				{#if isMultiAction && index < children.length - 1}
					<div class="flex items-center gap-2 py-1.5 pl-5">
						<span class="h-5 w-px bg-line-strong"></span>
						<label class="flex items-center gap-1.5 rounded-full border border-line bg-raised py-0.5 pr-2 pl-2 text-[11px] text-ink-muted">
							<Clock size="12" />
							wait
							<input
								type="number"
								min="0"
								max="300000"
								step="50"
								value={parent.settings?.delays?.[index] ?? 100}
								on:change={(event) => setDelay(index, event)}
								class="w-14 bg-transparent text-center text-[11px] text-ink tabular-nums outline-none"
								aria-label={$t("parent_action_view.delay.aria", { name: stepName(children[index + 1]) })}
							/>
							ms
						</label>
					</div>
				{:else if index < children.length - 1}
					<div class="py-1 pl-8"><span class="block h-3 w-px bg-line"></span></div>
				{/if}
			{/each}

			<!-- svelte-ignore a11y-no-noninteractive-tabindex a11y-no-noninteractive-element-interactions -->
			<div
				class="mt-4 flex flex-col items-center gap-2 rounded-xl border-2 border-dashed px-6 py-7 text-center transition-colors"
				class:border-accent={dropping}
				class:bg-accent-soft={dropping}
				class:border-line-strong={!dropping}
				on:dragover={handleDragOver}
				on:dragleave={() => (dropping = false)}
				on:drop={handleDrop}
				on:keydown={(e) => {
					if ((e.ctrlKey || e.metaKey) && e.key == "v") handlePaste();
				}}
				role="listitem"
				tabindex="0"
				aria-label={$t("parent_action_view.drag_copy")}
			>
				<DownloadSimple size="22" class="text-ink-faint" />
				<p class="text-[13px] font-medium text-ink-muted">{children.length ? "Add another step" : "Add the first step"}</p>
				<p class="hint">Drag an action here, or double-click one in the library.</p>
			</div>
		</div>
	</div>
</div>
