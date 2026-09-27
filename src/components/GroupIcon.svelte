<script lang="ts">
	import AppWindow from "phosphor-svelte/lib/AppWindow";
	import Broadcast from "phosphor-svelte/lib/Broadcast";
	import Desktop from "phosphor-svelte/lib/Desktop";
	import FilmSlate from "phosphor-svelte/lib/FilmSlate";
	import FlowArrow from "phosphor-svelte/lib/FlowArrow";
	import Globe from "phosphor-svelte/lib/Globe";
	import Hourglass from "phosphor-svelte/lib/Hourglass";
	import Keyboard from "phosphor-svelte/lib/Keyboard";
	import Lightbulb from "phosphor-svelte/lib/Lightbulb";
	import PuzzlePiece from "phosphor-svelte/lib/PuzzlePiece";
	import SpeakerHigh from "phosphor-svelte/lib/SpeakerHigh";
	import Stack from "phosphor-svelte/lib/Stack";
	import Sun from "phosphor-svelte/lib/Sun";

	import { libraryGroup } from "$lib/actionLibrary";

	/** Library group (category) name; anything else is a plugin category. */
	export let group: string;
	export let size: number | string = 16;
	export let weight: "regular" | "bold" | "fill" | "duotone" = "fill";

	const icons: Record<string, typeof AppWindow> = { AppWindow, Broadcast, Desktop, FilmSlate, FlowArrow, Globe, Hourglass, Keyboard, Lightbulb, SpeakerHigh, Stack, Sun };

	$: icon = icons[libraryGroup(group)?.icon ?? ""] ?? PuzzlePiece;
</script>

<svelte:component this={icon} {size} {weight} />
