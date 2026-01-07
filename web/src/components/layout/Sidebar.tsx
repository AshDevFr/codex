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
	IconDotsVertical,
	IconEdit,
	IconHome,
	IconLogout,
	IconPlus,
	IconRadar,
	IconScan,
	IconSettings,
	IconTrash,
	IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import { EditLibraryModal } from "@/components/forms/EditLibraryModal";
import { useAuthStore } from "@/store/authStore";
import type { Library } from "@/types/api";

interface SidebarProps {
	currentPath?: string;
}

export function Sidebar({ currentPath = "/" }: SidebarProps) {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { user, clearAuth } = useAuthStore();
	const isAdmin = user?.isAdmin;
	const [editLibraryOpened, setEditLibraryOpened] = useState(false);
	const [selectedLibrary, setSelectedLibrary] = useState<Library | null>(null);
	const [deleteConfirmOpened, setDeleteConfirmOpened] = useState(false);
	const [libraryToDelete, setLibraryToDelete] = useState<Library | null>(null);

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
			queryClient.invalidateQueries({ queryKey: ["libraries"] });
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
			queryClient.invalidateQueries({ queryKey: ["libraries"] });
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
							label="Libraries"
							leftSection={<IconBooks size={20} />}
							opened
							childrenOffset={32}
							disableRightSectionRotation
							rightSection={
								<Group gap={4}>
									<ActionIcon
										variant="subtle"
										size="sm"
										onClick={(e: React.MouseEvent) => {
											e.stopPropagation();
											navigate("/libraries");
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
										onClick={() => navigate(`/library/${library.id}`)}
										label={library.name}
										// leftSection={<IconFolder size={16} />}
										active={currentPath === `/library/${library.id}`}
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

						{isAdmin && (
							<>
								<NavLink
									onClick={() => navigate("/users")}
									label="Users"
									leftSection={<IconUsers size={20} />}
									active={currentPath === "/users"}
								/>
							</>
						)}

						<NavLink
							onClick={() => navigate("/settings")}
							label="Settings"
							leftSection={<IconSettings size={20} />}
							active={currentPath === "/settings"}
						/>
					</Stack>
				</AppShell.Section>

				<AppShell.Section>
					<NavLink
						label="Logout"
						leftSection={<IconLogout size={20} />}
						onClick={handleLogout}
						color="red"
					/>
				</AppShell.Section>
			</AppShell.Navbar>

			<EditLibraryModal
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
		</>
	);
}
