import { Alert, Anchor, Group, Text } from "@mantine/core";
import { IconPlugConnectedX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { Link } from "react-router-dom";
import { pluginsApi } from "@/api/plugins";
import { useAuthStore } from "@/store/authStore";

// Session storage key for dismissed plugins
const DISMISSED_KEY = "codex:dismissed-plugin-alerts";

/**
 * Get the set of dismissed plugin IDs from session storage.
 */
function getDismissedPluginIds(): Set<string> {
	try {
		const stored = sessionStorage.getItem(DISMISSED_KEY);
		if (stored) {
			return new Set(JSON.parse(stored));
		}
	} catch {
		// Ignore parsing errors
	}
	return new Set();
}

/**
 * Add a plugin ID to the dismissed set in session storage.
 */
function dismissPlugin(pluginId: string): void {
	const dismissed = getDismissedPluginIds();
	dismissed.add(pluginId);
	sessionStorage.setItem(DISMISSED_KEY, JSON.stringify([...dismissed]));
}

/**
 * A global banner that shows when plugins are disabled due to failures.
 * Only visible to admin users. Does not show for manually disabled plugins.
 */
export function PluginStatusBanner() {
	const { user } = useAuthStore();
	const isAdmin = user?.role === "admin";
	const [dismissedIds, setDismissedIds] = useState<Set<string>>(
		getDismissedPluginIds,
	);

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
		for (const plugin of failedPlugins) {
			dismissPlugin(plugin.id);
		}
		setDismissedIds(
			(prev) => new Set([...prev, ...failedPlugins.map((p) => p.id)]),
		);
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

	// Filter out dismissed plugins
	const visiblePlugins = failedPlugins.filter((p) => !dismissedIds.has(p.id));

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
		</Alert>
	);
}
