/**
 * Alias matcher for Nyaa releases.
 *
 * Nyaa identifies series only by name in the torrent title — there's no
 * `nyaa_id` or other stable external ID that ties a release to a specific
 * series in our DB. So matching is a two-step pipeline:
 *
 *   1. Normalize the parsed `seriesGuess` and every alias the host returned
 *      to a common shape (lowercase, alphanumeric + spaces only). This
 *      mirrors the `normalize_alias` function on the host
 *      ([src/db/entities/series_aliases.rs](src/db/entities/series_aliases.rs))
 *      so a release whose normalized title exactly matches one of a series'
 *      stored aliases lands at confidence 0.95.
 *   2. If no exact match, compute a token-level Sørensen-Dice similarity
 *      against every candidate alias. The highest ratio wins, scaled into a
 *      0.7..0.85 confidence band; below the configured threshold we skip.
 *
 * The Dice ratio is more forgiving than edit distance for word-rearranged
 * titles (`"Boruto Two Blue Vortex"` vs. `"Boruto - Two Blue Vortex"`) while
 * still rejecting unrelated series at the threshold. We deliberately don't
 * wire a heavy fuzzy-match library; the surface area is small.
 */

/** A tracked-series candidate with its raw aliases. */
export interface AliasCandidate {
  /** Codex series UUID. */
  seriesId: string;
  /** Raw aliases from `releases/list_tracked`. */
  aliases: string[];
}

/** A successful match. */
export interface AliasMatch {
  seriesId: string;
  confidence: number;
  /** Reason string surfaced in the SeriesMatch — "alias-exact" or "alias-fuzzy". */
  reason: string;
  /** The matched alias (raw form, for logging). */
  matchedAlias: string;
}

/**
 * Confidence assigned on an exact normalized match.
 *
 * Below 1.0 because we still don't have an external ID — a release titled
 * `"X"` could legitimately match multiple series with that alias. The host's
 * threshold treats this as a strong-but-not-certain signal.
 */
export const CONFIDENCE_EXACT = 0.95;

/**
 * Floor below which fuzzy matches don't get emitted. The host's default
 * threshold is 0.7; we share that floor so plugin-side filtering doesn't
 * silently second-guess host config.
 */
export const DEFAULT_FUZZY_FLOOR = 0.7;

/**
 * Anything below this Dice-coefficient is rejected outright (even before the
 * confidence floor kicks in). 0.85 lets through "two-blue-vortex" vs. "two
 * blue vortex" but kills "naruto" vs. "boruto two blue vortex".
 */
export const MIN_DICE_RATIO = 0.85;

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

/**
 * Normalize an alias to the same shape the host stores in
 * `series_aliases.normalized`. Mirrors the Rust `normalize_alias` impl — keep
 * these in lockstep.
 */
export function normalizeAlias(input: string): string {
  let out = "";
  let lastWasSpace = false;
  for (const ch of input) {
    // Match Rust's `is_alphanumeric()` (Unicode-aware).
    if (/[\p{L}\p{N}]/u.test(ch)) {
      out += ch.toLowerCase();
      lastWasSpace = false;
    } else if (/\s/.test(ch) && out.length > 0 && !lastWasSpace) {
      out += " ";
      lastWasSpace = true;
    }
    // Anything else (punctuation, control, symbols) is dropped.
  }
  return out.endsWith(" ") ? out.slice(0, -1) : out;
}

// ---------------------------------------------------------------------------
// Dice coefficient (token-level, character-bigram fallback)
// ---------------------------------------------------------------------------

/**
 * Sørensen-Dice coefficient on word-bigrams of the input strings (with a
 * character-bigram fallback for short / single-word strings).
 *
 * Range: 0..1, where 1.0 means identical bigram sets.
 */
export function diceRatio(a: string, b: string): number {
  if (a.length === 0 || b.length === 0) return 0;
  if (a === b) return 1;

  const bigramsA = bigrams(a);
  const bigramsB = bigrams(b);
  if (bigramsA.size === 0 || bigramsB.size === 0) return 0;

  let intersection = 0;
  for (const bg of bigramsA) {
    if (bigramsB.has(bg)) intersection++;
  }
  return (2 * intersection) / (bigramsA.size + bigramsB.size);
}

function bigrams(s: string): Set<string> {
  const out = new Set<string>();
  // Word bigrams first.
  const words = s.split(/\s+/).filter((w) => w.length > 0);
  if (words.length >= 2) {
    for (let i = 0; i < words.length - 1; i++) {
      out.add(`${words[i]} ${words[i + 1]}`);
    }
  }
  // Plus character bigrams to handle word-rearrangement and short strings.
  const flat = s.replace(/\s+/g, "");
  if (flat.length >= 2) {
    for (let i = 0; i < flat.length - 1; i++) {
      out.add(`#${flat.slice(i, i + 2)}`);
    }
  } else if (flat.length === 1) {
    out.add(`#${flat}`);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Public matching entry point
// ---------------------------------------------------------------------------

export interface MatchOptions {
  /**
   * Minimum confidence for a fuzzy match to be returned. Defaults to
   * `DEFAULT_FUZZY_FLOOR` (0.7). Below this, the matcher returns null.
   */
  fuzzyFloor?: number;
}

/**
 * Match a parsed series-guess against a list of tracked-series candidates and
 * their aliases. Returns the best match or null if nothing clears the floor.
 *
 * On an exact normalized match against any alias of a candidate, confidence
 * is `CONFIDENCE_EXACT` (0.95). If multiple candidates have aliases that
 * normalize to the same form, the first one wins — that's a data-quality
 * issue the host surfaces via the `latest_known_*` advance gate, not
 * something the matcher can untangle alone.
 *
 * On no exact match, the matcher computes Dice ratios across the cartesian
 * product (candidates × aliases), finds the maximum, scales it from
 * `[MIN_DICE_RATIO, 1.0]` into `[fuzzyFloor, 0.85]`, and returns a fuzzy
 * match if the result is at or above the floor.
 */
export function matchSeries(
  seriesGuess: string,
  candidates: AliasCandidate[],
  opts: MatchOptions = {},
): AliasMatch | null {
  const floor = opts.fuzzyFloor ?? DEFAULT_FUZZY_FLOOR;
  const target = normalizeAlias(seriesGuess);
  if (target.length === 0 || candidates.length === 0) return null;

  // Pass 1 — exact normalized match.
  for (const c of candidates) {
    for (const alias of c.aliases) {
      if (normalizeAlias(alias) === target) {
        return {
          seriesId: c.seriesId,
          confidence: CONFIDENCE_EXACT,
          reason: "alias-exact",
          matchedAlias: alias,
        };
      }
    }
  }

  // Pass 2 — best fuzzy match.
  let best: AliasMatch | null = null;
  let bestRatio = 0;
  for (const c of candidates) {
    for (const alias of c.aliases) {
      const ratio = diceRatio(target, normalizeAlias(alias));
      if (ratio > bestRatio) {
        bestRatio = ratio;
        best = {
          seriesId: c.seriesId,
          confidence: 0,
          reason: "alias-fuzzy",
          matchedAlias: alias,
        };
      }
    }
  }
  if (best === null || bestRatio < MIN_DICE_RATIO) return null;

  // Linearly scale [MIN_DICE_RATIO..1.0] → [fuzzyFloor..0.85].
  // (We cap the fuzzy ceiling below CONFIDENCE_EXACT so an alias-exact match
  // is always strictly stronger than the best alias-fuzzy match.)
  const ceiling = 0.85;
  const span = 1 - MIN_DICE_RATIO;
  const t = (bestRatio - MIN_DICE_RATIO) / span; // 0..1 inside the band
  const confidence = floor + t * (ceiling - floor);
  if (confidence < floor) return null;
  best.confidence = Number(confidence.toFixed(4));
  return best;
}

/**
 * Match a list of alias guesses (e.g. from a `Title A / Title B` Nyaa title)
 * and return the best result across them.
 *
 * Picks the highest-confidence match across all guesses, preferring
 * `alias-exact` over `alias-fuzzy` when ties exist (because exact carries a
 * fixed `CONFIDENCE_EXACT` and fuzzy is bounded below it). When two guesses
 * both produce alias-exact matches against different series, the first guess
 * wins — that's the same precedence rule `matchSeries` applies internally
 * across candidates.
 */
export function matchSeriesAny(
  seriesGuesses: string[],
  candidates: AliasCandidate[],
  opts: MatchOptions = {},
): AliasMatch | null {
  if (seriesGuesses.length === 0) return null;
  let best: AliasMatch | null = null;
  for (const guess of seriesGuesses) {
    const m = matchSeries(guess, candidates, opts);
    if (m === null) continue;
    if (best === null || m.confidence > best.confidence) {
      best = m;
    }
  }
  return best;
}
