import { useQuery } from "@tanstack/react-query";
import { type PublicSettingsMap, settingsApi } from "@/api/settings";

/**
 * Hook to fetch the server's default scheduler timezone from public settings.
 *
 * Returns the IANA timezone string (e.g., "America/Los_Angeles") configured
 * on the server, or "UTC" as a fallback.
 */
export function useSchedulerTimezone(): string {
  const { data } = useQuery<PublicSettingsMap>({
    queryKey: ["public-settings"],
    queryFn: settingsApi.getPublicSettings,
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
    retry: 1,
    refetchOnWindowFocus: false,
  });

  return data?.["scheduler.timezone"]?.value ?? "UTC";
}
