import {
	Box,
	Card,
	Group,
	Pagination,
	Skeleton,
	Stack,
	Text,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { seriesApi } from "@/api/series";
import { ActiveFilters } from "@/components/library/ActiveFilters";
import { MediaCard } from "@/components/library/MediaCard";
import { useFilterState } from "@/hooks/useFilterState";

/** Fixed skeleton IDs to avoid array index keys */
const SKELETON_IDS = [
	"s1",
	"s2",
	"s3",
	"s4",
	"s5",
	"s6",
	"s7",
	"s8",
	"s9",
	"s10",
	"s11",
	"s12",
];

/** Skeleton placeholder for loading state */
function SeriesGridSkeleton({ count = 12 }: { count?: number }) {
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

interface SeriesSectionProps {
	libraryId: string;
	searchParams: URLSearchParams;
	onTotalChange?: (total: number) => void;
}

export function SeriesSection({
	libraryId,
	searchParams,
	onTotalChange,
}: SeriesSectionProps) {
	const navigate = useNavigate();

	// Get filter state from URL (uses the advanced filtering system)
	// Filters are only applied when user clicks "Apply" in FilterPanel,
	// so no debouncing is needed here
	const { condition, filters, hasActiveFilters } = useFilterState();

	// Read query parameters (URL uses 1-indexed pages for user-friendly URLs)
	const page = parseInt(searchParams.get("page") || "1", 10);
	const pageSize = parseInt(searchParams.get("pageSize") || "20", 10);
	const sort = searchParams.get("sort") || "name,asc";

	// Serialize filter state for use in query key (stable reference)
	// We include the modes to ensure mode changes trigger a refetch even when
	// the condition is semantically identical (e.g., "any" vs "all" with one value)
	const filterKey = useMemo(() => {
		const modes = {
			genres: filters.genres.mode,
			tags: filters.tags.mode,
			status: filters.status.mode,
			readStatus: filters.readStatus.mode,
			publisher: filters.publisher.mode,
			language: filters.language.mode,
			sharingTags: filters.sharingTags.mode,
		};
		return condition ? JSON.stringify({ condition, modes }) : "none";
	}, [condition, filters]);

	// Fetch series data using the new POST search endpoint
	const { data: seriesData, isLoading } = useQuery({
		queryKey: [
			"series",
			"search",
			libraryId,
			page,
			pageSize,
			sort,
			filterKey,
		],
		queryFn: () =>
			seriesApi.search(libraryId, {
				condition,
				page: page - 1, // Convert to 0-indexed for backend
				pageSize,
				sort,
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

	const totalPages = seriesData
		? Math.ceil(seriesData.total / seriesData.pageSize)
		: 1;

	const showPagination = seriesData ? seriesData.total > pageSize : false;

	// Notify parent of total count change
	useEffect(() => {
		if (seriesData && onTotalChange) {
			onTotalChange(seriesData.total);
		}
	}, [seriesData, onTotalChange]);

	return (
		<Stack gap="md">
			{/* Active Filters Summary */}
			{hasActiveFilters && <ActiveFilters />}

			{/* Series Grid */}
			{isLoading ? (
				<SeriesGridSkeleton count={pageSize > 12 ? 12 : pageSize} />
			) : seriesData?.data && seriesData.data.length > 0 ? (
				<>
					{/* Top Pagination */}
					{showPagination && (
						<Group justify="center">
							<Pagination
								value={page}
								onChange={handlePageChange}
								total={totalPages}
							/>
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
						{seriesData.data.map((series) => (
							<MediaCard key={series.id} type="series" data={series} />
						))}
					</div>

					{/* Bottom Pagination */}
					{showPagination && (
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
