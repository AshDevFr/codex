import {
	Card,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { MediaCard } from "@/components/library/MediaCard";
import type { Book, Series } from "@/types/api";

interface RecommendedSectionProps {
	libraryId: string;
}

export function RecommendedSection({ libraryId }: RecommendedSectionProps) {
	// Fetch books with reading progress
	const { data: inProgressBooks, isLoading: loadingInProgress } = useQuery({
		queryKey: ["books", "in-progress", libraryId],
		queryFn: () => booksApi.getInProgress(libraryId),
	});

	// Fetch recently added books
	const { data: recentlyAddedBooks, isLoading: loadingRecent } = useQuery({
		queryKey: ["books", "recently-added", libraryId],
		queryFn: () => booksApi.getRecentlyAdded(libraryId, 50),
	});

	// Fetch started series for "On Deck"
	const { data: startedSeries, isLoading: loadingStarted } = useQuery({
		queryKey: ["series", "started", libraryId],
		queryFn: () => seriesApi.getStarted(libraryId),
	});

	return (
		<Stack gap="xl">
			{/* Keep Reading Section */}
			{inProgressBooks && inProgressBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Keep Reading</Title>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{inProgressBooks.map((book) => (
							<MediaCard key={book.id} type="book" data={book} showProgress />
						))}
					</div>
				</Stack>
			)}

			{/* On Deck Section */}
			{startedSeries && startedSeries.length > 0 && (
				<Stack gap="md">
					<Title order={2}>On Deck</Title>
					<Text size="sm" c="dimmed">
						Continue reading these series
					</Text>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{startedSeries.map((series) => (
							<MediaCard key={series.id} type="series" data={series} />
						))}
					</div>
				</Stack>
			)}

			{/* Recently Added Books */}
			{recentlyAddedBooks && recentlyAddedBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Added</Title>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{recentlyAddedBooks.map((book) => (
							<MediaCard key={book.id} type="book" data={book} />
						))}
					</div>
				</Stack>
			)}

			{/* Empty state */}
			{!loadingInProgress &&
				!loadingRecent &&
				!loadingStarted &&
				!inProgressBooks?.length &&
				!recentlyAddedBooks?.length &&
				!startedSeries?.length && (
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

