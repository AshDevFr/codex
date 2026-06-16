import {
  ActionIcon,
  AppShell,
  Box,
  Button,
  Divider,
  Group,
  Menu,
  Modal,
  NavLink,
  Stack,
  Text,
} from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAlertTriangle,
  IconBookmark,
  IconBooks,
  IconBrush,
  IconChartBar,
  IconClipboardList,
  IconCloudDownload,
  IconCopy,
  IconDatabase,
  IconDotsVertical,
  IconFileExport,
  IconFileTypePdf,
  IconHome,
  IconLayoutGrid,
  IconLink,
  IconList,
  IconLogout,
  IconPhoto,
  IconPlugConnected,
  IconPlus,
  IconRadar,
  IconRss,
  IconScan,
  IconServer,
  IconSettings,
  IconShare,
  IconShieldCheck,
  IconSparkles,
  IconTrashX,
  IconUser,
  IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { authApi } from "@/api/auth";
import { librariesApi } from "@/api/libraries";
import { userPluginsApi } from "@/api/userPlugins";
import { LibraryModal } from "@/components/forms/LibraryModal";
import { ReleasesNavBadge } from "@/components/layout/ReleasesNavBadge";
import { LibraryActionsMenu } from "@/components/library/LibraryActionsMenu";
import { TaskNotificationBadge } from "@/components/TaskNotificationBadge";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";
import { useAppInfo } from "@/hooks/useAppInfo";
import { useAppName } from "@/hooks/useAppName";
import { usePermissions } from "@/hooks/usePermissions";
import { useReleaseTrackingApplicability } from "@/hooks/useReleaseTrackingApplicability";
import { useAuthStore } from "@/store/authStore";
import { useLibraryPreferencesStore } from "@/store/libraryPreferencesStore";
import type { Library } from "@/types";
import { PERMISSIONS } from "@/types/permissions";
import classes from "./Sidebar.module.css";

interface SidebarProps {
  /** Called when the user taps a navigation link, so the mobile drawer can auto-close. */
  onNavigate?: () => void;
}

export function Sidebar({ onNavigate }: SidebarProps = {}) {
  const appName = useAppName();
  const { data: appInfo } = useAppInfo();
  const navigate = useNavigate();
  const { pathname: currentPath } = useLocation();
  const queryClient = useQueryClient();
  const { clearAuth } = useAuthStore();
  // Read refreshToken lazily inside handleLogout so we always see the current
  // value, even if the user logged in within this mount.
  // Only subscribe to getLastTab action (doesn't cause re-renders since it's not state)
  const getLastTab = useLibraryPreferencesStore((state) => state.getLastTab);
  const { isAdmin, hasPermission } = usePermissions();
  const canEditLibrary = hasPermission(PERMISSIONS.LIBRARIES_WRITE);
  const canDeleteLibrary = hasPermission(PERMISSIONS.LIBRARIES_DELETE);
  const canWriteTasks = hasPermission(PERMISSIONS.TASKS_WRITE);
  const [addLibraryOpened, setAddLibraryOpened] = useState(false);
  const [editLibraryOpened, setEditLibraryOpened] = useState(false);
  const [selectedLibrary, setSelectedLibrary] = useState<Library | null>(null);
  const [deleteConfirmOpened, setDeleteConfirmOpened] = useState(false);
  const [libraryToDelete, setLibraryToDelete] = useState<Library | null>(null);
  const [purgeConfirmOpened, setPurgeConfirmOpened] = useState(false);
  const [libraryToPurge, setLibraryToPurge] = useState<Library | null>(null);
  const [settingsOpened, setSettingsOpened] = useState(
    currentPath.startsWith("/settings"),
  );

  // Sync settingsOpened state when navigating to/from settings pages
  useEffect(() => {
    if (currentPath.startsWith("/settings")) {
      setSettingsOpened(true);
    }
  }, [currentPath]);

  // U4: Show a bottom fade cue on the mobile drawer when the nav overflows
  // (e.g. Settings is expanded and Users/Sharing Tags sit below the fold).
  // Driven by listening to scroll on the AppShell.Navbar element + a
  // ResizeObserver to catch content height changes when Settings toggles.
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;
  const navSectionRef = useRef<HTMLDivElement>(null);
  const [showScrollCue, setShowScrollCue] = useState(false);

  useEffect(() => {
    if (!isMobile) {
      setShowScrollCue(false);
      return;
    }
    // The scrollable element is the grow section itself (`.navSection`),
    // which scrolls internally so the footer below it stays pinned.
    const section = navSectionRef.current;
    if (!section) return;

    const update = () => {
      const overflowing = section.scrollHeight - section.clientHeight > 4;
      const atBottom =
        section.scrollTop + section.clientHeight >= section.scrollHeight - 4;
      setShowScrollCue(overflowing && !atBottom);
    };

    update();
    section.addEventListener("scroll", update, { passive: true });
    // The ResizeObserver catches content height changes (e.g. Settings
    // expand/collapse) that grow the section without a scroll event.
    const ro = new ResizeObserver(update);
    ro.observe(section);

    return () => {
      section.removeEventListener("scroll", update);
      ro.disconnect();
    };
  }, [isMobile]);

  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });

  const { data: pluginData } = useQuery({
    queryKey: ["user-plugins"],
    queryFn: userPluginsApi.list,
    staleTime: 5 * 60_000,
  });
  const hasRecommendationPlugin = pluginData?.enabled?.some(
    (p) => p.connected && p.capabilities?.userRecommendationProvider === true,
  );
  const { data: releaseApplicability } = useReleaseTrackingApplicability();
  const hasReleasePlugin = releaseApplicability?.applicable === true;

  // Mutations for "All Libraries" actions
  const scanAllMutation = useMutation({
    mutationFn: ({
      libraryId,
      mode,
    }: {
      libraryId: string;
      mode: "normal" | "deep";
    }) => librariesApi.scan(libraryId, mode),
    onSuccess: (_, variables) => {
      notifications.show({
        title: "Scan started",
        message: `${variables.mode === "deep" ? "Deep" : "Normal"} scan has been initiated`,
        color: "blue",
      });
      queryClient.refetchQueries({ queryKey: ["libraries"] });
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Scan failed",
        message: error.message || "Failed to start scan",
        color: "red",
      });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (libraryId: string) => librariesApi.delete(libraryId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Library deleted successfully",
        color: "green",
      });
      queryClient.refetchQueries({ queryKey: ["libraries"] });
      setDeleteConfirmOpened(false);
      setLibraryToDelete(null);
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete library",
        color: "red",
      });
    },
  });

  const purgeMutation = useMutation({
    mutationFn: (libraryId: string) => librariesApi.purgeDeleted(libraryId),
    onSuccess: (count) => {
      notifications.show({
        title: "Success",
        message: `Purged ${count} deleted book${count !== 1 ? "s" : ""} from library`,
        color: "green",
      });
      queryClient.refetchQueries({ queryKey: ["libraries"] });
      setPurgeConfirmOpened(false);
      setLibraryToPurge(null);
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to purge deleted books",
        color: "red",
      });
    },
  });

  // "All Libraries" thumbnail mutations
  const generateMissingThumbnailsAllMutation = useMutation({
    mutationFn: (libraryId: string) =>
      librariesApi.generateMissingThumbnails(libraryId),
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

  const regenerateAllThumbnailsAllMutation = useMutation({
    mutationFn: (libraryId: string) =>
      librariesApi.regenerateAllThumbnails(libraryId),
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

  const generateMissingSeriesThumbnailsAllMutation = useMutation({
    mutationFn: (libraryId: string) =>
      librariesApi.generateMissingSeriesThumbnails(libraryId),
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

  const regenerateAllSeriesThumbnailsAllMutation = useMutation({
    mutationFn: (libraryId: string) =>
      librariesApi.regenerateAllSeriesThumbnails(libraryId),
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

  const handleScanAll = (mode: "normal" | "deep") => {
    if (!libraries) return;
    libraries.forEach((library) => {
      scanAllMutation.mutate({ libraryId: library.id, mode });
    });
  };

  const handleGenerateAllMissingThumbnails = () => {
    if (!libraries) return;
    libraries.forEach((library) => {
      generateMissingThumbnailsAllMutation.mutate(library.id);
    });
  };

  const handleRegenerateAllThumbnails = () => {
    if (!libraries) return;
    libraries.forEach((library) => {
      regenerateAllThumbnailsAllMutation.mutate(library.id);
    });
  };

  const handleGenerateAllMissingSeriesThumbnails = () => {
    if (!libraries) return;
    libraries.forEach((library) => {
      generateMissingSeriesThumbnailsAllMutation.mutate(library.id);
    });
  };

  const handleRegenerateAllSeriesThumbnails = () => {
    if (!libraries) return;
    libraries.forEach((library) => {
      regenerateAllSeriesThumbnailsAllMutation.mutate(library.id);
    });
  };

  const handlePurgeAllDeleted = () => {
    if (!libraries) return;
    libraries.forEach((library) => {
      purgeMutation.mutate(library.id);
    });
  };

  const handleEditLibrary = (library: Library) => {
    setSelectedLibrary(library);
    setEditLibraryOpened(true);
  };

  const handleDeleteLibrary = (library: Library) => {
    setLibraryToDelete(library);
    setDeleteConfirmOpened(true);
  };

  const confirmDelete = () => {
    if (libraryToDelete) {
      deleteMutation.mutate(libraryToDelete.id);
    }
  };

  const handlePurgeDeleted = (library: Library) => {
    setLibraryToPurge(library);
    setPurgeConfirmOpened(true);
  };

  const confirmPurge = () => {
    if (libraryToPurge) {
      purgeMutation.mutate(libraryToPurge.id);
    }
  };

  const handleLogout = async () => {
    const refreshToken = useAuthStore.getState().refreshToken;
    await authApi.logout(refreshToken);
    clearAuth();
    navigate("/login");
    onNavigate?.();
  };

  return (
    <>
      <AppShell.Navbar p="md">
        <AppShell.Section
          grow
          ref={navSectionRef}
          className={classes.navSection}
        >
          <Stack gap="xs" className={classes.navStack}>
            <NavLink
              component={Link}
              to="/"
              label="Home"
              leftSection={<IconHome size={20} stroke={2} />}
              active={currentPath === "/"}
              onClick={onNavigate}
            />
            <NavLink
              component={Link}
              to="/want-to-read"
              label="Want to Read"
              leftSection={<IconBookmark size={20} stroke={2} />}
              active={currentPath === "/want-to-read"}
              onClick={onNavigate}
            />
            <NavLink
              component={Link}
              to="/collections"
              label="Collections"
              leftSection={<IconLayoutGrid size={20} stroke={2} />}
              active={currentPath.startsWith("/collections")}
              onClick={onNavigate}
            />
            <NavLink
              component={Link}
              to="/readlists"
              label="Read Lists"
              leftSection={<IconList size={20} stroke={2} />}
              active={currentPath.startsWith("/readlists")}
              onClick={onNavigate}
            />
            {hasRecommendationPlugin && (
              <NavLink
                component={Link}
                to="/recommendations"
                label="Recommendations"
                leftSection={<IconSparkles size={20} stroke={2} />}
                active={currentPath === "/recommendations"}
                onClick={onNavigate}
              />
            )}
            {hasReleasePlugin && (
              <NavLink
                component={Link}
                to="/releases"
                label="Releases"
                leftSection={<IconRss size={20} stroke={2} />}
                active={currentPath.startsWith("/releases")}
                rightSection={<ReleasesNavBadge />}
                onClick={onNavigate}
              />
            )}
            <NavLink
              component={Link}
              to={`/libraries/all/${getLastTab("all") || "series"}`}
              label="Libraries"
              leftSection={<IconBooks size={20} stroke={2} />}
              active={currentPath.startsWith("/libraries/all")}
              onClick={onNavigate}
              data-section-break="true"
              rightSection={
                canEditLibrary && (
                  <Group gap={4}>
                    <ActionIcon
                      variant="subtle"
                      size="sm"
                      onClick={(e: React.MouseEvent) => {
                        e.preventDefault();
                        e.stopPropagation();
                        setAddLibraryOpened(true);
                      }}
                      title="Add Library"
                    >
                      <IconPlus size={16} />
                    </ActionIcon>
                    <Menu shadow="md" width={200} position="bottom-end">
                      <Menu.Target>
                        <ActionIcon
                          variant="subtle"
                          size="sm"
                          onClick={(e: React.MouseEvent) => {
                            e.preventDefault();
                            e.stopPropagation();
                          }}
                          title="Options"
                        >
                          <IconDotsVertical size={16} />
                        </ActionIcon>
                      </Menu.Target>

                      <Menu.Dropdown>
                        <Menu.Item
                          leftSection={<IconScan size={16} />}
                          onClick={(e: React.MouseEvent) => {
                            e.preventDefault();
                            e.stopPropagation();
                            handleScanAll("normal");
                          }}
                        >
                          Scan All Libraries
                        </Menu.Item>
                        <Menu.Item
                          leftSection={<IconRadar size={16} />}
                          onClick={(e: React.MouseEvent) => {
                            e.preventDefault();
                            e.stopPropagation();
                            handleScanAll("deep");
                          }}
                        >
                          Scan All Libraries (Deep)
                        </Menu.Item>
                        {canWriteTasks && (
                          <>
                            <Menu.Divider />
                            <Menu.Label>Book Thumbnails</Menu.Label>
                            <Menu.Item
                              leftSection={<IconPhoto size={16} />}
                              onClick={(e: React.MouseEvent) => {
                                e.preventDefault();
                                e.stopPropagation();
                                handleGenerateAllMissingThumbnails();
                              }}
                              disabled={
                                generateMissingThumbnailsAllMutation.isPending
                              }
                            >
                              Generate Missing
                            </Menu.Item>
                            <Menu.Item
                              leftSection={<IconPhoto size={16} />}
                              onClick={(e: React.MouseEvent) => {
                                e.preventDefault();
                                e.stopPropagation();
                                handleRegenerateAllThumbnails();
                              }}
                              disabled={
                                regenerateAllThumbnailsAllMutation.isPending
                              }
                            >
                              Regenerate All
                            </Menu.Item>
                            <Menu.Divider />
                            <Menu.Label>Series Thumbnails</Menu.Label>
                            <Menu.Item
                              leftSection={<IconPhoto size={16} />}
                              onClick={(e: React.MouseEvent) => {
                                e.preventDefault();
                                e.stopPropagation();
                                handleGenerateAllMissingSeriesThumbnails();
                              }}
                              disabled={
                                generateMissingSeriesThumbnailsAllMutation.isPending
                              }
                            >
                              Generate Missing
                            </Menu.Item>
                            <Menu.Item
                              leftSection={<IconPhoto size={16} />}
                              onClick={(e: React.MouseEvent) => {
                                e.preventDefault();
                                e.stopPropagation();
                                handleRegenerateAllSeriesThumbnails();
                              }}
                              disabled={
                                regenerateAllSeriesThumbnailsAllMutation.isPending
                              }
                            >
                              Regenerate All
                            </Menu.Item>
                            <Menu.Divider />
                            <Menu.Item
                              leftSection={<IconTrashX size={16} />}
                              color="orange"
                              onClick={(e: React.MouseEvent) => {
                                e.preventDefault();
                                e.stopPropagation();
                                handlePurgeAllDeleted();
                              }}
                            >
                              Purge All Deleted Books
                            </Menu.Item>
                          </>
                        )}
                      </Menu.Dropdown>
                    </Menu>
                  </Group>
                )
              }
            />
            {libraries && libraries.length > 0 ? (
              libraries.map((library) => (
                <NavLink
                  key={library.id}
                  component={Link}
                  to={`/libraries/${library.id}/${getLastTab(library.id) || "recommended"}`}
                  label={library.name}
                  active={currentPath.startsWith(`/libraries/${library.id}/`)}
                  onClick={onNavigate}
                  data-no-accent="true"
                  styles={{
                    root: { paddingLeft: 48 },
                    label: { textTransform: "capitalize" },
                  }}
                  rightSection={
                    (canEditLibrary || canDeleteLibrary) && (
                      <Menu shadow="md" width={200} position="right-start">
                        <Menu.Target>
                          <ActionIcon
                            variant="subtle"
                            size="xs"
                            onClick={(e: React.MouseEvent) => {
                              e.preventDefault();
                              e.stopPropagation();
                            }}
                            title="Library options"
                          >
                            <IconDotsVertical size={14} />
                          </ActionIcon>
                        </Menu.Target>
                        <LibraryActionsMenu
                          library={library}
                          onEdit={() => handleEditLibrary(library)}
                          onDelete={() => handleDeleteLibrary(library)}
                          onPurge={() => handlePurgeDeleted(library)}
                        />
                      </Menu>
                    )
                  }
                />
              ))
            ) : (
              <NavLink
                label="No libraries"
                disabled
                data-no-accent="true"
                styles={{ root: { paddingLeft: 32 } }}
              />
            )}

            <NavLink
              label="Settings"
              leftSection={<IconSettings size={20} stroke={2} />}
              opened={settingsOpened}
              onChange={setSettingsOpened}
              childrenOffset={32}
              active={currentPath.startsWith("/settings")}
              data-section-break="true"
            >
              {isAdmin && (
                <>
                  {/* System Section */}
                  <Divider
                    label="System"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/server"
                    label="Server"
                    leftSection={<IconServer size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/server")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/tasks"
                    label="Tasks"
                    leftSection={<IconClipboardList size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/tasks")}
                    onClick={onNavigate}
                    rightSection={<TaskNotificationBadge variant="compact" />}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/metrics"
                    label="Metrics"
                    leftSection={<IconChartBar size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/metrics")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/plugins"
                    label="Plugins"
                    leftSection={<IconPlugConnected size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/plugins")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/release-tracking"
                    label="Release Tracking"
                    leftSection={<IconRss size={16} stroke={1.5} />}
                    active={currentPath.startsWith(
                      "/settings/release-tracking",
                    )}
                    onClick={onNavigate}
                  />

                  {/* Access Section */}
                  <Divider
                    label="Access"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/users"
                    label="Users"
                    leftSection={<IconUsers size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/users")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/sharing-tags"
                    label="Sharing Tags"
                    leftSection={<IconShare size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/sharing-tags")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/access-groups"
                    label="Access Groups"
                    leftSection={<IconShieldCheck size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/access-groups")}
                    onClick={onNavigate}
                  />

                  {/* Library Health Section */}
                  <Divider
                    label="Library Health"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/duplicates"
                    label="Duplicates"
                    leftSection={<IconCopy size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/duplicates")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/book-errors"
                    label="Book Errors"
                    leftSection={<IconAlertTriangle size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/book-errors")}
                    onClick={onNavigate}
                  />

                  {/* Storage Section */}
                  <Divider
                    label="Storage"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/cleanup"
                    label="Thumbnails"
                    leftSection={<IconBrush size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/cleanup")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/pdf-cache"
                    label="Page Cache"
                    leftSection={<IconFileTypePdf size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/pdf-cache")}
                    onClick={onNavigate}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/plugin-storage"
                    label="Plugin Storage"
                    leftSection={<IconDatabase size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/plugin-storage")}
                    onClick={onNavigate}
                  />

                  {/* Data Export Section */}
                  <Divider
                    label="Data"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                  <NavLink
                    component={Link}
                    to="/settings/exports"
                    label="Data Exports"
                    leftSection={<IconFileExport size={16} stroke={1.5} />}
                    active={currentPath.startsWith("/settings/exports")}
                    onClick={onNavigate}
                  />

                  {/* Account Section */}
                  <Divider
                    label="Account"
                    labelPosition="left"
                    my="xs"
                    styles={{ label: { fontSize: 11, fontWeight: 600 } }}
                  />
                </>
              )}

              <NavLink
                component={Link}
                to="/settings/downloads"
                label="Offline Downloads"
                leftSection={<IconCloudDownload size={16} stroke={1.5} />}
                active={currentPath.startsWith("/settings/downloads")}
                onClick={onNavigate}
              />
              <NavLink
                component={Link}
                to="/settings/integrations"
                label="Integrations"
                leftSection={<IconLink size={16} stroke={1.5} />}
                active={currentPath.startsWith("/settings/integrations")}
                onClick={onNavigate}
              />
              <NavLink
                component={Link}
                to="/settings/profile"
                label="Profile"
                leftSection={<IconUser size={16} stroke={1.5} />}
                active={currentPath.startsWith("/settings/profile")}
                onClick={onNavigate}
              />
            </NavLink>
          </Stack>
        </AppShell.Section>

        {/* U4: bottom fade cue indicating the nav scrolls (mobile only, when
            overflowing and not at the bottom). Sits between the grow section
            and the pinned footer so it visually trails the scrollable area. */}
        {showScrollCue && (
          <Box
            aria-hidden="true"
            data-testid="sidebar-scroll-cue"
            style={{
              position: "sticky",
              bottom: 0,
              height: 24,
              marginTop: -24,
              marginLeft: "calc(var(--mantine-spacing-md) * -1)",
              marginRight: "calc(var(--mantine-spacing-md) * -1)",
              background:
                "linear-gradient(to top, var(--mantine-color-body), transparent)",
              pointerEvents: "none",
              flexShrink: 0,
              zIndex: 1,
            }}
          />
        )}

        <AppShell.Section>
          <footer className={classes.footer} data-testid="sidebar-footer">
            <NavLink
              label="Logout"
              leftSection={<IconLogout size={20} stroke={2} />}
              onClick={handleLogout}
              color="red"
            />
            {appInfo?.version && (
              <Text size="xs" c="dimmed" ta="center" fw={400}>
                v{appInfo.version}
              </Text>
            )}
          </footer>
        </AppShell.Section>
      </AppShell.Navbar>

      <LibraryModal
        opened={addLibraryOpened}
        onClose={(createdLibrary) => {
          setAddLibraryOpened(false);
          // Navigate to the newly created library if one was created
          if (createdLibrary) {
            const lastTab = getLastTab(createdLibrary.id) || "series";
            navigate(`/libraries/${createdLibrary.id}/${lastTab}`);
          }
        }}
      />

      <LibraryModal
        opened={editLibraryOpened}
        onClose={() => {
          setEditLibraryOpened(false);
          setSelectedLibrary(null);
        }}
        library={selectedLibrary}
      />

      <Modal
        opened={deleteConfirmOpened}
        onClose={() => {
          setDeleteConfirmOpened(false);
          setLibraryToDelete(null);
        }}
        title="Delete Library"
        centered
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to delete{" "}
            <strong>{libraryToDelete?.name}</strong>?
          </Text>
          <Text size="sm" c="dimmed">
            This will remove the library from {appName}. The files on disk will
            not be deleted.
          </Text>
          <Group justify="flex-end" mt="md">
            <Button
              variant="subtle"
              onClick={() => {
                setDeleteConfirmOpened(false);
                setLibraryToDelete(null);
              }}
            >
              Cancel
            </Button>
            <Button
              color="red"
              onClick={confirmDelete}
              loading={deleteMutation.isPending}
            >
              Delete Library
            </Button>
          </Group>
        </Stack>
      </Modal>

      <Modal
        opened={purgeConfirmOpened}
        onClose={() => {
          setPurgeConfirmOpened(false);
          setLibraryToPurge(null);
        }}
        title="Purge Deleted Books"
        centered
      >
        <Stack gap="md">
          <Text>
            Are you sure you want to permanently delete all soft-deleted books
            from <strong>{libraryToPurge?.name}</strong>?
          </Text>
          <Text size="sm" c="dimmed">
            This action cannot be undone. All books marked as deleted will be
            permanently removed from the database.
          </Text>
          <Group justify="flex-end" mt="md">
            <Button
              variant="subtle"
              onClick={() => {
                setPurgeConfirmOpened(false);
                setLibraryToPurge(null);
              }}
            >
              Cancel
            </Button>
            <Button
              color="orange"
              onClick={confirmPurge}
              loading={purgeMutation.isPending}
            >
              Purge Deleted Books
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
