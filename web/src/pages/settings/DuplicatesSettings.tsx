import {
	ActionIcon,
	Alert,
	Anchor,
	Badge,
	Box,
	Button,
	Card,
	Group,
	Loader,
	Stack,
	Table,
	Text,
	Title,
	Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
	IconAlertCircle,
	IconCopy,
	IconRefresh,
	IconSearch,
	IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { api } from "@/api/client";
import { type DuplicateGroup, duplicatesApi } from "@/api/duplicates";
import { AppLink } from "@/components/common/AppLink";
import { useTaskProgress } from "@/hooks/useTaskProgress";
import type { Book } from "@/types";

// Duplicate scan task type
const DUPLICATE_SCAN_TASK_TYPE = "find_duplicates";

// Throttle duration for refresh (30 seconds)
const REFRESH_THROTTLE_MS = 30000;

// Duplicate group card component
function DuplicateGroupCard({
	group,
	books,
	onDelete,
	isDeleting,
}: {
	group: DuplicateGroup;
	books: Book[];
	onDelete: () => void;
	isDeleting: boolean;
}) {
	const [expanded, setExpanded] = useState(false);

	return (
		<Card withBorder padding="md">
			<Group justify="space-between" mb="md">
				<Group gap="sm">
					<IconCopy size={20} />
					<Box>
						<Text fw={500}>{group.duplicate_count} Duplicates</Text>
						<Text size="xs" c="dimmed" style={{ fontFamily: "monospace" }}>
							{group.file_hash.slice(0, 16)}...
						</Text>
					</Box>
				</Group>
				<Group gap="xs">
					<Badge variant="light" color="orange">
						{group.duplicate_count} copies
					</Badge>
					<Button
						variant="subtle"
						size="xs"
						onClick={() => setExpanded(!expanded)}
					>
						{expanded ? "Hide" : "Show"} Details
					</Button>
					<Tooltip label="Delete duplicate group (keeps all files)">
						<ActionIcon
							variant="subtle"
							color="red"
							onClick={onDelete}
							loading={isDeleting}
						>
							<IconTrash size={16} />
						</ActionIcon>
					</Tooltip>
				</Group>
			</Group>

			{expanded && (
				<Table layout="fixed">
					<Table.Thead>
						<Table.Tr>
							<Table.Th style={{ width: "20%" }}>Book</Table.Th>
							<Table.Th style={{ width: "15%" }}>Library</Table.Th>
							<Table.Th style={{ width: "15%" }}>Series</Table.Th>
							<Table.Th style={{ width: "35%" }}>Path</Table.Th>
							<Table.Th style={{ width: "15%" }}>Size</Table.Th>
						</Table.Tr>
					</Table.Thead>
					<Table.Tbody>
						{books.map((book, index) => (
							<Table.Tr key={`${book.id}-${index}`}>
								<Table.Td>
									<Anchor
										size="sm"
										fw={500}
										truncate="end"
										c="blue.4"
										component={AppLink}
										to={`/books/${book.id}`}
									>
										{book.title}
									</Anchor>
								</Table.Td>
								<Table.Td>
									<Anchor
										size="sm"
										truncate="end"
										c="blue.4"
										component={AppLink}
										to={`/libraries/${book.libraryId}`}
									>
										{book.libraryName || "-"}
									</Anchor>
								</Table.Td>
								<Table.Td>
									{book.seriesId ? (
										<Anchor
											size="sm"
											truncate="end"
											c="blue.4"
											component={AppLink}
											to={`/series/${book.seriesId}`}
										>
											{book.seriesName || "-"}
										</Anchor>
									) : (
										<Text size="sm" truncate>
											-
										</Text>
									)}
								</Table.Td>
								<Table.Td>
									<Tooltip label={book.filePath}>
										<Text size="sm" truncate>
											{book.filePath}
										</Text>
									</Tooltip>
								</Table.Td>
								<Table.Td>
									<Text size="sm">
										{book.fileSize
											? `${(book.fileSize / 1024 / 1024).toFixed(2)} MB`
											: "-"}
									</Text>
								</Table.Td>
							</Table.Tr>
						))}
					</Table.Tbody>
				</Table>
			)}

			<Text size="xs" c="dimmed" mt="sm">
				Detected: {new Date(group.created_at).toLocaleString()}
			</Text>
		</Card>
	);
}

export function DuplicatesSettings() {
	const queryClient = useQueryClient();
	const [deletingGroupId, setDeletingGroupId] = useState<string | null>(null);
	const [bookDetailsCache, setBookDetailsCache] = useState<Map<string, Book[]>>(
		new Map(),
	);

	// Track completed duplicate scan tasks to trigger refresh
	const { activeTasks } = useTaskProgress();
	const lastRefreshTime = useRef<number>(0);
	const processedTaskIds = useRef<Set<string>>(new Set());

	// Fetch duplicates
	const {
		data: duplicates,
		isLoading,
		error,
		refetch: refetchDuplicates,
	} = useQuery({
		queryKey: ["duplicates"],
		queryFn: duplicatesApi.list,
	});

	// Watch for duplicate scan task completions and refresh (throttled to 30s)
	useEffect(() => {
		const completedScanTasks = activeTasks.filter(
			(task) =>
				task.task_type === DUPLICATE_SCAN_TASK_TYPE &&
				task.status === "completed" &&
				!processedTaskIds.current.has(task.task_id),
		);

		if (completedScanTasks.length > 0) {
			// Mark these tasks as processed
			for (const task of completedScanTasks) {
				processedTaskIds.current.add(task.task_id);
			}

			// Throttle refresh to avoid hammering the API
			const now = Date.now();
			if (now - lastRefreshTime.current >= REFRESH_THROTTLE_MS) {
				lastRefreshTime.current = now;
				refetchDuplicates();
			}
		}
	}, [activeTasks, refetchDuplicates]);

	// Fetch book details for a group
	const fetchBooksForGroup = async (group: DuplicateGroup): Promise<Book[]> => {
		const cached = bookDetailsCache.get(group.id);
		if (cached) return cached;

		const books: Book[] = [];
		for (const bookId of group.book_ids) {
			try {
				// API returns { book: BookDto, metadata: ... }
				const response = await api.get<{ book: Book }>(`/books/${bookId}`);
				books.push(response.data.book);
			} catch (err) {
				console.error(`Failed to fetch book ${bookId}:`, err);
			}
		}

		setBookDetailsCache((prev) => new Map(prev).set(group.id, books));
		return books;
	};

	// Preload book details when duplicates change
	const { data: groupBooks } = useQuery({
		queryKey: ["duplicate-books", duplicates?.map((d) => d.id).join(",")],
		queryFn: async () => {
			if (!duplicates) return new Map<string, Book[]>();

			const results = new Map<string, Book[]>();
			for (const group of duplicates) {
				const books = await fetchBooksForGroup(group);
				results.set(group.id, books);
			}
			return results;
		},
		enabled: !!duplicates && duplicates.length > 0,
	});

	// Mutations
	const scanMutation = useMutation({
		mutationFn: duplicatesApi.scan,
		onSuccess: (result) => {
			queryClient.invalidateQueries({ queryKey: ["duplicates"] });
			notifications.show({
				title: "Success",
				message: result.message || "Duplicate scan started",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to start duplicate scan",
				color: "red",
			});
		},
	});

	const deleteGroupMutation = useMutation({
		mutationFn: async (groupId: string) => {
			setDeletingGroupId(groupId);
			try {
				await duplicatesApi.delete(groupId);
			} finally {
				setDeletingGroupId(null);
			}
		},
		onSuccess: () => {
			queryClient.invalidateQueries({ queryKey: ["duplicates"] });
			notifications.show({
				title: "Success",
				message: "Duplicate group deleted",
				color: "green",
			});
		},
		onError: () => {
			notifications.show({
				title: "Error",
				message: "Failed to delete duplicate group",
				color: "red",
			});
		},
	});

	const totalDuplicates =
		duplicates?.reduce((sum, group) => sum + group.duplicate_count, 0) || 0;

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Group justify="space-between">
					<Box>
						<Title order={1}>Duplicate Detection</Title>
						<Text c="dimmed" size="sm">
							Find and manage duplicate files in your library
						</Text>
					</Box>
					<Group gap="xs">
						<Button
							variant="light"
							leftSection={<IconRefresh size={16} />}
							onClick={() =>
								queryClient.invalidateQueries({ queryKey: ["duplicates"] })
							}
						>
							Refresh
						</Button>
						<Button
							leftSection={<IconSearch size={16} />}
							onClick={() => scanMutation.mutate()}
							loading={scanMutation.isPending}
						>
							Scan for Duplicates
						</Button>
					</Group>
				</Group>

				{/* Summary Stats */}
				<Card withBorder>
					<Group justify="space-around">
						<Box style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{duplicates?.length || 0}
							</Text>
							<Text size="sm" c="dimmed">
								Duplicate Groups
							</Text>
						</Box>
						<Box style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{totalDuplicates}
							</Text>
							<Text size="sm" c="dimmed">
								Total Duplicates
							</Text>
						</Box>
						<Box style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{totalDuplicates - (duplicates?.length || 0)}
							</Text>
							<Text size="sm" c="dimmed">
								Redundant Copies
							</Text>
						</Box>
					</Group>
				</Card>

				{/* Info Alert */}
				<Alert icon={<IconAlertCircle size={16} />} color="blue">
					Duplicates are detected by comparing file hashes (SHA-256). Files with
					identical content are grouped together. Deleting a duplicate group
					only removes the tracking record - the actual files are not deleted.
				</Alert>

				{/* Duplicate Groups */}
				{isLoading ? (
					<Group justify="center" py="xl">
						<Loader />
					</Group>
				) : error ? (
					<Alert icon={<IconAlertCircle size={16} />} color="red">
						Failed to load duplicates. Please try again.
					</Alert>
				) : duplicates && duplicates.length > 0 ? (
					<Stack gap="md">
						{duplicates.map((group) => (
							<DuplicateGroupCard
								key={group.id}
								group={group}
								books={groupBooks?.get(group.id) || []}
								onDelete={() => deleteGroupMutation.mutate(group.id)}
								isDeleting={deletingGroupId === group.id}
							/>
						))}
					</Stack>
				) : (
					<Card withBorder>
						<Stack align="center" py="xl">
							<IconCopy size={48} color="gray" />
							<Text c="dimmed">No duplicate files detected.</Text>
							<Text size="sm" c="dimmed">
								Run a scan to check for duplicates in your library.
							</Text>
							<Button
								variant="light"
								leftSection={<IconSearch size={16} />}
								onClick={() => scanMutation.mutate()}
								loading={scanMutation.isPending}
							>
								Scan Now
							</Button>
						</Stack>
					</Card>
				)}
			</Stack>
		</Box>
	);
}
