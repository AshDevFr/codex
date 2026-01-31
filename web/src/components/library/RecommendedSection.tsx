import { Card, Stack, Text } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useCallback } from "react";

import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { HorizontalCarousel } from "@/components/library/HorizontalCarousel";
import { MediaCard } from "@/components/library/MediaCard";
import {
	selectCanSelectType,
	selectIsSelectionMode,
	useBulkSelectionStore,
} from "@/store/bulkSelectionStore";

const MAX_ITEMS_PER_SECTION = 20;

interface RecommendedSectionProps {
	libraryId: string;
}

export function RecommendedSection({ libraryId }: RecommendedSectionProps) {
	// Bulk selection state - use stable selectors to minimize re-renders
	const isSelectionMode = useBulkSelectionStore(selectIsSelectionMode);
	const canSelectBooks = useBulkSelectionStore(selectCanSelectType("book"));
	const canSelectSeries = useBulkSelectionStore(selectCanSelectType("series"));
	const toggleSelection = useBulkSelectionStore(
		(state) => state.toggleSelection,
	);
	// Get the Set directly for O(1) lookups - only re-renders when the Set changes
	const selectedIds = useBulkSelectionStore((state) => state.selectedIds);

	// Handle selection for books
	const handleBookSelect = useCallback(
		(id: string, _shiftKey: boolean) => {
			// TODO: Implement shift+click range selection
			toggleSelection(id, "book");
		},
		[toggleSelection],
	);

	// Handle selection for series
	const handleSeriesSelect = useCallback(
		(id: string, _shiftKey: boolean) => {
			// TODO: Implement shift+click range selection
			toggleSelection(id, "series");
		},
		[toggleSelection],
	);

	// Fetch books with reading progress (Keep Reading)
	const { data: inProgressBooks, isLoading: loadingInProgress } = useQuery({
		queryKey: ["books", "in-progress", libraryId],
		queryFn: () => booksApi.getInProgress(libraryId),
	});

	// Fetch recently added books
	const { data: recentlyAddedBooks, isLoading: loadingRecentBooks } = useQuery({
		queryKey: ["books", "recently-added", libraryId],
		queryFn: () => booksApi.getRecentlyAdded(libraryId, MAX_ITEMS_PER_SECTION),
	});

	// Fetch on-deck books (next unread book in series where user has completed books)
	const { data: onDeckResponse, isLoading: loadingOnDeck } = useQuery({
		queryKey: ["books", "on-deck", libraryId],
		queryFn: () => booksApi.getOnDeck(libraryId),
	});

	// Fetch recently added series
	const { data: recentlyAddedSeries, isLoading: loadingRecentSeries } =
		useQuery({
			queryKey: ["series", "recently-added", libraryId],
			queryFn: () =>
				seriesApi.getRecentlyAdded(libraryId, { limit: MAX_ITEMS_PER_SECTION }),
		});

	// Fetch recently updated series
	const { data: recentlyUpdatedSeries, isLoading: loadingUpdatedSeries } =
		useQuery({
			queryKey: ["series", "recently-updated", libraryId],
			queryFn: () =>
				seriesApi.getRecentlyUpdated(libraryId, {
					limit: MAX_ITEMS_PER_SECTION,
				}),
		});

	// Fetch recently read books
	const { data: recentlyReadBooks, isLoading: loadingRecentlyRead } = useQuery({
		queryKey: ["books", "recently-read", libraryId],
		queryFn: () => booksApi.getRecentlyRead(libraryId, MAX_ITEMS_PER_SECTION),
	});

	const onDeckBooks = (onDeckResponse?.data ?? []).slice(
		0,
		MAX_ITEMS_PER_SECTION,
	);
	const limitedInProgressBooks = (inProgressBooks?.data ?? []).slice(
		0,
		MAX_ITEMS_PER_SECTION,
	);

	const isLoading =
		loadingInProgress ||
		loadingRecentBooks ||
		loadingOnDeck ||
		loadingRecentSeries ||
		loadingUpdatedSeries ||
		loadingRecentlyRead;

	const limitedRecentlyAddedBooks = (recentlyAddedBooks?.data ?? []).slice(
		0,
		MAX_ITEMS_PER_SECTION,
	);

	const hasContent =
		limitedInProgressBooks.length > 0 ||
		limitedRecentlyAddedBooks.length > 0 ||
		onDeckBooks.length > 0 ||
		(recentlyAddedSeries?.length ?? 0) > 0 ||
		(recentlyUpdatedSeries?.length ?? 0) > 0 ||
		(recentlyReadBooks?.length ?? 0) > 0;

	return (
		<Stack gap="xl">
			{/* Keep Reading Section */}
			{limitedInProgressBooks.length > 0 && (
				<HorizontalCarousel title="Keep Reading">
					{limitedInProgressBooks.map((book) => (
						<MediaCard
							key={book.id}
							type="book"
							data={book}
							onSelect={handleBookSelect}
							isSelected={selectedIds.has(book.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectBooks}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* On Deck Section */}
			{onDeckBooks.length > 0 && (
				<HorizontalCarousel
					title="On Deck"
					subtitle="Next book in series you've been reading"
				>
					{onDeckBooks.map((book) => (
						<MediaCard
							key={book.id}
							type="book"
							data={book}
							onSelect={handleBookSelect}
							isSelected={selectedIds.has(book.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectBooks}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* Recently Added Books */}
			{limitedRecentlyAddedBooks.length > 0 && (
				<HorizontalCarousel title="Recently Added Books">
					{limitedRecentlyAddedBooks.map((book) => (
						<MediaCard
							key={book.id}
							type="book"
							data={book}
							onSelect={handleBookSelect}
							isSelected={selectedIds.has(book.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectBooks}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* Recently Added Series */}
			{recentlyAddedSeries && recentlyAddedSeries.length > 0 && (
				<HorizontalCarousel title="Recently Added Series">
					{recentlyAddedSeries.map((series) => (
						<MediaCard
							key={series.id}
							type="series"
							data={series}
							onSelect={handleSeriesSelect}
							isSelected={selectedIds.has(series.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectSeries}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* Recently Updated Series */}
			{recentlyUpdatedSeries && recentlyUpdatedSeries.length > 0 && (
				<HorizontalCarousel
					title="Recently Updated Series"
					subtitle="Series with new or updated content"
				>
					{recentlyUpdatedSeries.map((series) => (
						<MediaCard
							key={series.id}
							type="series"
							data={series}
							onSelect={handleSeriesSelect}
							isSelected={selectedIds.has(series.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectSeries}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* Recently Read Books */}
			{recentlyReadBooks && recentlyReadBooks.length > 0 && (
				<HorizontalCarousel
					title="Recently Read Books"
					subtitle="Books you've read recently"
				>
					{recentlyReadBooks.map((book) => (
						<MediaCard
							key={book.id}
							type="book"
							data={book}
							onSelect={handleBookSelect}
							isSelected={selectedIds.has(book.id)}
							isSelectionMode={isSelectionMode}
							canBeSelected={canSelectBooks}
						/>
					))}
				</HorizontalCarousel>
			)}

			{/* Empty state */}
			{!isLoading && !hasContent && (
				<Card p="xl" withBorder>
					<Stack align="center" gap="sm">
						<Text size="lg" fw={600}>
							No content available
						</Text>
						<Text size="sm" c="dimmed">
							Start scanning your library to see recommendations
						</Text>
					</Stack>
				</Card>
			)}
		</Stack>
	);
}
