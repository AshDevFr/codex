/**
 * Human-readable descriptions of what each release action does to a ledger
 * row. Shared by the bulk action bar and the per-row icon buttons so the two
 * can't drift apart.
 *
 * Each description states its own permanence. Ingestion dedups on
 * `(source_id, external_release_id)` and returns before touching an existing
 * row, so the first three states survive every future poll; Reset is the
 * only undo. Delete is the exception and says so, since it clears the
 * source's etag and lets the release be re-announced.
 *
 * Dismiss and Ignore are the same state change server-side; the wording is
 * the only thing telling the user which bucket to reach for, so it carries
 * the reason ("this release" vs "this content") rather than the mechanism.
 */
export const RELEASE_ACTION_DESCRIPTIONS = {
  markAcquired:
    "You got this release. Moves it out of New into Acquired, permanently: it won't return on the next poll. Undo with Reset.",
  dismiss:
    "You don't want this particular release (wrong group, language, or quality). Moves it out of New into Dismissed, permanently: it won't return on the next poll. Undo with Reset.",
  ignore:
    "You already own this chapter or volume, so no release of it interests you. Moves it out of New into Ignored, permanently: it won't return on the next poll. Undo with Reset.",
  reset:
    "The undo. Puts the release back into New from Acquired, Dismissed, or Ignored.",
  delete:
    "Erases the release from the ledger and clears the source's cache. Unlike the other actions this is not permanent: the release returns on the next poll if the source still lists it.",
} as const;
