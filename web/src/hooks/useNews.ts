import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { newsApi } from "@/api/news";

export const newsKeys = {
  all: ["news"] as const,
};

/** The server fetches the feeds daily, so an hourly refetch is plenty. */
const REFETCH_INTERVAL_MS = 60 * 60 * 1000;

/**
 * News for the current admin. Pass `enabled: false` for non-admins so the
 * request is never made (the endpoint answers 403 to them).
 */
export function useNews(enabled: boolean) {
  return useQuery({
    queryKey: newsKeys.all,
    queryFn: newsApi.get,
    enabled,
    refetchInterval: REFETCH_INTERVAL_MS,
    refetchOnWindowFocus: true,
  });
}

export function useMarkNewsSeen() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: newsApi.markSeen,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: newsKeys.all }),
  });
}
