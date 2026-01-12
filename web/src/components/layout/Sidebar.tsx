import {
	ActionIcon,
	AppShell,
	Button,
	Group,
	Menu,
	Modal,
	NavLink,
	Stack,
	Text,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconBooks,
	IconChartBar,
	IconClipboardList,
	IconCopy,
	IconDotsVertical,
	IconEdit,
	IconHome,
	IconLogout,
	IconPlus,
	IconRadar,
	IconScan,
	IconServer,
	IconSettings,
	IconTrash,
	IconTrashX,
	IconUser,
	IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import { LibraryModal } from "@/components/forms/LibraryModal";
import { TaskNotificationBadge } from "@/components/TaskNotificationBadge";
import { useAuthStore } from "@/store/authStore";
import { useLibraryPreferencesStore } from "@/store/libraryPreferencesStore";
import type { Library } from "@/types";

interface SidebarProps {
	currentPath?: string;
}

export function Sidebar({ currentPath = "/" }: SidebarProps) {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { user, clearAuth } = useAuthStore();
	// Only subscribe to getLastTab action (doesn't cause re-renders since it's not state)
	const getLastTab = useLibraryPreferencesStore((state) => state.getLastTab);
	const isAdmin = user?.isAdmin;
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

	const { data: libraries } = useQuery({
		queryKey: ["libraries"],
		queryFn: librariesApi.getAll,
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

	const handleScanAll = (mode: "normal" | "deep") => {
		if (!libraries) return;

		libraries.forEach((library) => {
			scanMutation.mutate({ libraryId: library.id, mode });
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
							onClick={() => navigate("/")}
							label="Home"
							leftSection={<IconHome size={20} />}
							active={currentPath === "/"}
						/>
						<NavLink
							onClick={() => {
								const lastTab = getLastTab("all") || "series";
								navigate(`/libraries/all/${lastTab}`);
							}}
							label="Libraries"
							leftSection={<IconBooks size={20} />}
							opened
							childrenOffset={32}
							disableRightSectionRotation
							active={currentPath.startsWith("/libraries/all")}
							rightSection={
								<Group gap={4}>
									<ActionIcon
										variant="subtle"
										size="sm"
										onClick={(e: React.MouseEvent) => {
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
												onClick={(e: React.MouseEvent) => e.stopPropagation()}
												title="Options"
											>
												<IconDotsVertical size={16} />
											</ActionIcon>
										</Menu.Target>

										<Menu.Dropdown>
											<Menu.Item
												leftSection={<IconScan size={16} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													handleScanAll("normal");
												}}
											>
												Scan All Libraries
											</Menu.Item>
											<Menu.Item
												leftSection={<IconRadar size={16} />}
												onClick={(e: React.MouseEvent) => {
													e.stopPropagation();
													handleScanAll("deep");
												}}
											>
												Deep Scan All Libraries
											</Menu.Item>
										</Menu.Dropdown>
									</Menu>
								</Group>
							}
						>
							{libraries && libraries.length > 0 ? (
								libraries.map((library) => (
									<NavLink
										key={library.id}
										onClick={() => {
											const lastTab = getLastTab(library.id) || "recommended";
											navigate(`/libraries/${library.id}/${lastTab}`);
										}}
										label={library.name}
										// leftSection={<IconFolder size={16} />}
										active={currentPath.startsWith(`/libraries/${library.id}/`)}
										styles={{ label: { textTransform: "capitalize" } }}
										rightSection={
											<Menu shadow="md" width={200} position="right-start">
												<Menu.Target>
													<ActionIcon
														variant="subtle"
														size="xs"
														onClick={(e: React.MouseEvent) =>
															e.stopPropagation()
														}
														title="Library options"
													>
														<IconDotsVertical size={14} />
													</ActionIcon>
												</Menu.Target>

												<Menu.Dropdown>
													<Menu.Item
														leftSection={<IconScan size={16} />}
														onClick={(e: React.MouseEvent) => {
															e.stopPropagation();
															scanMutation.mutate({
																libraryId: library.id,
																mode: "normal",
															});
														}}
													>
														Scan Library
													</Menu.Item>
													<Menu.Item
														leftSection={<IconRadar size={16} />}
														onClick={(e: React.MouseEvent) => {
															e.stopPropagation();
															scanMutation.mutate({
																libraryId: library.id,
																mode: "deep",
															});
														}}
													>
														Scan Library (Deep)
													</Menu.Item>
													<Menu.Divider />
													<Menu.Item
														leftSection={<IconEdit size={16} />}
														onClick={(e: React.MouseEvent) => {
															e.stopPropagation();
															handleEditLibrary(library);
														}}
													>
														Edit Library
													</Menu.Item>
													<Menu.Divider />
													<Menu.Item
														leftSection={<IconTrashX size={16} />}
														color="orange"
														onClick={(e: React.MouseEvent) => {
															e.stopPropagation();
															handlePurgeDeleted(library);
														}}
													>
														Purge Deleted Books
													</Menu.Item>
													<Menu.Item
														leftSection={<IconTrash size={16} />}
														color="red"
														onClick={(e: React.MouseEvent) => {
															e.stopPropagation();
															handleDeleteLibrary(library);
														}}
													>
														Delete Library
													</Menu.Item>
												</Menu.Dropdown>
											</Menu>
										}
									/>
								))
							) : (
								<NavLink label="No libraries" disabled />
							)}
						</NavLink>

						<NavLink
							label="Settings"
							leftSection={<IconSettings size={20} />}
							opened={settingsOpened}
							onChange={setSettingsOpened}
							childrenOffset={32}
							active={currentPath.startsWith("/settings")}
						>
							{/* Admin Section */}
							{isAdmin && (
								<>
									<NavLink
										onClick={() => navigate("/settings/server")}
										label="Server"
										leftSection={<IconServer size={16} />}
										active={currentPath.startsWith("/settings/server")}
									/>
									<NavLink
										onClick={() => navigate("/settings/users")}
										label="Users"
										leftSection={<IconUsers size={16} />}
										active={currentPath.startsWith("/settings/users")}
									/>
									<NavLink
										onClick={() => navigate("/settings/tasks")}
										label="Tasks"
										leftSection={<IconClipboardList size={16} />}
										active={currentPath.startsWith("/settings/tasks")}
									/>
									<NavLink
										onClick={() => navigate("/settings/duplicates")}
										label="Duplicates"
										leftSection={<IconCopy size={16} />}
										active={currentPath.startsWith("/settings/duplicates")}
									/>
									<NavLink
										onClick={() => navigate("/settings/metrics")}
										label="Metrics"
										leftSection={<IconChartBar size={16} />}
										active={currentPath.startsWith("/settings/metrics")}
									/>
								</>
							)}

							{/* User Section */}
							<NavLink
								onClick={() => navigate("/settings/profile")}
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
						This will remove the library from Codex. The files on disk will not
						be deleted.
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
