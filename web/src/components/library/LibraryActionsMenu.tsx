import { Menu } from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconEdit,
  IconPhoto,
  IconRadar,
  IconRefresh,
  IconScan,
  IconTrash,
  IconTrashX,
  IconWand,
} from "@tabler/icons-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { librariesApi } from "@/api/libraries";
import {
  type PluginActionDto,
  pluginActionsApi,
  pluginsApi,
} from "@/api/plugins";
import { usePermissions } from "@/hooks/usePermissions";
import type { Library } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

interface LibraryActionsMenuProps {
  library: Library;
  onEdit?: () => void;
  onDelete?: () => void;
  onPurge?: () => void;
}

/**
 * Shared menu dropdown content for library actions.
 * Used by both the Sidebar and Library page.
 * Renders only the Menu.Dropdown content (not the Menu wrapper).
 */
export function LibraryActionsMenu({
  library,
  onEdit,
  onDelete,
  onPurge,
}: LibraryActionsMenuProps) {
  // Wrapper to stop event propagation and prevent navigation to library page
  // when triggering actions from the sidebar menu
  const handleMenuAction = <T extends unknown[]>(
    action: (...args: T) => void,
  ) => {
    return (e: React.MouseEvent, ...args: T) => {
      e.stopPropagation();
      e.preventDefault();
      action(...args);
    };
  };
  const { hasPermission } = usePermissions();
  const canEditLibrary = hasPermission(PERMISSIONS.LIBRARIES_WRITE);
  const canDeleteLibrary = hasPermission(PERMISSIONS.LIBRARIES_DELETE);
  const canWriteTasks = hasPermission(PERMISSIONS.TASKS_WRITE);

  // Fetch available plugin actions for library:detail scope
  const { data: pluginActions } = useQuery({
    queryKey: ["plugin-actions", "library:detail"],
    queryFn: () => pluginsApi.getActions("library:detail"),
    staleTime: 5 * 60 * 1000, // Cache for 5 minutes
    enabled: canEditLibrary,
  });

  // Scan mutation
  const scanMutation = useMutation({
    mutationFn: ({ mode }: { mode: "normal" | "deep" }) =>
      librariesApi.scan(library.id, mode),
    onSuccess: (_, variables) => {
      notifications.show({
        title: "Scan started",
        message: `${variables.mode === "deep" ? "Deep" : "Normal"} scan has been initiated`,
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Scan failed",
        message: error.message || "Failed to start scan",
        color: "red",
      });
    },
  });

  // Generate missing thumbnails mutation
  const generateMissingThumbnailsMutation = useMutation({
    mutationFn: () => librariesApi.generateMissingThumbnails(library.id),
    onSuccess: () => {
      notifications.show({
        title: "Thumbnail generation started",
        message: "Missing thumbnails are being generated",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Thumbnail generation failed",
        message: error.message || "Failed to start thumbnail generation",
        color: "red",
      });
    },
  });

  // Regenerate all thumbnails mutation
  const regenerateAllThumbnailsMutation = useMutation({
    mutationFn: () => librariesApi.regenerateAllThumbnails(library.id),
    onSuccess: () => {
      notifications.show({
        title: "Thumbnail regeneration started",
        message: "All book thumbnails are being regenerated",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Thumbnail regeneration failed",
        message: error.message || "Failed to start thumbnail regeneration",
        color: "red",
      });
    },
  });

  // Generate missing series thumbnails mutation
  const generateMissingSeriesThumbnailsMutation = useMutation({
    mutationFn: () => librariesApi.generateMissingSeriesThumbnails(library.id),
    onSuccess: () => {
      notifications.show({
        title: "Series thumbnail generation started",
        message: "Missing series thumbnails are being generated",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Series thumbnail generation failed",
        message: error.message || "Failed to start series thumbnail generation",
        color: "red",
      });
    },
  });

  // Regenerate all series thumbnails mutation
  const regenerateAllSeriesThumbnailsMutation = useMutation({
    mutationFn: () => librariesApi.regenerateAllSeriesThumbnails(library.id),
    onSuccess: () => {
      notifications.show({
        title: "Series thumbnail regeneration started",
        message: "All series thumbnails are being regenerated",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Series thumbnail regeneration failed",
        message:
          error.message || "Failed to start series thumbnail regeneration",
        color: "red",
      });
    },
  });

  // Reprocess series titles mutation
  const reprocessSeriesTitlesMutation = useMutation({
    mutationFn: () => librariesApi.reprocessSeriesTitles(library.id),
    onSuccess: () => {
      notifications.show({
        title: "Reprocessing series titles",
        message: "Series titles will be reprocessed using library rules",
        color: "blue",
      });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Reprocessing failed",
        message: error.message || "Failed to start title reprocessing",
        color: "red",
      });
    },
  });

  // Auto-match mutation
  const autoMatchMutation = useMutation({
    mutationFn: ({ pluginId }: { pluginId: string }) =>
      pluginActionsApi.enqueueLibraryAutoMatchTasks(library.id, pluginId),
    onSuccess: (data) => {
      if (data.success) {
        notifications.show({
          title: "Auto-match started",
          message: data.message,
          color: "blue",
        });
      } else {
        notifications.show({
          title: "Auto-match",
          message: data.message,
          color: "yellow",
        });
      }
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Auto-match failed",
        message: error.message || "Failed to start auto-match",
        color: "red",
      });
    },
  });

  const handleAutoMatch = (plugin: PluginActionDto) => {
    autoMatchMutation.mutate({ pluginId: plugin.pluginId });
  };

  // Filter plugin actions to only show those that apply to this library
  const libraryPluginActions =
    pluginActions?.actions.filter((action) => {
      const libIds = action.libraryIds ?? [];
      return libIds.length === 0 || libIds.includes(library.id);
    }) ?? [];

  return (
    <Menu.Dropdown>
      {canEditLibrary && (
        <>
          <Menu.Item
            leftSection={<IconScan size={16} />}
            onClick={handleMenuAction(() =>
              scanMutation.mutate({ mode: "normal" }),
            )}
          >
            Scan Library
          </Menu.Item>
          <Menu.Item
            leftSection={<IconRadar size={16} />}
            onClick={handleMenuAction(() =>
              scanMutation.mutate({ mode: "deep" }),
            )}
          >
            Scan Library (Deep)
          </Menu.Item>
          <Menu.Divider />
          <Menu.Item
            leftSection={<IconEdit size={16} />}
            onClick={handleMenuAction(() => onEdit?.())}
          >
            Edit Library
          </Menu.Item>
          {canWriteTasks && (
            <>
              <Menu.Divider />
              <Menu.Label>Book Thumbnails</Menu.Label>
              <Menu.Item
                leftSection={<IconPhoto size={16} />}
                onClick={handleMenuAction(() =>
                  generateMissingThumbnailsMutation.mutate(),
                )}
                disabled={generateMissingThumbnailsMutation.isPending}
              >
                Generate Missing
              </Menu.Item>
              <Menu.Item
                leftSection={<IconPhoto size={16} />}
                onClick={handleMenuAction(() =>
                  regenerateAllThumbnailsMutation.mutate(),
                )}
                disabled={regenerateAllThumbnailsMutation.isPending}
              >
                Regenerate All
              </Menu.Item>
              <Menu.Divider />
              <Menu.Label>Series Thumbnails</Menu.Label>
              <Menu.Item
                leftSection={<IconPhoto size={16} />}
                onClick={handleMenuAction(() =>
                  generateMissingSeriesThumbnailsMutation.mutate(),
                )}
                disabled={generateMissingSeriesThumbnailsMutation.isPending}
              >
                Generate Missing
              </Menu.Item>
              <Menu.Item
                leftSection={<IconPhoto size={16} />}
                onClick={handleMenuAction(() =>
                  regenerateAllSeriesThumbnailsMutation.mutate(),
                )}
                disabled={regenerateAllSeriesThumbnailsMutation.isPending}
              >
                Regenerate All
              </Menu.Item>
              <Menu.Divider />
              <Menu.Item
                leftSection={<IconRefresh size={16} />}
                onClick={handleMenuAction(() =>
                  reprocessSeriesTitlesMutation.mutate(),
                )}
                disabled={reprocessSeriesTitlesMutation.isPending}
              >
                Reprocess Series Titles
              </Menu.Item>
            </>
          )}
          {/* Plugin actions for library-wide auto-match */}
          {libraryPluginActions.length > 0 && (
            <>
              <Menu.Divider />
              <Menu.Label>Auto-Apply Metadata</Menu.Label>
              {libraryPluginActions.map((action) => (
                <Menu.Item
                  key={`auto-match-${action.pluginId}`}
                  leftSection={<IconWand size={16} />}
                  onClick={handleMenuAction(() => handleAutoMatch(action))}
                  disabled={autoMatchMutation.isPending}
                >
                  {action.pluginDisplayName}
                </Menu.Item>
              ))}
            </>
          )}
          <Menu.Divider />
          <Menu.Item
            leftSection={<IconTrashX size={16} />}
            color="orange"
            onClick={handleMenuAction(() => onPurge?.())}
          >
            Purge Deleted Books
          </Menu.Item>
        </>
      )}
      {canDeleteLibrary && (
        <Menu.Item
          leftSection={<IconTrash size={16} />}
          color="red"
          onClick={handleMenuAction(() => onDelete?.())}
        >
          Delete Library
        </Menu.Item>
      )}
    </Menu.Dropdown>
  );
}
