import { useQuery } from "@tanstack/react-query";
import { releaseSourcesApi } from "@/api/releases";

/**
 * Whether release tracking is available in the user's current scope.
 *
 * Backed by `GET /api/v1/release-sources/applicability`, which returns
 * `applicable: true` when at least one enabled `release_source` plugin
 * applies to `libraryId` (or, with `libraryId` omitted, applies to *any*
 * library — useful for the global navigation Releases entry).
 *
 * Single source of truth for three UI gates:
 *
 * 1. **Per-series Tracking panel + Releases tab**: hide entirely on
 *    libraries with no covering plugin. Avoids dead-end UI like "click to
 *    track this series" on a library that has no plugin to actually do
 *    anything with the tracked state.
 *
 * 2. **Bulk-selection menu Track / Don't track entries**: only show when
 *    at least one selected series's library is covered. Mirrors how
 *    `getActions("series:bulk")` gates other plugin-driven entries.
 *
 * 3. **Top-level "Releases" navigation**: hidden when no plugin is
 *    installed at all (no `libraryId` argument).
 *
 * The query is cheap (one DB hit, no joins) and stale-cached for 5 minutes
 * because the answer only flips when an admin enables/disables a plugin
 * or changes its library scope — both rare operations.
 */
export function useReleaseTrackingApplicability(libraryId?: string) {
  return useQuery({
    queryKey: ["release-tracking-applicability", libraryId ?? null],
    queryFn: () => releaseSourcesApi.applicability(libraryId),
    // Plugin install/disable is rare; treat the answer as essentially static
    // for the life of a normal session. Mutations on the plugin admin page
    // can invalidate this key explicitly if we ever want instant updates.
    staleTime: 5 * 60 * 1000,
  });
}
