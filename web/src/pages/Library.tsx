import {
	ActionIcon,
	Badge,
	Box,
	Button,
	Center,
	Container,
	Group,
	Loader,
	Menu,
	Modal,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconDotsVertical,
	IconEdit,
	IconRadar,
	IconScan,
	IconTrash,
	IconTrashX,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import {
	Navigate,
	useLocation,
	useNavigate,
	useParams,
	useSearchParams,
} from "react-router-dom";
import { librariesApi } from "@/api/libraries";
import { LibraryModal } from "@/components/forms/LibraryModal";
import { BooksSection } from "@/components/library/BooksSection";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { RecommendedSection } from "@/components/library/RecommendedSection";
import { SeriesSection } from "@/components/library/SeriesSection";
import { useDynamicDocumentTitle } from "@/hooks/useDocumentTitle";
import { usePermissions } from "@/hooks/usePermissions";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import {
	useLibraryPreferencesHydrated,
	useLibraryPreferencesStore,
} from "@/store/libraryPreferencesStore";
import type { Library } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

export function LibraryPage() {
	const { libraryId } = useParams<{ libraryId: string }>();
	const location = useLocation();
	const navigate = useNavigate();
	const [searchParams, setSearchParams] = useSearchParams();

	// Wait for preferences store to hydrate from localStorage
	const hasHydrated = useLibraryPreferencesHydrated();

	// Use performance selectors - only subscribe to actions (no re-renders)
	const getTabPreferences = useLibraryPreferencesStore(
		(state) => state.getTabPreferences,
	);
	const setTabPreferences = useLibraryPreferencesStore(
		(state) => state.setTabPreferences,
	);
	const setLastTab = useLibraryPreferencesStore((state) => state.setLastTab);

	// Determine current tab from URL
	const pathParts = location.pathname.split("/");

	// Handle libraryId === "all" case
	const isAllLibraries = libraryId === "all";

	// Default tab: "recommended" for specific libraries, "series" for all libraries
	const defaultTab = isAllLibraries ? "series" : "recommended";
	const currentTab = pathParts[pathParts.length - 1] || defaultTab;

	// State for total counts
	const [booksCount, setBooksCount] = useState<number | null>(null);
	const [seriesCount, setSeriesCount] = useState<number | null>(null);

	// State for library actions
	const [editLibraryOpened, setEditLibraryOpened] = useState(false);
	const [deleteConfirmOpened, setDeleteConfirmOpened] = useState(false);
	const [purgeConfirmOpened, setPurgeConfirmOpened] = useState(false);
	const [libraryToDelete, setLibraryToDelete] = useState<Library | null>(null);
	const [libraryToPurge, setLibraryToPurge] = useState<Library | null>(null);

	const queryClient = useQueryClient();

	// Permission checks
	const { hasPermission } = usePermissions();
	const canEditLibrary = hasPermission(PERMISSIONS.LIBRARIES_WRITE);
	const canDeleteLibrary = hasPermission(PERMISSIONS.LIBRARIES_DELETE);

	// Get active tasks for progress display
	const { getTasksByLibrary } = useTaskProgress();

	// Get active scan tasks for this library
	const activeScanTasks = libraryId
		? getTasksByLibrary(libraryId).filter(
				(task) =>
					task.task_type === "scan_library" && task.status === "running",
			)
		: [];

	// Reset counts when tab changes
	useEffect(() => {
		if (currentTab === "recommended") {
			setBooksCount(null);
			setSeriesCount(null);
		}
	}, [currentTab]);

	// Fetch library data (if not "all")
	const {
		data: library,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["library", libraryId],
		queryFn: () => {
			if (!libraryId) throw new Error("Library ID is required");
			return librariesApi.getById(libraryId);
		},
		enabled: !isAllLibraries && !!libraryId,
	});

	// Set document title based on library name
	useDynamicDocumentTitle(
		isAllLibraries ? "All Libraries" : library?.name,
		"Library",
	);

	// Redirect to base path if no tab specified
	useEffect(() => {
		if (
			location.pathname === `/libraries/${libraryId}` ||
			location.pathname === `/libraries/${libraryId}/`
		) {
			navigate(`/libraries/${libraryId}/${defaultTab}`, { replace: true });
		}
	}, [location.pathname, libraryId, navigate, defaultTab]);

	// Handle 404 - redirect to home
	useEffect(() => {
		if (error && !isAllLibraries) {
			navigate("/", { replace: true });
		}
	}, [error, isAllLibraries, navigate]);

	// Restore preferences from localStorage or update store from URL
	useEffect(() => {
		if (!libraryId) return;

		const hasUrlParams = Array.from(searchParams.keys()).length > 0;
		const storedPrefs = getTabPreferences(libraryId, currentTab);

		if (!hasUrlParams && storedPrefs) {
			// Restore from localStorage
			const params = new URLSearchParams();
			if (storedPrefs.pageSize)
				params.set("pageSize", storedPrefs.pageSize.toString());
			if (storedPrefs.sort) params.set("sort", storedPrefs.sort);
			if (storedPrefs.filters) {
				Object.entries(storedPrefs.filters).forEach(([key, value]) => {
					params.set(key, value);
				});
			}
			setSearchParams(params, { replace: true });
		} else if (hasUrlParams) {
			// URL params exist - update store to match
			const currentPrefs = {
				pageSize: parseInt(searchParams.get("pageSize") || "20", 10),
				sort: searchParams.get("sort") || undefined,
				filters: {} as Record<string, string>,
			};

			// Capture filter params (anything that's not page/pageSize/sort)
			searchParams.forEach((value, key) => {
				if (!["page", "pageSize", "sort"].includes(key)) {
					currentPrefs.filters[key] = value;
				}
			});

			setTabPreferences(libraryId, currentTab, currentPrefs);
		}
	}, [
		libraryId,
		currentTab,
		searchParams,
		getTabPreferences,
		setTabPreferences,
		setSearchParams,
	]);

	// Mutations for library actions
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
			queryClient.refetchQueries({ queryKey: ["libraries"] });
			queryClient.refetchQueries({ queryKey: ["library", libraryId] });
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
			navigate("/");
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
			queryClient.refetchQueries({ queryKey: ["library", libraryId] });
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

	// Tab navigation
	const handleTabChange = (value: string | null) => {
		if (value && libraryId) {
			setLastTab(libraryId, value);
			navigate(`/libraries/${libraryId}/${value}`);
		}
	};

	// Library action handlers
	const handleEditLibrary = () => {
		if (library) {
			setEditLibraryOpened(true);
		}
	};

	const handleDeleteLibrary = () => {
		if (library) {
			setLibraryToDelete(library);
			setDeleteConfirmOpened(true);
		}
	};

	const confirmDelete = () => {
		if (libraryToDelete) {
			deleteMutation.mutate(libraryToDelete.id);
		}
	};

	const handlePurgeDeleted = () => {
		if (library) {
			setLibraryToPurge(library);
			setPurgeConfirmOpened(true);
		}
	};

	const confirmPurge = () => {
		if (libraryToPurge) {
			purgeMutation.mutate(libraryToPurge.id);
		}
	};

	// Read sort and page size from URL
	const pageSize = parseInt(searchParams.get("pageSize") || "20", 10);
	const sort =
		searchParams.get("sort") ||
		(currentTab === "books" ? "title,asc" : "name,asc");

	// Handle filter changes
	const handleFilterChange = (updates: Record<string, string | number>) => {
		if (!libraryId) return;

		const params = new URLSearchParams(searchParams);

		Object.entries(updates).forEach(([key, value]) => {
			if (value) {
				params.set(key, value.toString());
			} else {
				params.delete(key);
			}
		});

		// Reset to page 1 when filters change
		if (!("page" in updates)) {
			params.set("page", "1");
		}

		setSearchParams(params, { replace: true });

		// Persist filters to store (exclude page/pageSize/sort)
		const currentPrefs = getTabPreferences(libraryId, currentTab) || {};
		const newFilters = { ...currentPrefs.filters };

		Object.entries(updates).forEach(([key, value]) => {
			if (!["page", "pageSize", "sort"].includes(key)) {
				if (value) {
					newFilters[key] = value.toString();
				} else {
					delete newFilters[key];
				}
			}
		});

		setTabPreferences(libraryId, currentTab, {
			...currentPrefs,
			filters: newFilters,
		});
	};

	// Get current count based on tab
	const currentCount =
		currentTab === "books"
			? booksCount
			: currentTab === "series"
				? seriesCount
				: null;
	const countLabel =
		currentTab === "books" ? "books" : currentTab === "series" ? "series" : "";

	// Sort options based on tab
	const sortOptions =
		currentTab === "books"
			? [
					{
						field: "series",
						label: "Series",
						defaultDirection: "asc" as const,
					},
					{ field: "title", label: "Title", defaultDirection: "asc" as const },
					{
						field: "created_at",
						label: "Date Added",
						defaultDirection: "desc" as const,
					},
					{
						field: "release_date",
						label: "Release Date",
						defaultDirection: "desc" as const,
					},
					{
						field: "chapter_number",
						label: "Chapter Number",
						defaultDirection: "asc" as const,
					},
					{
						field: "file_size",
						label: "File Size",
						defaultDirection: "desc" as const,
					},
					{
						field: "filename",
						label: "Filename",
						defaultDirection: "asc" as const,
					},
					{
						field: "page_count",
						label: "Page Count",
						defaultDirection: "desc" as const,
					},
				]
			: [
					{ field: "name", label: "Name", defaultDirection: "asc" as const },
					{
						field: "date_added",
						label: "Date Added",
						defaultDirection: "desc" as const,
					},
					{
						field: "date_updated",
						label: "Date Updated",
						defaultDirection: "desc" as const,
					},
					{
						field: "release_date",
						label: "Release Date",
						defaultDirection: "desc" as const,
					},
					{
						field: "date_read",
						label: "Recently Read",
						defaultDirection: "desc" as const,
					},
					{
						field: "book_count",
						label: "Book Count",
						defaultDirection: "desc" as const,
					},
				];

	if (!libraryId) {
		return <Navigate to="/" replace />;
	}

	// Wait for preferences to hydrate before rendering to prevent flash of default values
	if (!hasHydrated || (isLoading && !isAllLibraries)) {
		return (
			<Center h={400}>
				<Loader size="lg" />
			</Center>
		);
	}

	if (error && !isAllLibraries) {
		return (
			<Container size="xl" py="xl">
				<Center h={400}>
					<Stack align="center" gap="md">
						<Text size="xl" fw={600}>
							Library Not Found
						</Text>
						<Text c="dimmed">The requested library could not be found.</Text>
					</Stack>
				</Center>
			</Container>
		);
	}

	return (
		<>
			<Box
				style={{
					display: "flex",
					flexDirection: "column",
					height: "calc(100vh - 64px)", // Subtract AppShell header height
					overflow: "hidden",
					margin: "calc(-1 * var(--mantine-spacing-md))", // Offset AppShell padding
				}}
			>
				{/* Fixed header area - does not scroll */}
				<Box px="md" pt="sm" pb="xs" style={{ flexShrink: 0, backgroundColor: "var(--mantine-color-body)" }}>
					<Stack gap="xs">
						{/* Header with library name, count, and action menu */}
						<Group gap="md" align="center" justify="space-between">
						<Group gap="xs" align="baseline">
							{!isAllLibraries &&
								library &&
								(canEditLibrary || canDeleteLibrary) && (
									<Menu shadow="md" width={200} position="bottom-start">
										<Menu.Target>
											<ActionIcon variant="subtle" color="gray" size="lg">
												<IconDotsVertical size={20} />
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
														onClick={handleEditLibrary}
													>
														Edit Library
													</Menu.Item>
												</>
											)}
											{(canEditLibrary || canDeleteLibrary) && (
												<>
													<Menu.Divider />
													{canEditLibrary && (
														<Menu.Item
															leftSection={<IconTrashX size={16} />}
															color="orange"
															onClick={handlePurgeDeleted}
														>
															Purge Deleted Books
														</Menu.Item>
													)}
													{canDeleteLibrary && (
														<Menu.Item
															leftSection={<IconTrash size={16} />}
															color="red"
															onClick={handleDeleteLibrary}
														>
															Delete Library
														</Menu.Item>
													)}
												</>
											)}
										</Menu.Dropdown>
									</Menu>
								)}
							<Title order={3} tt="capitalize">
								{isAllLibraries ? "All Libraries" : library?.name || "Library"}
							</Title>
							{currentCount !== null && (
								<Text size="sm" c="dimmed" fw={500}>
									{currentCount} {countLabel}
								</Text>
							)}
							{/* Show scan progress badge when scanning */}
							{activeScanTasks.length > 0 && activeScanTasks[0].progress && (
								<Badge color="blue" variant="filled" size="sm">
									Scanning... {activeScanTasks[0].progress.current} /{" "}
									{activeScanTasks[0].progress.total}
								</Badge>
							)}
						</Group>
					</Group>

					{/* Toolbar with Tabs and Controls */}
					<LibraryToolbar
						currentTab={currentTab}
						onTabChange={handleTabChange}
						showRecommended={!isAllLibraries}
						sort={sort}
						onSortChange={(value) => {
							if (libraryId) {
								handleFilterChange({ sort: value });
								const currentPrefs =
									getTabPreferences(libraryId, currentTab) || {};
								setTabPreferences(libraryId, currentTab, {
									...currentPrefs,
									sort: value,
								});
							}
						}}
						sortOptions={sortOptions}
						pageSize={pageSize}
						onPageSizeChange={(value) => {
							if (libraryId) {
								handleFilterChange({ pageSize: value });
								const currentPrefs =
									getTabPreferences(libraryId, currentTab) || {};
								setTabPreferences(libraryId, currentTab, {
									...currentPrefs,
									pageSize: value,
								});
							}
						}}
					/>
					</Stack>
				</Box>

				{/* Scrollable content area */}
				<Box
					px="md"
					pb="xl"
					style={{
						flex: 1,
						overflowY: "auto",
					}}
				>
					{currentTab === "recommended" && !isAllLibraries && (
						<RecommendedSection libraryId={libraryId} />
					)}

					{currentTab === "series" && (
						<SeriesSection
							libraryId={libraryId}
							searchParams={searchParams}
							onTotalChange={setSeriesCount}
						/>
					)}

					{currentTab === "books" && (
						<BooksSection
							libraryId={libraryId}
							searchParams={searchParams}
							onTotalChange={setBooksCount}
						/>
					)}
				</Box>
			</Box>

			{/* Edit Library Modal */}
			<LibraryModal
				opened={editLibraryOpened}
				onClose={() => {
					setEditLibraryOpened(false);
					queryClient.refetchQueries({ queryKey: ["library", libraryId] });
				}}
				library={library || undefined}
			/>

			{/* Delete Confirmation Modal */}
			<Modal
				opened={deleteConfirmOpened}
				onClose={() => setDeleteConfirmOpened(false)}
				title="Delete Library"
				centered
			>
				<Stack gap="md">
					<Text>
						Are you sure you want to delete "{libraryToDelete?.name}"? This
						action cannot be undone.
					</Text>
					<Group justify="flex-end" gap="sm">
						<Button
							variant="subtle"
							onClick={() => setDeleteConfirmOpened(false)}
						>
							Cancel
						</Button>
						<Button color="red" onClick={confirmDelete}>
							Delete
						</Button>
					</Group>
				</Stack>
			</Modal>

			{/* Purge Deleted Confirmation Modal */}
			<Modal
				opened={purgeConfirmOpened}
				onClose={() => setPurgeConfirmOpened(false)}
				title="Purge Deleted Books"
				centered
			>
				<Stack gap="md">
					<Text>
						Are you sure you want to permanently delete all soft-deleted books
						from "{libraryToPurge?.name}"? This action cannot be undone.
					</Text>
					<Group justify="flex-end" gap="sm">
						<Button
							variant="subtle"
							onClick={() => setPurgeConfirmOpened(false)}
						>
							Cancel
						</Button>
						<Button color="orange" onClick={confirmPurge}>
							Purge
						</Button>
					</Group>
				</Stack>
			</Modal>
		</>
	);
}
