import {
	ActionIcon,
	Alert,
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
import { useState } from "react";
import { api } from "@/api/client";
import { type DuplicateGroup, duplicatesApi } from "@/api/duplicates";
import type { Book } from "@/types";

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
					<div>
						<Text fw={500}>{group.duplicate_count} Duplicates</Text>
						<Text size="xs" c="dimmed" style={{ fontFamily: "monospace" }}>
							{group.file_hash.slice(0, 16)}...
						</Text>
					</div>
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
				<Table>
					<Table.Thead>
						<Table.Tr>
							<Table.Th>Book</Table.Th>
							<Table.Th>Series</Table.Th>
							<Table.Th>Path</Table.Th>
							<Table.Th>Size</Table.Th>
						</Table.Tr>
					</Table.Thead>
					<Table.Tbody>
						{books.map((book) => (
							<Table.Tr key={book.id}>
								<Table.Td>
									<Text size="sm" fw={500}>
										{book.title}
									</Text>
								</Table.Td>
								<Table.Td>
									<Text size="sm">{book.seriesName || "-"}</Text>
								</Table.Td>
								<Table.Td>
									<Tooltip label={book.filePath}>
										<Text size="sm" lineClamp={1} style={{ maxWidth: 300 }}>
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

	// Fetch duplicates
	const {
		data: duplicates,
		isLoading,
		error,
	} = useQuery({
		queryKey: ["duplicates"],
		queryFn: duplicatesApi.list,
	});

	// Fetch book details for a group
	const fetchBooksForGroup = async (group: DuplicateGroup): Promise<Book[]> => {
		const cached = bookDetailsCache.get(group.id);
		if (cached) return cached;

		const books: Book[] = [];
		for (const bookId of group.book_ids) {
			try {
				const response = await api.get<Book>(`/books/${bookId}`);
				books.push(response.data);
			} catch {
				// Book might have been deleted
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
					<div>
						<Title order={1}>Duplicate Detection</Title>
						<Text c="dimmed" size="sm">
							Find and manage duplicate files in your library
						</Text>
					</div>
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
						<div style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{duplicates?.length || 0}
							</Text>
							<Text size="sm" c="dimmed">
								Duplicate Groups
							</Text>
						</div>
						<div style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{totalDuplicates}
							</Text>
							<Text size="sm" c="dimmed">
								Total Duplicates
							</Text>
						</div>
						<div style={{ textAlign: "center" }}>
							<Text size="xl" fw={700}>
								{totalDuplicates - (duplicates?.length || 0)}
							</Text>
							<Text size="sm" c="dimmed">
								Redundant Copies
							</Text>
						</div>
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
