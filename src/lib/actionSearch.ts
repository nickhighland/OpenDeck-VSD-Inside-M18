import type { Action } from "./Action.ts";

export type ActionCategory = { icon?: string; actions: Action[] };
export type ActionLocalisations = Record<
  string,
  Record<string, { Name?: string; Tooltip?: string } | undefined> | undefined
>;

export function actionName(
  action: Action,
  localisations: ActionLocalisations,
): string {
  return localisations[action.plugin]?.[action.uuid]?.Name ?? action.name;
}

function actionMatches(
  action: Action,
  query: string,
  localisations: ActionLocalisations,
): boolean {
  const translations = localisations[action.plugin]?.[action.uuid];
  return [
    action.name,
    translations?.Name,
    action.tooltip,
    translations?.Tooltip,
    action.uuid,
    action.plugin,
  ].some(
    (value) =>
      typeof value === "string" && value.toLocaleLowerCase().includes(query),
  );
}

export function filterActionCategories(
  categories: Record<string, ActionCategory>,
  query: string,
  localisations: ActionLocalisations,
  productName: string,
): [string, ActionCategory][] {
  const normalizedQuery = query.toLocaleLowerCase().trim();

  return Object.entries(categories)
    .sort((
      [left],
      [right],
    ) => (left === productName
      ? -1
      : right === productName
      ? 1
      : left.localeCompare(right))
    )
    .map(([categoryName, category]): [string, ActionCategory] => {
      const categoryMatches = categoryName.toLocaleLowerCase().includes(
        normalizedQuery,
      );
      const actions = !normalizedQuery || categoryMatches
        ? category.actions
        : category.actions.filter((action) =>
          actionMatches(action, normalizedQuery, localisations)
        );
      return [categoryName, {
        icon: category.icon,
        actions: [...actions].sort((left, right) =>
          actionName(left, localisations).localeCompare(
            actionName(right, localisations),
            undefined,
            { sensitivity: "base" },
          )
        ),
      }];
    })
    .filter(([, category]) => category.actions.length > 0);
}

export function shouldOpenActionCategory(
  name: string,
  query: string,
  overrides: ReadonlyMap<string, boolean>,
  productName: string,
): boolean {
  if (query.trim()) return true;
  if (overrides.has(name)) return overrides.get(name) ?? false;
  return name === productName || name.startsWith("M18 · ");
}
