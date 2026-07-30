import type { SeriesCondition } from "@/types/filters";

/**
 * Condition fields whose value depends on who is asking.
 *
 * A rule using any of these resolves against the viewer's own data, so the same
 * collection legitimately holds different series for different people. That is
 * the point of a "Favourites" rule, but it surprises anyone comparing notes on a
 * shared server, so the UI labels it.
 */
const PERSONAL_FIELDS = new Set(["userRating", "readStatus", "hasUserRating"]);

/** Group keys, which carry children rather than an operator. */
const GROUP_KEYS = ["allOf", "anyOf"] as const;

/**
 * `true` when any leaf in the rule references the viewer's own ratings or
 * reading progress, at any nesting depth.
 */
export function describesPersonalData(
  condition: SeriesCondition | null | undefined,
): boolean {
  if (!condition || typeof condition !== "object") return false;

  const record = condition as Record<string, unknown>;

  for (const group of GROUP_KEYS) {
    const children = record[group];
    if (Array.isArray(children)) {
      return (children as SeriesCondition[]).some(describesPersonalData);
    }
  }

  return Object.keys(record).some((field) => PERSONAL_FIELDS.has(field));
}

/**
 * Count the leaves in a rule, ignoring group nodes.
 *
 * Used to describe a rule in one line ("3 conditions") where rendering the whole
 * tree would not fit, such as a collection card.
 */
export function countRuleConditions(
  condition: SeriesCondition | null | undefined,
): number {
  if (!condition || typeof condition !== "object") return 0;

  const record = condition as Record<string, unknown>;

  for (const group of GROUP_KEYS) {
    const children = record[group];
    if (Array.isArray(children)) {
      return (children as SeriesCondition[]).reduce(
        (sum, child) => sum + countRuleConditions(child),
        0,
      );
    }
  }

  return Object.keys(record).length > 0 ? 1 : 0;
}
