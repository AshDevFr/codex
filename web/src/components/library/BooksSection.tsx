import {
	ActionIcon,
	Card,
	Group,
	Menu,
	Pagination,
	Select,
	SimpleGrid,
	Stack,
	Text,
	TextInput,
} from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import { IconAnalyze, IconDots, IconSearch } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import type { Book } from "@/types/api";

interface BooksSectionProps {
	libraryId: string;
	searchParams: URLSearchParams;
}

export function BooksSection({ libraryId, searchParams }: BooksSectionProps) {
	const navigate = useNavigate();

	// Read query parameters (URL uses 1-indexed pages for user-friendly URLs)
	const page = parseInt(searchParams.get("page") || "1", 10);
	const pageSize = parseInt(searchParams.get("pageSize") || "20", 10);
	const sort = searchParams.get("sort") || "title,asc";
	const seriesFilter = searchParams.get("series") || "";
	const genreFilter = searchParams.get("genre") || "";

	// Local search state
	const [searchQuery, setSearchQuery] = useState("");
	const [debouncedSearch] = useDebouncedValue(searchQuery, 300);

	// Fetch books data (convert to 0-indexed for backend)
	const { data: booksData, isLoading } = useQuery({
		queryKey: [
			"books",
			libraryId,
			page,
			pageSize,
			sort,
			seriesFilter,
			genreFilter,
			debouncedSearch,
		],
		queryFn: () =>
			booksApi.getByLibrary(libraryId, {
				page: page - 1, // Convert to 0-indexed for backend
				pageSize,
				sort,
				series_id: seriesFilter,
				genre: genreFilter,
			}),
		staleTime: 30000, // 30 seconds - shorter than global default
		refetchOnMount: true, // Always refetch when component mounts
	});

	// Update URL when filters change
	const handleFilterChange = (updates: Record<string, string | number>) => {
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

		navigate({ search: params.toString() }, { replace: true });
	};

	const handlePageChange = (newPage: number) => {
		handleFilterChange({ page: newPage });
	};

	const handleSortChange = (value: string | null) => {
		if (value) {
			handleFilterChange({ sort: value });
		}
	};

	const totalPages = booksData
		? Math.ceil(booksData.total / booksData.pageSize)
		: 1;

	return (
		<Stack gap="md">
			{/* Filter Bar */}
			<Group>
				<TextInput
					placeholder="Search books..."
					leftSection={<IconSearch size={16} />}
					value={searchQuery}
					onChange={(e) => setSearchQuery(e.currentTarget.value)}
					style={{ flex: 1, minWidth: 200 }}
				/>
				<Select
					label="Sort"
					value={sort}
					onChange={handleSortChange}
					data={[
						{ value: "title,asc", label: "Title (A-Z)" },
						{ value: "title,desc", label: "Title (Z-A)" },
						{ value: "created_at,desc", label: "Recently Added" },
						{ value: "release_date,desc", label: "Release Date (Newest)" },
						{ value: "release_date,asc", label: "Release Date (Oldest)" },
						{ value: "chapter_number,asc", label: "Chapter Number" },
					]}
					style={{ minWidth: 200 }}
				/>
				<Select
					label="Page Size"
					value={pageSize.toString()}
					onChange={(value) =>
						value && handleFilterChange({ pageSize: parseInt(value) })
					}
					data={[
						{ value: "20", label: "20 per page" },
						{ value: "50", label: "50 per page" },
						{ value: "100", label: "100 per page" },
						{ value: "500", label: "500 per page" },
					]}
					style={{ minWidth: 140 }}
				/>
			</Group>

	{/* Books Grid */}
	{isLoading ? (
		<Text c="dimmed">Loading books...</Text>
	) : booksData?.data && booksData.data.length > 0 ? (
				<>
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4, xl: 5 }}>
						{booksData.data.map((book) => (
							<BookCard key={book.id} book={book} />
						))}
					</SimpleGrid>

					{/* Pagination */}
					{totalPages > 1 && (
						<Group justify="center" mt="xl">
							<Pagination value={page} onChange={handlePageChange} total={totalPages} />
						</Group>
					)}

					{/* Results info */}
					<Text size="sm" c="dimmed" ta="center">
						Showing {(page - 1) * pageSize + 1} to{" "}
						{Math.min(page * pageSize, booksData.total)} of {booksData.total}{" "}
						books
					</Text>
				</>
			) : (
				<Card p="xl" withBorder>
					<Stack align="center" gap="sm">
						<Text size="lg" fw={600}>
							No books found
						</Text>
						<Text size="sm" c="dimmed">
							Try adjusting your filters or search query
						</Text>
					</Stack>
				</Card>
			)}
		</Stack>
	);
}

// Placeholder BookCard component
function BookCard({ book }: { book: Book }) {
	const queryClient = useQueryClient();

	const analyzeMutation = useMutation({
		mutationFn: () => booksApi.analyze(book.id),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Book analysis has been queued",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["books"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start book analysis",
				color: "red",
			});
		},
	});

	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Stack gap="xs">
				<Group justify="space-between" align="flex-start">
					<Text fw={500} lineClamp={1} c="dimmed" size="sm" style={{ flex: 1 }}>
						{book.seriesName}
					</Text>
					<Menu position="bottom-end" shadow="md" withinPortal>
						<Menu.Target>
							<ActionIcon variant="subtle" color="gray" size="sm">
								<IconDots size={16} />
							</ActionIcon>
						</Menu.Target>
						<Menu.Dropdown>
							<Menu.Item
								leftSection={<IconAnalyze size={14} />}
								onClick={() => analyzeMutation.mutate()}
								disabled={analyzeMutation.isPending}
							>
								{analyzeMutation.isPending ? "Analyzing..." : "Analyze"}
							</Menu.Item>
						</Menu.Dropdown>
					</Menu>
				</Group>
				<Text fw={600} lineClamp={2}>
					{book.number !== undefined && book.number !== null ? `${book.number} - ` : ""}
					{book.title}
				</Text>
				{book.pageCount && (
					<Text size="xs" c="dimmed">
						{book.pageCount} pages
					</Text>
				)}
				<Text size="xs" c="dimmed">
					{book.fileFormat.toUpperCase()}
				</Text>
			</Stack>
		</Card>
	);
}
