import { type ActionCategory, type ActionLocalisations, filterActionCategories, shouldOpenActionCategory } from "../src/lib/actionSearch.ts";
import type { Action } from "../src/lib/Action.ts";

declare const Deno: { test(name: string, callback: () => void): void };

function makeAction(name: string, uuid: string, plugin = "", tooltip = "", visible = true): Action {
	return {
		name,
		uuid,
		plugin,
		tooltip,
		icon: "icon.png",
		visible_in_action_list: visible,
		supported_in_multi_actions: true,
		property_inspector: "",
		controllers: ["Keypad"],
		states: [],
	};
}

function assert(condition: boolean, message: string): void {
	if (!condition) throw new Error(message);
}

const categories: Record<string, ActionCategory> = {
	"Media & Audio": {
		actions: [
			makeAction("Volume Up", "volume.up"),
			makeAction("Siri", "siri", "", "Open the voice assistant"),
			makeAction("Volume Up (VSD Craft)", "vsd.volume.up", "", "", false),
		],
	},
	"Apps & Websites": { actions: [makeAction("Open App", "open.app")] },
	"Coming Soon": { actions: [makeAction("Weather", "weather", "", "Show the weather")] },
	"Zeta Plugin": { actions: [makeAction("Zebra", "zebra", "zeta"), makeAction("Alpha", "alpha", "zeta")] },
	OpenDeck: { actions: [makeAction("Run Command", "opendeck.runcommand", "starterpack")] },
};

const localisations: ActionLocalisations = {
	"": { siri: { Name: "Voice Assistant", Tooltip: "Open the assistant" } },
};

Deno.test("search matches names, localized labels, tooltips, identifiers, and plugin IDs", () => {
	for (const query of ["volume", "voice assistant", "assistant", "siri", "starterpack"]) {
		const results = filterActionCategories(categories, query, localisations);
		assert(results.length > 0, `expected results for ${query}`);
	}
	const siri = filterActionCategories(categories, "siri", localisations);
	assert(siri.length === 1 && siri[0][1].actions.length === 1, "name search should narrow to the Siri action");
});

Deno.test("hidden duplicates never appear, even when searched for", () => {
	const results = filterActionCategories(categories, "vsd craft", localisations);
	assert(results.length === 0, "hidden actions must not be listed");
	const media = filterActionCategories(categories, "", localisations).find(([name]) => name === "Media & Audio");
	assert(media?.[1].actions.length === 2, "the library lists only visible actions");
});

Deno.test("built-in groups keep their curated order, plugins sort by name, Coming Soon is last", () => {
	const names = filterActionCategories(categories, "", localisations).map(([name]) => name);
	assert(names.join(",") === "Apps & Websites,Media & Audio,OpenDeck,Zeta Plugin,Coming Soon", `unexpected group order: ${names.join(",")}`);
	const media = filterActionCategories(categories, "", localisations).find(([name]) => name === "Media & Audio")!;
	assert(media[1].actions.map((action) => action.name).join(",") === "Volume Up,Siri", "curated order is kept");
	const plugin = filterActionCategories(categories, "", localisations).find(([name]) => name === "Zeta Plugin")!;
	assert(plugin[1].actions.map((action) => action.name).join(",") === "Alpha,Zebra", "plugin actions sort by name");
});

Deno.test("Coming Soon can be hidden, but search still finds its actions", () => {
	const hidden = filterActionCategories(categories, "", localisations, { includeComingSoon: false }).map(([name]) => name);
	assert(!hidden.includes("Coming Soon"), "Coming Soon is hidden when requested");
	const searched = filterActionCategories(categories, "weather", localisations, { includeComingSoon: false });
	assert(searched.length === 1 && searched[0][0] === "Coming Soon", "search reveals unfinished actions");
});

Deno.test("search opens matching groups while default groups stay organized", () => {
	const overrides = new Map<string, boolean>([["Media & Audio", false]]);
	assert(shouldOpenActionCategory("Media & Audio", "Siri", overrides, []), "search must reveal results even when a group was collapsed");
	assert(shouldOpenActionCategory("Apps & Websites", "", new Map(), ["Apps & Websites"]), "default groups open");
	assert(!shouldOpenActionCategory("Browser", "", new Map(), ["Apps & Websites"]), "other groups stay collapsed");
	assert(!shouldOpenActionCategory("Media & Audio", "", overrides, ["Media & Audio"]), "the viewer's choice wins over the default");
});
