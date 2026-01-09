import {
	Card,
	Group,
	Pagination,
	Select,
	SimpleGrid,
	Stack,
	Text,
	TextInput,
} from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { IconSearch } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
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

	// Read query parameters
	const page = parseInt(searchParams.get("page") || "1");
	const pageSize = parseInt(searchParams.get("pageSize") || "20");
	const sort = searchParams.get("sort") || "title,asc";
	const seriesFilter = searchParams.get("series") || "";
	const genreFilter = searchParams.get("genre") || "";

	// Local search state
	const [searchQuery, setSearchQuery] = useState("");
	const [debouncedSearch] = useDebouncedValue(searchQuery, 300);

	// Fetch books data
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
				page,
				pageSize,
				sort,
				series_id: seriesFilter,
				genre: genreFilter,
			}),
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
		? Math.ceil(booksData.total / booksData.page_size)
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
			) : booksData && booksData.items.length > 0 ? (
				<>
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4, xl: 5 }}>
						{booksData.items.map((book) => (
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
	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Stack gap="xs">
				<Text fw={500} lineClamp={2}>
					{book.title}
				</Text>
				{book.chapter_number && (
					<Text size="xs" c="dimmed">
						Chapter {book.chapter_number}
					</Text>
				)}
				{book.writer && (
					<Text size="xs" c="dimmed" lineClamp={1}>
						{book.writer}
					</Text>
				)}
				{book.page_count && (
					<Text size="xs" c="dimmed">
						{book.page_count} pages
					</Text>
				)}
			</Stack>
		</Card>
	);
}
