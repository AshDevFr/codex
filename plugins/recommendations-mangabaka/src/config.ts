/**
 * Parsing of host-supplied configuration into validated settings.
 *
 * Every value here is user input arriving as loose JSON, so nothing is trusted:
 * types are checked, numbers are clamped, and values outside an upstream enum
 * are dropped rather than forwarded. That last part matters more than it looks.
 * MangaBaka rejects the *entire request* on an unrecognised enum value, so one
 * typo in a filter would cost the user every recommendation rather than one
 * filter.
 */

import { DEFAULT_COLLABORATIVE_SEEDS } from "./collaborative.js";
import { logger } from "./logger.js";
import { DEFAULT_CONTENT_WEIGHT } from "./scoring.js";
import type { MbContentRating, MbSeriesType } from "./types.js";

/** Upper bound on the collaborative fan-out, whatever the user asks for. */
const MAX_COLLABORATIVE_SEEDS = 10;

const CONTENT_RATINGS: readonly string[] = ["safe", "suggestive", "erotica", "pornographic"];
const SERIES_TYPES: readonly string[] = ["manga", "novel", "manhwa", "manhua", "oel", "other"];

/** Per-user recommendation settings, validated. */
export interface UserSettings {
  /** Allowed content ratings. Empty means no rating filter. */
  contentRating: MbContentRating[];
  /** Restrict results to these series types. */
  includedTypes: MbSeriesType[];
  /** Exclude these series types. */
  excludedTypes: MbSeriesType[];
  /** Exclude these genres. */
  excludedGenres: string[];
  /** Tag names to exclude, still to be resolved to IDs. */
  excludedTagNames: string[];
  /** Minimum upstream rating, 0-100. Undefined means no filter. */
  minimumRating?: number;
  /** Share of the blended score from content similarity. */
  contentWeight: number;
  /** How many top-rated seeds to query for the collaborative signal. */
  collaborativeSeeds: number;
  /** Drop rather than de-rank results sharing an author with a seed. */
  excludeSameAuthor: boolean;
}

/** Admin-level client options. */
export interface AdminSettings {
  timeout?: number;
  baseUrl?: string;
}

type LooseConfig = Record<string, unknown> | undefined | null;

/**
 * Split a comma-separated setting into trimmed values.
 *
 * The user-plugin settings UI renders anything that is not a boolean, number,
 * or JSON field as a plain text input, so multi-value settings have to be
 * comma-separated strings. This matches the convention the AniList provider
 * already uses.
 */
function parseList(value: unknown): string[] {
  if (typeof value !== "string") return [];
  return value
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

/**
 * Split a list and keep only values the upstream enum accepts.
 *
 * Anything else is dropped with a warning, because forwarding it would make
 * upstream reject the whole request.
 */
function parseEnumList<T extends string>(
  value: unknown,
  allowed: readonly string[],
  field: string,
): T[] {
  const kept: T[] = [];

  for (const item of parseList(value)) {
    const normalized = item.toLowerCase();
    if (allowed.includes(normalized)) {
      kept.push(normalized as T);
    } else {
      logger.warn(`Ignoring unrecognised ${field} value "${item}"`);
    }
  }

  return kept;
}

/** Read a number, clamped into range, or undefined when unusable. */
function parseNumber(value: unknown, min: number, max: number): number | undefined {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  return Math.max(min, Math.min(value, max));
}

/** Parse the per-user settings block. */
export function parseUserConfig(config: LooseConfig): UserSettings {
  const source = config ?? {};

  const minimumRating = parseNumber(source.minimumRating, 0, 100);
  const contentWeight = parseNumber(source.contentWeight, 0, 1);
  const collaborativeSeeds = parseNumber(source.collaborativeSeeds, 0, MAX_COLLABORATIVE_SEEDS);

  return {
    contentRating: parseEnumList<MbContentRating>(
      source.contentRating,
      CONTENT_RATINGS,
      "content rating",
    ),
    includedTypes: parseEnumList<MbSeriesType>(source.includedTypes, SERIES_TYPES, "series type"),
    excludedTypes: parseEnumList<MbSeriesType>(source.excludedTypes, SERIES_TYPES, "series type"),
    excludedGenres: parseList(source.excludedGenres).map((genre) => genre.toLowerCase()),
    // Left as typed, since tag resolution is case-insensitive but needs the
    // words themselves.
    excludedTagNames: parseList(source.excludedTags),
    // Zero means "no minimum", which is how the field is documented, so it maps
    // to no filter rather than a filter at zero.
    minimumRating: minimumRating && minimumRating > 0 ? minimumRating : undefined,
    contentWeight: contentWeight ?? DEFAULT_CONTENT_WEIGHT,
    collaborativeSeeds:
      collaborativeSeeds === undefined
        ? DEFAULT_COLLABORATIVE_SEEDS
        : Math.round(collaborativeSeeds),
    excludeSameAuthor: source.excludeSameAuthor === true,
  };
}

/** Parse the admin settings block. */
export function parseAdminConfig(config: LooseConfig): AdminSettings {
  const source = config ?? {};
  const settings: AdminSettings = {};

  // Not clamped like the tuning values: a zero or negative timeout is
  // meaningless rather than merely out of range, and clamping it up to the
  // minimum would give every request a one-second deadline. Fall back to the
  // client default instead.
  if (typeof source.timeout === "number" && Number.isFinite(source.timeout) && source.timeout > 0) {
    settings.timeout = Math.min(source.timeout, 300);
  }

  if (typeof source.base_url === "string" && source.base_url.trim().length > 0) {
    settings.baseUrl = source.base_url.trim();
  }

  return settings;
}

/** Summarise the active filters for the log, so a thin result set is explicable. */
export function describeSettings(settings: UserSettings): string {
  const parts: string[] = [];

  if (settings.contentRating.length > 0) parts.push(`rating=[${settings.contentRating}]`);
  if (settings.includedTypes.length > 0) parts.push(`types=[${settings.includedTypes}]`);
  if (settings.excludedTypes.length > 0) parts.push(`notTypes=[${settings.excludedTypes}]`);
  if (settings.excludedGenres.length > 0) parts.push(`notGenres=[${settings.excludedGenres}]`);
  if (settings.excludedTagNames.length > 0) parts.push(`notTags=[${settings.excludedTagNames}]`);
  if (settings.minimumRating !== undefined) parts.push(`minRating=${settings.minimumRating}`);
  if (settings.excludeSameAuthor) parts.push("excludeSameAuthor");

  parts.push(`contentWeight=${settings.contentWeight}`);
  parts.push(`collaborativeSeeds=${settings.collaborativeSeeds}`);

  return parts.join(", ");
}
