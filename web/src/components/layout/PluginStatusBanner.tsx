import { Alert, Anchor, Group, Stack, Text } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { IconPlugConnectedX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { pluginsApi } from "@/api/plugins";
import { MOBILE_MEDIA_QUERY } from "@/components/ui/ResponsiveTable";
import { useAuthStore } from "@/store/authStore";

// Local storage key for dismissed plugins. We map plugin ID -> failureCount
// at the moment of dismissal. The banner re-appears whenever the plugin's
// current failureCount exceeds the stored value, which corresponds to a new
// failure since the user last dismissed it. Persisting across reloads
// (rather than sessionStorage) is intentional: on a phone the banner eats
// ~75px of above-the-fold space, so reload-survival matters. (U5)
const DISMISSED_KEY = "codex:dismissed-plugin-alerts";

type DismissedMap = Record<string, number>;

/**
 * Get the dismissed map (plugin id -> failureCount at dismissal time) from
 * localStorage. Returns an empty map on parse / storage errors.
 */
function getDismissedMap(): DismissedMap {
  try {
    const stored = localStorage.getItem(DISMISSED_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return parsed as DismissedMap;
      }
    }
  } catch {
    // Ignore parsing errors
  }
  return {};
}

/**
 * Persist the dismissal map back to localStorage. No-op on quota / private-
 * mode failures.
 */
function saveDismissedMap(map: DismissedMap): void {
  try {
    localStorage.setItem(DISMISSED_KEY, JSON.stringify(map));
  } catch {
    // Ignore storage errors
  }
}

/**
 * A global banner that shows when plugins are disabled due to failures.
 * Only visible to admin users. Does not show for manually disabled plugins.
 */
export function PluginStatusBanner() {
  const { user } = useAuthStore();
  const isAdmin = user?.role === "admin";
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;
  const [dismissedMap, setDismissedMap] =
    useState<DismissedMap>(getDismissedMap);

  const { data: pluginsResponse } = useQuery({
    queryKey: ["plugins"],
    queryFn: pluginsApi.getAll,
    // Only fetch if user is admin
    enabled: isAdmin,
    // Don't refetch too aggressively for this status check
    refetchInterval: 60000, // 1 minute
    staleTime: 30000, // 30 seconds
  });

  const handleDismissAll = useCallback(() => {
    if (!pluginsResponse) return;
    const failedPlugins = pluginsResponse.plugins.filter(
      (p) =>
        p.disabledReason ||
        (p.healthStatus === "unhealthy" && p.failureCount > 0),
    );
    setDismissedMap((prev) => {
      const next = { ...prev };
      for (const plugin of failedPlugins) {
        next[plugin.id] = plugin.failureCount;
      }
      saveDismissedMap(next);
      return next;
    });
  }, [pluginsResponse]);

  // Don't show for non-admins or if no data
  if (!isAdmin || !pluginsResponse) {
    return null;
  }

  // Find plugins that are disabled due to failures (have disabledReason set)
  // or are unhealthy with failures. This excludes manually disabled plugins.
  const failedPlugins = pluginsResponse.plugins.filter(
    (p) =>
      // Plugin was auto-disabled due to failures (has a reason)
      p.disabledReason ||
      // Plugin is unhealthy and has failure count (but not manually disabled)
      (p.healthStatus === "unhealthy" && p.failureCount > 0 && p.enabled),
  );

  // Filter out plugins the user has dismissed at the current failureCount.
  // If a *new* failure has happened since dismissal (current failureCount >
  // stored), the banner returns; that's the desired behavior per U5.
  const visiblePlugins = failedPlugins.filter((p) => {
    const dismissedAt = dismissedMap[p.id];
    if (dismissedAt === undefined) return true;
    return p.failureCount > dismissedAt;
  });

  if (visiblePlugins.length === 0) {
    return null;
  }

  const pluginNames = visiblePlugins
    .map((p) => p.displayName)
    .slice(0, 3)
    .join(", ");
  const moreCount = visiblePlugins.length - 3;

  return (
    <Alert
      icon={<IconPlugConnectedX size={16} />}
      color="red"
      variant="light"
      radius={0}
      style={{ borderBottom: "1px solid var(--mantine-color-red-3)" }}
      withCloseButton
      onClose={handleDismissAll}
      closeButtonLabel="Dismiss all"
    >
      {isMobile ? (
        <Stack gap={4} align="flex-start">
          <Text size="sm">
            {visiblePlugins.length === 1
              ? `Plugin "${pluginNames}" is disabled due to failures.`
              : `${visiblePlugins.length} plugins are having issues: ${pluginNames}${moreCount > 0 ? ` and ${moreCount} more` : ""}.`}
          </Text>
          <Anchor component={Link} to="/settings/plugins" size="sm">
            View Plugins
          </Anchor>
        </Stack>
      ) : (
        <Group justify="space-between" wrap="nowrap">
          <Text size="sm">
            {visiblePlugins.length === 1
              ? `Plugin "${pluginNames}" is disabled due to failures.`
              : `${visiblePlugins.length} plugins are having issues: ${pluginNames}${moreCount > 0 ? ` and ${moreCount} more` : ""}.`}
          </Text>
          <Anchor component={Link} to="/settings/plugins" size="sm">
            View Plugins
          </Anchor>
        </Group>
      )}
    </Alert>
  );
}
