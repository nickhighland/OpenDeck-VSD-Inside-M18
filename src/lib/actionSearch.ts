import type { Action } from "./Action.ts";
import { COMING_SOON, LIBRARY_GROUPS } from "./actionLibrary.ts";

export type ActionCategory = { icon?: string | null; actions: Action[] };
export type ActionLocalisations = Record<
	string,
	Record<string, { Name?: string; Tooltip?: string } | undefined> | undefined
>;

export function actionName(action: Action, localisations: ActionLocalisations): string {
	return localisations[action.plugin]?.[action.uuid]?.Name ?? action.name;
}

export function actionTooltip(action: Action, localisations: ActionLocalisations): string {
	return localisations[action.plugin]?.[action.uuid]?.Tooltip ?? action.tooltip;
}

function actionMatches(action: Action, query: string, localisations: ActionLocalisations): boolean {
	const translations = localisations[action.plugin]?.[action.uuid];
	return [action.name, translations?.Name, action.tooltip, translations?.Tooltip, action.uuid, action.plugin].some(
		(value) => typeof value === "string" && value.toLocaleLowerCase().includes(query),
	);
}

const GROUP_ORDER: readonly string[] = LIBRARY_GROUPS.map((group) => group.name);

/**
 * Built-in groups in their curated order, then plugin categories
 * alphabetically, then "Coming Soon" last.
 */
function categoryRank(name: string): [number, string] {
	if (name === COMING_SOON) return [2, ""];
	const index = GROUP_ORDER.indexOf(name);
	return index >= 0 ? [0, String(index).padStart(3, "0")] : [1, name.toLocaleLowerCase()];
}

export function compareCategories(left: string, right: string): number {
	const [leftTier, leftKey] = categoryRank(left);
	const [rightTier, rightKey] = categoryRank(right);
	return leftTier - rightTier || leftKey.localeCompare(rightKey);
}

export function isBuiltInGroup(name: string): boolean {
	return GROUP_ORDER.includes(name);
}

/**
 * The categories and actions to show for a search query. Hidden actions
 * (duplicates kept for imported profiles) never appear. Built-in groups keep
 * their curated action order; plugin categories are sorted by name.
 */
export function filterActionCategories(
	categories: Record<string, ActionCategory>,
	query: string,
	localisations: ActionLocalisations,
	options: { includeComingSoon?: boolean } = {},
): [string, ActionCategory][] {
	const normalizedQuery = query.toLocaleLowerCase().trim();
	const includeComingSoon = options.includeComingSoon ?? true;

	return Object.entries(categories)
		.filter(([name]) => includeComingSoon || normalizedQuery !== "" || name !== COMING_SOON)
		.sort(([left], [right]) => compareCategories(left, right))
		.map(([categoryName, category]): [string, ActionCategory] => {
			const listed = category.actions.filter((action) => action.visible_in_action_list !== false);
			const categoryMatches = categoryName.toLocaleLowerCase().includes(normalizedQuery);
			const actions = !normalizedQuery || categoryMatches ? listed : listed.filter((action) => actionMatches(action, normalizedQuery, localisations));
			const ordered = isBuiltInGroup(categoryName)
				? actions
				: [...actions].sort((left, right) => actionName(left, localisations).localeCompare(actionName(right, localisations), undefined, { sensitivity: "base" }));
			return [categoryName, { icon: category.icon, actions: ordered }];
		})
		.filter(([, category]) => category.actions.length > 0);
}

/** Whether a group is expanded: searching opens every match; otherwise the viewer's choice, or the default. */
export function shouldOpenActionCategory(name: string, query: string, overrides: ReadonlyMap<string, boolean>, defaultOpen: readonly string[]): boolean {
	if (query.trim()) return true;
	if (overrides.has(name)) return overrides.get(name) ?? false;
	return defaultOpen.includes(name);
}
