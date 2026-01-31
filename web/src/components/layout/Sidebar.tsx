import {
	ActionIcon,
	AppShell,
	Button,
	Divider,
	Group,
	Menu,
	Modal,
	NavLink,
	Stack,
	Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAlertTriangle,
	IconBooks,
	IconBrush,
	IconChartBar,
	IconClipboardList,
	IconCopy,
	IconDotsVertical,
	IconEdit,
	IconFileTypePdf,
	IconHome,
	IconLogout,
	IconPhoto,
	IconPlugConnected,
	IconPlus,
	IconRadar,
	IconScan,
	IconServer,
	IconSettings,
	IconShare,
	IconTrash,
	IconTrashX,
	IconUser,
	IconUsers,
	IconWand,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import {
	type PluginActionDto,
	pluginActionsApi,
	pluginsApi,
} from "@/api/plugins";
import { LibraryModal } from "@/components/forms/LibraryModal";
import { TaskNotificationBadge } from "@/components/TaskNotificationBadge";
import { useAppInfo } from "@/hooks/useAppInfo";
import { useAppName } from "@/hooks/useAppName";
import { usePermissions } from "@/hooks/usePermissions";
import { useAuthStore } from "@/store/authStore";
import { useLibraryPreferencesStore } from "@/store/libraryPreferencesStore";
import type { Library } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

interface SidebarProps {
	currentPath?: string;
}

export function Sidebar({ currentPath = "/" }: SidebarProps) {
	const appName = useAppName();
	const { data: appInfo } = useAppInfo();
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { clearAuth } = useAuthStore();
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

	const { data: libraries } = useQuery({
		queryKey: ["libraries"],
		queryFn: librariesApi.getAll,
	});

	// Fetch available plugin actions for library:detail scope
	const { data: pluginActions } = useQuery({
		queryKey: ["plugin-actions", "library:detail"],
		queryFn: () => pluginsApi.getActions("library:detail"),
		staleTime: 5 * 60 * 1000, // Cache for 5 minutes
		enabled: canEditLibrary, // Only fetch if user can edit libraries
	});

	const scanMutation = useMutation({
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
			// Use refetchQueries to force immediate refetch, bypassing staleTime
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
			// Use refetchQueries to force immediate refetch, bypassing staleTime
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
			// Use refetchQueries to force immediate refetch, bypassing staleTime
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

	// Auto-match mutation for library-wide metadata matching
	const autoMatchMutation = useMutation({
		mutationFn: ({
			libraryId,
			pluginId,
		}: {
			libraryId: string;
			pluginId: string;
		}) => pluginActionsApi.enqueueLibraryAutoMatchTasks(libraryId, pluginId),
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

	// Generate missing thumbnails mutation
	const generateMissingThumbnailsMutation = useMutation({
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

	// Regenerate all thumbnails mutation (force)
	const regenerateAllThumbnailsMutation = useMutation({
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

	// Generate missing series thumbnails mutation
	const generateMissingSeriesThumbnailsMutation = useMutation({
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

	// Regenerate all series thumbnails mutation (force)
	const regenerateAllSeriesThumbnailsMutation = useMutation({
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
			scanMutation.mutate({ libraryId: library.id, mode });
		});
	};

	const handleGenerateAllMissingThumbnails = () => {
		if (!libraries) return;

		libraries.forEach((library) => {
			generateMissingThumbnailsMutation.mutate(library.id);
		});
	};

	const handleRegenerateAllThumbnails = () => {
		if (!libraries) return;

		libraries.forEach((library) => {
			regenerateAllThumbnailsMutation.mutate(library.id);
		});
	};

	const handleGenerateAllMissingSeriesThumbnails = () => {
		if (!libraries) return;

		libraries.forEach((library) => {
			generateMissingSeriesThumbnailsMutation.mutate(library.id);
		});
	};

	const handleRegenerateAllSeriesThumbnails = () => {
		if (!libraries) return;

		libraries.forEach((library) => {
			regenerateAllSeriesThumbnailsMutation.mutate(library.id);
		});
	};

	const handlePurgeAllDeleted = () => {
		if (!libraries) return;

		libraries.forEach((library) => {
			purgeMutation.mutate(library.id);
		});
	};

	// Handler for library auto-match action
	const handleLibraryAutoMatch = (
		library: Library,
		plugin: PluginActionDto,
	) => {
		autoMatchMutation.mutate({
			libraryId: library.id,
			pluginId: plugin.pluginId,
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

	const handleLogout = () => {
		clearAuth();
		navigate("/login");
	};

	return (
		<>
			<AppShell.Navbar p="md">
				<AppShell.Section grow>
					<Stack gap="xs">
						<NavLink
							component={Link}
							to="/"
							label="Home"
							leftSection={<IconHome size={20} />}
							active={currentPath === "/"}
						/>
						<NavLink
							component={Link}
							to={`/libraries/all/${getLastTab("all") || "series"}`}
							label="Libraries"
							leftSection={<IconBooks size={20} />}
							active={currentPath.startsWith("/libraries/all")}
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
													onClick={() => handleScanAll("normal")}
												>
													Scan All Libraries
												</Menu.Item>
												<Menu.Item
													leftSection={<IconRadar size={16} />}
													onClick={() => handleScanAll("deep")}
												>
													Scan All Libraries (Deep)
												</Menu.Item>
												{canWriteTasks && (
													<>
														<Menu.Divider />
														<Menu.Label>Book Thumbnails</Menu.Label>
														<Menu.Item
															leftSection={<IconPhoto size={16} />}
															onClick={handleGenerateAllMissingThumbnails}
															disabled={
																generateMissingThumbnailsMutation.isPending
															}
														>
															Generate Missing
														</Menu.Item>
														<Menu.Item
															leftSection={<IconPhoto size={16} />}
															onClick={handleRegenerateAllThumbnails}
															disabled={
																regenerateAllThumbnailsMutation.isPending
															}
														>
															Regenerate All
														</Menu.Item>
														<Menu.Divider />
														<Menu.Label>Series Thumbnails</Menu.Label>
														<Menu.Item
															leftSection={<IconPhoto size={16} />}
															onClick={handleGenerateAllMissingSeriesThumbnails}
															disabled={
																generateMissingSeriesThumbnailsMutation.isPending
															}
														>
															Generate Missing
														</Menu.Item>
														<Menu.Item
															leftSection={<IconPhoto size={16} />}
															onClick={handleRegenerateAllSeriesThumbnails}
															disabled={
																regenerateAllSeriesThumbnailsMutation.isPending
															}
														>
															Regenerate All
														</Menu.Item>
														<Menu.Divider />
														<Menu.Item
															leftSection={<IconTrashX size={16} />}
															color="orange"
															onClick={handlePurgeAllDeleted}
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

												<Menu.Dropdown>
													{canEditLibrary && (
														<>
															<Menu.Item
																leftSection={<IconScan size={16} />}
																onClick={() =>
																	scanMutation.mutate({
																		libraryId: library.id,
																		mode: "normal",
																	})
																}
															>
																Scan Library
															</Menu.Item>
															<Menu.Item
																leftSection={<IconRadar size={16} />}
																onClick={() =>
																	scanMutation.mutate({
																		libraryId: library.id,
																		mode: "deep",
																	})
																}
															>
																Scan Library (Deep)
															</Menu.Item>
															<Menu.Divider />
															<Menu.Item
																leftSection={<IconEdit size={16} />}
																onClick={() => handleEditLibrary(library)}
															>
																Edit Library
															</Menu.Item>
															{canWriteTasks && (
																<>
																	<Menu.Divider />
																	<Menu.Label>Book Thumbnails</Menu.Label>
																	<Menu.Item
																		leftSection={<IconPhoto size={16} />}
																		onClick={() =>
																			generateMissingThumbnailsMutation.mutate(
																				library.id,
																			)
																		}
																		disabled={
																			generateMissingThumbnailsMutation.isPending
																		}
																	>
																		Generate Missing
																	</Menu.Item>
																	<Menu.Item
																		leftSection={<IconPhoto size={16} />}
																		onClick={() =>
																			regenerateAllThumbnailsMutation.mutate(
																				library.id,
																			)
																		}
																		disabled={
																			regenerateAllThumbnailsMutation.isPending
																		}
																	>
																		Regenerate All
																	</Menu.Item>
																	<Menu.Divider />
																	<Menu.Label>Series Thumbnails</Menu.Label>
																	<Menu.Item
																		leftSection={<IconPhoto size={16} />}
																		onClick={() =>
																			generateMissingSeriesThumbnailsMutation.mutate(
																				library.id,
																			)
																		}
																		disabled={
																			generateMissingSeriesThumbnailsMutation.isPending
																		}
																	>
																		Generate Missing
																	</Menu.Item>
																	<Menu.Item
																		leftSection={<IconPhoto size={16} />}
																		onClick={() =>
																			regenerateAllSeriesThumbnailsMutation.mutate(
																				library.id,
																			)
																		}
																		disabled={
																			regenerateAllSeriesThumbnailsMutation.isPending
																		}
																	>
																		Regenerate All
																	</Menu.Item>
																</>
															)}
															{/* Plugin actions for library-wide auto-match */}
															{(() => {
																// Filter plugin actions to only show those that apply to this library
																// Empty libraryIds means plugin applies to all libraries
																const libraryPluginActions =
																	pluginActions?.actions.filter((action) => {
																		const libIds = action.libraryIds ?? [];
																		return (
																			libIds.length === 0 ||
																			libIds.includes(library.id)
																		);
																	}) ?? [];

																return (
																	libraryPluginActions.length > 0 && (
																		<>
																			<Menu.Divider />
																			<Menu.Label>
																				Auto-Apply Metadata
																			</Menu.Label>
																			{libraryPluginActions.map((action) => (
																				<Menu.Item
																					key={`auto-match-${action.pluginId}`}
																					leftSection={<IconWand size={16} />}
																					onClick={() =>
																						handleLibraryAutoMatch(
																							library,
																							action,
																						)
																					}
																					disabled={autoMatchMutation.isPending}
																				>
																					{action.pluginDisplayName}
																				</Menu.Item>
																			))}
																		</>
																	)
																);
															})()}
															<Menu.Divider />
															<Menu.Item
																leftSection={<IconTrashX size={16} />}
																color="orange"
																onClick={() => handlePurgeDeleted(library)}
															>
																Purge Deleted Books
															</Menu.Item>
														</>
													)}
													{canDeleteLibrary && (
														<Menu.Item
															leftSection={<IconTrash size={16} />}
															color="red"
															onClick={() => handleDeleteLibrary(library)}
														>
															Delete Library
														</Menu.Item>
													)}
												</Menu.Dropdown>
											</Menu>
										)
									}
								/>
							))
						) : (
							<NavLink
								label="No libraries"
								disabled
								styles={{ root: { paddingLeft: 32 } }}
							/>
						)}

						<NavLink
							label="Settings"
							leftSection={<IconSettings size={20} />}
							opened={settingsOpened}
							onChange={setSettingsOpened}
							childrenOffset={32}
							active={currentPath.startsWith("/settings")}
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
										leftSection={<IconServer size={16} />}
										active={currentPath.startsWith("/settings/server")}
									/>
									<NavLink
										component={Link}
										to="/settings/tasks"
										label="Tasks"
										leftSection={<IconClipboardList size={16} />}
										active={currentPath.startsWith("/settings/tasks")}
									/>
									<NavLink
										component={Link}
										to="/settings/metrics"
										label="Metrics"
										leftSection={<IconChartBar size={16} />}
										active={currentPath.startsWith("/settings/metrics")}
									/>
									<NavLink
										component={Link}
										to="/settings/plugins"
										label="Plugins"
										leftSection={<IconPlugConnected size={16} />}
										active={currentPath.startsWith("/settings/plugins")}
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
										leftSection={<IconUsers size={16} />}
										active={currentPath.startsWith("/settings/users")}
									/>
									<NavLink
										component={Link}
										to="/settings/sharing-tags"
										label="Sharing Tags"
										leftSection={<IconShare size={16} />}
										active={currentPath.startsWith("/settings/sharing-tags")}
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
										leftSection={<IconCopy size={16} />}
										active={currentPath.startsWith("/settings/duplicates")}
									/>
									<NavLink
										component={Link}
										to="/settings/book-errors"
										label="Book Errors"
										leftSection={<IconAlertTriangle size={16} />}
										active={currentPath.startsWith("/settings/book-errors")}
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
										leftSection={<IconBrush size={16} />}
										active={currentPath.startsWith("/settings/cleanup")}
									/>
									<NavLink
										component={Link}
										to="/settings/pdf-cache"
										label="Page Cache"
										leftSection={<IconFileTypePdf size={16} />}
										active={currentPath.startsWith("/settings/pdf-cache")}
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
								to="/settings/profile"
								label="Profile"
								leftSection={<IconUser size={16} />}
								active={currentPath.startsWith("/settings/profile")}
							/>
						</NavLink>
					</Stack>
				</AppShell.Section>

				<AppShell.Section>
					<Stack gap="xs">
						<TaskNotificationBadge />
						<NavLink
							label="Logout"
							leftSection={<IconLogout size={20} />}
							onClick={handleLogout}
							color="red"
						/>
						{appInfo?.version && (
							<Text size="xs" c="dimmed" ta="center">
								v{appInfo.version}
							</Text>
						)}
					</Stack>
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
