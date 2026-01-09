import {
	ActionIcon,
	Badge,
	Button,
	Card,
	Center,
	Container,
	Group,
	Loader,
	Menu,
	Modal,
	Progress,
	SimpleGrid,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconBooks,
	IconDots,
	IconEdit,
	IconFolder,
	IconPlus,
	IconRadar,
	IconScan,
	IconTrash,
	IconTrashX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { librariesApi } from "@/api/libraries";
import { scanApi } from "@/api/scan";
import { AddLibraryModal } from "@/components/forms/AddLibraryModal";
import { EditLibraryModal } from "@/components/forms/EditLibraryModal";
import { useAuthStore } from "@/store/authStore";
import type { Library, ScanProgress } from "@/types/api";

export function Home() {
	const queryClient = useQueryClient();
	const { isAuthenticated } = useAuthStore();
	const [addLibraryOpened, setAddLibraryOpened] = useState(false);
	const [editLibraryOpened, setEditLibraryOpened] = useState(false);
	const [selectedLibrary, setSelectedLibrary] = useState<Library | null>(null);
	const [deleteConfirmOpened, setDeleteConfirmOpened] = useState(false);
	const [libraryToDelete, setLibraryToDelete] = useState<Library | null>(null);
	const [purgeConfirmOpened, setPurgeConfirmOpened] = useState(false);
	const [libraryToPurge, setLibraryToPurge] = useState<Library | null>(null);
	const [scanProgress, setScanProgress] = useState<
		Record<string, ScanProgress>
	>({});

	const { data: libraries, isLoading } = useQuery({
		queryKey: ["libraries"],
		queryFn: librariesApi.getAll,
	});

	// Subscribe to scan progress updates via SSE
	useEffect(() => {
		// Only subscribe if user is authenticated
		if (!isAuthenticated) {
			return;
		}

		const unsubscribe = scanApi.subscribeToProgress(
			(progress) => {
				setScanProgress((prev) => ({
					...prev,
					[progress.library_id]: progress,
				}));

				// Refresh library data when scan completes
				if (progress.status === "completed" || progress.status === "failed") {
					queryClient.invalidateQueries({ queryKey: ["libraries"] });

					// Show notification
					if (progress.status === "completed") {
						notifications.show({
							title: "Scan completed",
							message: `Found ${progress.books_found} books in ${progress.series_found} series from ${progress.files_processed} files`,
							color: "green",
						});
					} else if (progress.status === "failed") {
						notifications.show({
							title: "Scan failed",
							message:
								progress.error_message || "An error occurred during scanning",
							color: "red",
						});
					}

					// Remove progress after a delay
					setTimeout(() => {
						setScanProgress((prev) => {
							const updated = { ...prev };
							delete updated[progress.library_id];
							return updated;
						});
					}, 5000);
				}
			},
			(error) => {
				console.error("SSE error:", error);
			},
		);

		return () => {
			unsubscribe();
		};
	}, [queryClient, isAuthenticated]);

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

	const purgeMutation = useMutation({
		mutationFn: (libraryId: string) => librariesApi.purgeDeleted(libraryId),
		onSuccess: (count) => {
			notifications.show({
				title: "Success",
				message: `Purged ${count} deleted book${count !== 1 ? "s" : ""} from library`,
				color: "green",
			});
			queryClient.invalidateQueries({ queryKey: ["libraries"] });
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

	const handleScanAll = (mode: "normal" | "deep") => {
		if (!libraries) return;

		// Trigger scan for each library
		libraries.forEach((library) => {
			scanMutation.mutate({ libraryId: library.id, mode });
		});
	};

	if (isLoading) {
		return (
			<Center h="100vh">
				<Loader size="xl" />
			</Center>
		);
	}

	return (
		<Container size="xl" py="xl">
			<Stack gap="xl">
				<Group justify="space-between">
					<Title order={1}>Libraries</Title>
					<Group gap="xs">
						<Menu shadow="md" width={200} position="bottom-end">
							<Menu.Target>
								<ActionIcon
									variant="light"
									size="lg"
									title="Scan All Libraries"
								>
									<IconScan size={20} />
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
									Deep Scan All Libraries
								</Menu.Item>
							</Menu.Dropdown>
						</Menu>

						<ActionIcon
							variant="light"
							size="lg"
							onClick={() => setAddLibraryOpened(true)}
							title="Add Library"
						>
							<IconPlus size={20} />
						</ActionIcon>
					</Group>
				</Group>

				{libraries && libraries.length > 0 ? (
					<SimpleGrid cols={{ base: 1, sm: 2, lg: 3 }} spacing="lg">
						{libraries.map((library: Library) => {
							const progress = scanProgress[library.id];
							const isScanning =
								progress &&
								(progress.status === "pending" ||
									progress.status === "running");
							const progressPercent = progress?.files_total
								? Math.round(
										(progress.files_processed / progress.files_total) * 100,
									)
								: 0;

							return (
								<Card
									key={library.id}
									shadow="sm"
									padding="lg"
									radius="md"
									withBorder
								>
									<Stack gap="md">
										<Group justify="space-between">
											<Group gap="xs">
												<Text fw={500} size="lg">
													{library.name}
												</Text>
												<Menu shadow="md" width={200} position="bottom-start">
													<Menu.Target>
														<ActionIcon variant="subtle" color="gray" size="sm">
															<IconDots size={16} />
														</ActionIcon>
													</Menu.Target>

													<Menu.Dropdown>
														<Menu.Item
															leftSection={<IconScan size={16} />}
															onClick={() =>
																scanMutation.mutate({
																	libraryId: library.id,
																	mode: "normal",
																})
															}
															disabled={isScanning}
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
															disabled={isScanning}
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
														<Menu.Divider />
														<Menu.Item
															leftSection={<IconTrashX size={16} />}
															color="orange"
															onClick={() => handlePurgeDeleted(library)}
														>
															Purge Deleted Books
														</Menu.Item>
														<Menu.Item
															leftSection={<IconTrash size={16} />}
															color="red"
															onClick={() => handleDeleteLibrary(library)}
														>
															Delete Library
														</Menu.Item>
													</Menu.Dropdown>
												</Menu>
											</Group>
											<Badge
												color={
													library.scanningConfig?.enabled ? "green" : "gray"
												}
											>
												{library.scanningConfig?.enabled ? "Auto" : "Manual"}
											</Badge>
										</Group>

										<Text size="sm" c="dimmed" lineClamp={2}>
											{library.path}
										</Text>

										<Group gap="xs">
											<Group gap={4}>
												<IconBooks size={16} />
												<Text size="sm">{library.bookCount || 0} books</Text>
											</Group>
											<Text size="sm" c="dimmed">
												{library.seriesCount || 0} series
											</Text>
										</Group>

										{library.lastScannedAt && !isScanning && (
											<Text size="xs" c="dimmed">
												Last scan:{" "}
												{new Date(library.lastScannedAt).toLocaleString()}
											</Text>
										)}

										{isScanning && progress && (
											<Stack gap="xs">
												<Group justify="space-between">
													<Text size="sm" fw={500}>
														{progress.status === "pending"
															? "Starting scan..."
															: "Scanning..."}
													</Text>
													<Text size="sm" c="dimmed">
														{progress.files_processed} / {progress.files_total}
													</Text>
												</Group>
												<Progress value={progressPercent} size="sm" animated />
												{(progress.books_found > 0 ||
													progress.series_found > 0) && (
													<Text size="xs" c="dimmed">
														Found {progress.books_found} books in{" "}
														{progress.series_found} series
													</Text>
												)}
											</Stack>
										)}
									</Stack>
								</Card>
							);
						})}
					</SimpleGrid>
				) : (
					<Card padding="xl" radius="md" withBorder>
						<Center>
							<Stack align="center" gap="md">
								<IconFolder size={48} stroke={1.5} />
								<Title order={3}>No libraries found</Title>
								<Text c="dimmed" ta="center">
									Get started by adding your first library
								</Text>
								<Button
									leftSection={<IconFolder size={18} />}
									onClick={() => setAddLibraryOpened(true)}
								>
									Add Library
								</Button>
							</Stack>
						</Center>
					</Card>
				)}
			</Stack>

			<AddLibraryModal
				opened={addLibraryOpened}
				onClose={() => setAddLibraryOpened(false)}
			/>

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
		</Container>
	);
}
