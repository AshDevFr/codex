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
import { seriesApi } from "@/api/series";
import type { Series } from "@/types/api";

interface SeriesSectionProps {
	libraryId: string;
	searchParams: URLSearchParams;
}

export function SeriesSection({
	libraryId,
	searchParams,
}: SeriesSectionProps) {
	const navigate = useNavigate();

	// Read query parameters
	const page = parseInt(searchParams.get("page") || "1");
	const pageSize = parseInt(searchParams.get("pageSize") || "20");
	const sort = searchParams.get("sort") || "name,asc";
	const genreFilter = searchParams.get("genre") || "";
	const statusFilter = searchParams.get("status") || "";

	// Local search state
	const [searchQuery, setSearchQuery] = useState("");
	const [debouncedSearch] = useDebouncedValue(searchQuery, 300);

	// Fetch series data
	const { data: seriesData, isLoading } = useQuery({
		queryKey: [
			"series",
			libraryId,
			page,
			pageSize,
			sort,
			genreFilter,
			statusFilter,
			debouncedSearch,
		],
		queryFn: () =>
			seriesApi.getByLibrary(libraryId, {
				page,
				pageSize,
				sort,
				genre: genreFilter,
				status: statusFilter,
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

	const totalPages = seriesData
		? Math.ceil(seriesData.total / seriesData.page_size)
		: 1;

	return (
		<Stack gap="md">
			{/* Filter Bar */}
			<Group>
				<TextInput
					placeholder="Search series..."
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
						{ value: "name,asc", label: "Name (A-Z)" },
						{ value: "name,desc", label: "Name (Z-A)" },
						{ value: "created_at,desc", label: "Recently Added" },
						{ value: "book_count,desc", label: "Most Books" },
						{ value: "year,desc", label: "Year (Newest)" },
						{ value: "year,asc", label: "Year (Oldest)" },
					]}
					style={{ minWidth: 180 }}
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

			{/* Series Grid */}
			{isLoading ? (
				<Text c="dimmed">Loading series...</Text>
			) : seriesData && seriesData.items.length > 0 ? (
				<>
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4 }}>
						{seriesData.items.map((series) => (
							<SeriesCard key={series.id} series={series} />
						))}
					</SimpleGrid>

					{/* Pagination */}
					{totalPages > 1 && (
						<Group justify="center" mt="xl">
							<Pagination
								value={page}
								onChange={handlePageChange}
								total={totalPages}
							/>
						</Group>
					)}

					{/* Results info */}
					<Text size="sm" c="dimmed" ta="center">
						Showing {(page - 1) * pageSize + 1} to{" "}
						{Math.min(page * pageSize, seriesData.total)} of {seriesData.total}{" "}
						series
					</Text>
				</>
			) : (
				<Card p="xl" withBorder>
					<Stack align="center" gap="sm">
						<Text size="lg" fw={600}>
							No series found
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

// Placeholder SeriesCard component
function SeriesCard({ series }: { series: Series }) {
	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Stack gap="xs">
				<Text fw={500} lineClamp={2}>
					{series.name}
				</Text>
				{series.publisher && (
					<Text size="xs" c="dimmed">
						{series.publisher}
					</Text>
				)}
				{series.book_count !== undefined && (
					<Text size="xs" c="dimmed">
						{series.book_count} book{series.book_count !== 1 ? "s" : ""}
					</Text>
				)}
				{series.year && (
					<Text size="xs" c="dimmed">
						{series.year}
					</Text>
				)}
			</Stack>
		</Card>
	);
}
