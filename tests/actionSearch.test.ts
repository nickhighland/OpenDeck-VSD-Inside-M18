import {
  type ActionCategory,
  type ActionLocalisations,
  filterActionCategories,
  shouldOpenActionCategory,
} from "../src/lib/actionSearch.ts";
import type { Action } from "../src/lib/Action.ts";

declare const Deno: { test(name: string, callback: () => void): void };

function makeAction(
  name: string,
  uuid: string,
  plugin = "native",
  tooltip = "",
): Action {
  return {
    name,
    uuid,
    plugin,
    tooltip,
    icon: "icon.png",
    visible_in_action_list: true,
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
  "M18 · System Controls": {
    icon: "system.svg",
    actions: [
      makeAction("Volume up", "volume.up"),
      makeAction("Siri", "siri", "native", "Open the voice assistant"),
    ],
  },
  OpenDeck: {
    actions: [makeAction("Run Command", "opendeck.runcommand", "opendeck")],
  },
};

const localisations: ActionLocalisations = {
  native: { siri: { Name: "Voice Assistant", Tooltip: "Open the assistant" } },
};

Deno.test("action search matches native names, localized labels, tooltips, identifiers, and plugin IDs", () => {
  for (
    const query of ["volume", "voice assistant", "assistant", "siri", "native"]
  ) {
    const results = filterActionCategories(
      categories,
      query,
      localisations,
      "OpenDeck",
    );
    assert(results.length > 0, `expected results for ${query}`);
  }
  assert(
    filterActionCategories(categories, "siri", localisations, "OpenDeck")[0][1]
      .actions.length === 1,
    "name search should narrow to the Siri action",
  );
});

Deno.test("category-name search reveals its actions and results are neatly sorted", () => {
  const results = filterActionCategories(
    categories,
    "system controls",
    localisations,
    "OpenDeck",
  );
  assert(
    results.length === 1,
    "category search should retain only the matching category",
  );
  assert(
    results[0][1].actions.map((action) => action.name).join(",") ===
      "Siri,Volume up",
    "category search should show all actions in sorted order",
  );
  assert(
    filterActionCategories(categories, "", localisations, "OpenDeck")[0][0] ===
      "OpenDeck",
    "product category should sort first",
  );
});

Deno.test("search opens matching action groups while default groups stay organized", () => {
  const overrides = new Map<string, boolean>([[
    "M18 · System Controls",
    false,
  ]]);
  assert(
    shouldOpenActionCategory(
      "M18 · System Controls",
      "Siri",
      overrides,
      "OpenDeck",
    ),
    "search must reveal results even when a group was previously collapsed",
  );
  assert(
    shouldOpenActionCategory("OpenDeck", "", new Map(), "OpenDeck"),
    "the product group opens by default",
  );
  assert(
    !shouldOpenActionCategory("Third-party", "", new Map(), "OpenDeck"),
    "other groups stay collapsed by default",
  );
});
