import { useQuery } from "@tanstack/react-query";
import { type Genre, genresApi } from "@/api/genres";
import { type Tag, tagsApi } from "@/api/tags";

/**
 * Reference-data lists (all tags, all genres) shared by filter panels and
 * metadata editors.
 *
 * Both `getAll`s paginate the whole table (≈one request per 500 rows), so a
 * large library makes each fetch several round-trips. These lists change only
 * on an explicit tag/genre edit — which already invalidates the cache — so they
 * are given a long `staleTime`. That stops the expensive multi-page sweep from
 * re-running on every component remount or `refetchOnReconnect` (a flapping SSE
 * connection on a remote deployment otherwise re-fetched the full list every
 * time the socket bounced). Freshness after an edit still works: the metadata
 * editors invalidate `["tags"]` / `["genres"]`, which overrides `staleTime`.
 *
 * Keyed `["tags"]` / `["genres"]` (unchanged) so all consumers and the existing
 * invalidations dedupe to one shared query.
 */
const REFERENCE_DATA_STALE_TIME = 10 * 60 * 1000; // 10 minutes

export function useAllTags(enabled = true) {
  return useQuery<Tag[]>({
    queryKey: ["tags"],
    queryFn: () => tagsApi.getAll(),
    staleTime: REFERENCE_DATA_STALE_TIME,
    enabled,
  });
}

export function useAllGenres(enabled = true) {
  return useQuery<Genre[]>({
    queryKey: ["genres"],
    queryFn: () => genresApi.getAll(),
    staleTime: REFERENCE_DATA_STALE_TIME,
    enabled,
  });
}
