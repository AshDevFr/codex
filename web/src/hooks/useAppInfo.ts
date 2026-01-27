import { useQuery } from "@tanstack/react-query";
import { type AppInfoDto, infoApi } from "@/api/info";

/**
 * Query key for app info
 */
export const appInfoQueryKey = ["app-info"] as const;

/**
 * Hook to fetch application info (version and name).
 *
 * This hook:
 * - Fetches from the unauthenticated `/api/v1/info` endpoint
 * - Caches the result indefinitely (version never changes during session)
 * - Returns undefined while loading
 *
 * @example
 * ```tsx
 * function Footer() {
 *   const { data } = useAppInfo();
 *   return <Text>v{data?.version}</Text>;
 * }
 * ```
 */
export function useAppInfo() {
	return useQuery<AppInfoDto>({
		queryKey: appInfoQueryKey,
		queryFn: infoApi.getInfo,
		staleTime: Number.POSITIVE_INFINITY, // Version never changes during session
		gcTime: Number.POSITIVE_INFINITY, // Keep in cache forever
		retry: 1, // Only retry once on failure
		refetchOnWindowFocus: false, // Don't refetch on window focus
	});
}

/**
 * Hook to get just the application version.
 *
 * @example
 * ```tsx
 * function VersionBadge() {
 *   const version = useAppVersion();
 *   if (!version) return null;
 *   return <Badge>v{version}</Badge>;
 * }
 * ```
 */
export function useAppVersion(): string | undefined {
	const { data } = useAppInfo();
	return data?.version;
}
