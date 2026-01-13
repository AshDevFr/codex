import { Box, Card, Group, Pagination, Skeleton, Stack, Text } from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { booksApi } from "@/api/books";
import { MediaCard } from "@/components/library/MediaCard";
import { useBookFilterState } from "@/hooks/useBookFilterState";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";

/** Fixed skeleton IDs to avoid array index keys */
const SKELETON_IDS = ["b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8", "b9", "b10", "b11", "b12"];

/** Skeleton placeholder for loading state */
function BooksGridSkeleton({ count = 12 }: { count?: number }) {
	const ids = SKELETON_IDS.slice(0, count);
	return (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
				gap: "var(--mantine-spacing-md)",
				width: "100%",
			}}
		>
			{ids.map((id) => (
				<Box key={id}>
					<Skeleton height={225} radius="md" mb="xs" />
					<Skeleton height={16} width="80%" radius="sm" />
				</Box>
			))}
		</div>
	);
}

interface BooksSectionProps {
	libraryId: string;
	searchParams: URLSearchParams;
	onTotalChange?: (total: number) => void;
}

export function BooksSection({ libraryId, searchParams, onTotalChange }: BooksSectionProps) {
	const navigate = useNavigate();
	const filterState = useBookFilterState();

	// Get show deleted preference from user preferences store
	const showDeletedBooks = useUserPreferencesStore((state) =>
		state.getPreference("library.show_deleted_books"),
	);

	// Read query parameters (URL uses 1-indexed pages for user-friendly URLs)
	const page = parseInt(searchParams.get("page") || "1", 10);
	const pageSize = parseInt(searchParams.get("pageSize") || "20", 10);
	const sort = searchParams.get("sort") || "title,asc";

	// Debounce the filter condition to avoid rapid API calls when clicking multiple chips
	const [debouncedCondition] = useDebouncedValue(filterState.condition, 150);
	const [debouncedIncludeDeleted] = useDebouncedValue(showDeletedBooks, 150);

	// Serialize the condition for use as a query key (stable string representation)
	const conditionKey = useMemo(() => {
		if (!debouncedCondition) return "none";
		return JSON.stringify(debouncedCondition);
	}, [debouncedCondition]);

	// Fetch books data using the search endpoint with conditions
	const { data: booksData, isLoading } = useQuery({
		queryKey: ["books", "search", libraryId, page, pageSize, sort, conditionKey, debouncedIncludeDeleted],
		queryFn: () =>
			booksApi.search(libraryId, {
				condition: debouncedCondition,
				page: page - 1, // Convert to 0-indexed for backend
				pageSize,
				sort,
				includeDeleted: debouncedIncludeDeleted,
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

	const totalPages = booksData ? Math.ceil(booksData.total / booksData.pageSize) : 1;

	const showPagination = booksData ? booksData.total > pageSize : false;

	// Notify parent of total count change
	useEffect(() => {
		if (booksData && onTotalChange) {
			onTotalChange(booksData.total);
		}
	}, [booksData, onTotalChange]);

	return (
		<Stack gap="md">
			{/* Books Grid */}
			{isLoading ? (
				<BooksGridSkeleton count={pageSize > 12 ? 12 : pageSize} />
			) : booksData?.data && booksData.data.length > 0 ? (
				<>
					{/* Top Pagination */}
					{showPagination && (
						<Group justify="center">
							<Pagination value={page} onChange={handlePageChange} total={totalPages} />
						</Group>
					)}

					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{booksData.data.map((book) => (
							<MediaCard key={book.id} type="book" data={book} />
						))}
					</div>

					{/* Bottom Pagination */}
					{showPagination && (
						<Group justify="center" mt="xl">
							<Pagination value={page} onChange={handlePageChange} total={totalPages} />
						</Group>
					)}

					{/* Results info */}
					<Text size="sm" c="dimmed" ta="center">
						Showing {(page - 1) * pageSize + 1} to {Math.min(page * pageSize, booksData.total)} of{" "}
						{booksData.total} books
					</Text>
				</>
			) : (
				<Card p="xl" withBorder>
					<Stack align="center" gap="sm">
						<Text size="lg" fw={600}>
							No books found
						</Text>
						<Text size="sm" c="dimmed">
							{filterState.hasActiveFilters
								? "Try adjusting your filters"
								: "This library doesn't have any books yet"}
						</Text>
					</Stack>
				</Card>
			)}
		</Stack>
	);
}
