import { useQuery } from "@tanstack/react-query";
import { type BrandingSettingsDto, settingsApi } from "@/api/settings";

/** Default application name used as fallback */
export const DEFAULT_APP_NAME = "Codex";

/**
 * Query key for branding settings
 */
export const brandingQueryKey = ["branding"] as const;

/**
 * Hook to fetch the application name from branding settings.
 *
 * This hook:
 * - Fetches from the unauthenticated `/api/v1/settings/branding` endpoint
 * - Caches the result for 5 minutes (settings don't change often)
 * - Returns "Codex" as fallback if the request fails or is loading
 *
 * @example
 * ```tsx
 * function Header() {
 *   const appName = useAppName();
 *   return <h1>{appName}</h1>;
 * }
 * ```
 */
export function useAppName(): string {
  const { data } = useQuery<BrandingSettingsDto>({
    queryKey: brandingQueryKey,
    queryFn: settingsApi.getBranding,
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
    gcTime: 10 * 60 * 1000, // Keep in cache for 10 minutes
    retry: 1, // Only retry once on failure
    refetchOnWindowFocus: false, // Don't refetch on window focus
  });

  return data?.applicationName ?? DEFAULT_APP_NAME;
}

/**
 * Hook to access branding settings with loading and error states.
 *
 * Use this when you need to handle loading/error states explicitly.
 *
 * @example
 * ```tsx
 * function BrandedComponent() {
 *   const { appName, isLoading, error } = useBranding();
 *
 *   if (isLoading) return <Skeleton />;
 *   if (error) return <Text>Error loading branding</Text>;
 *
 *   return <Text>{appName}</Text>;
 * }
 * ```
 */
export function useBranding() {
  const { data, isLoading, error, isError } = useQuery<BrandingSettingsDto>({
    queryKey: brandingQueryKey,
    queryFn: settingsApi.getBranding,
    staleTime: 5 * 60 * 1000,
    gcTime: 10 * 60 * 1000,
    retry: 1,
    refetchOnWindowFocus: false,
  });

  return {
    appName: data?.applicationName ?? DEFAULT_APP_NAME,
    isLoading,
    error: error as Error | null,
    isError,
  };
}
