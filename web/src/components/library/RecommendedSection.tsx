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

interface RecommendedSectionProps {
	libraryId: string;
}

export function RecommendedSection({ libraryId }: RecommendedSectionProps) {
	// Fetch books with reading progress (Keep Reading)
	const { data: inProgressBooks, isLoading: loadingInProgress } = useQuery({
		queryKey: ["books", "in-progress", libraryId],
		queryFn: () => booksApi.getInProgress(libraryId),
	});

	// Fetch recently added books
	const { data: recentlyAddedBooks, isLoading: loadingRecentBooks } = useQuery({
		queryKey: ["books", "recently-added", libraryId],
		queryFn: () => booksApi.getRecentlyAdded(libraryId, 50),
	});

	// Fetch on-deck books (next unread book in series where user has completed books)
	const { data: onDeckResponse, isLoading: loadingOnDeck } = useQuery({
		queryKey: ["books", "on-deck", libraryId],
		queryFn: () => booksApi.getOnDeck(libraryId),
	});

	// Fetch recently added series
	const { data: recentlyAddedSeries, isLoading: loadingRecentSeries } = useQuery({
		queryKey: ["series", "recently-added", libraryId],
		queryFn: () => seriesApi.getRecentlyAdded(libraryId, 50),
	});

	// Fetch recently updated series
	const { data: recentlyUpdatedSeries, isLoading: loadingUpdatedSeries } = useQuery({
		queryKey: ["series", "recently-updated", libraryId],
		queryFn: () => seriesApi.getRecentlyUpdated(libraryId, 50),
	});

	// Fetch recently read books
	const { data: recentlyReadBooks, isLoading: loadingRecentlyRead } = useQuery({
		queryKey: ["books", "recently-read", libraryId],
		queryFn: () => booksApi.getRecentlyRead(libraryId, 50),
	});

	const onDeckBooks = onDeckResponse?.data ?? [];

	const isLoading = loadingInProgress || loadingRecentBooks || loadingOnDeck ||
		loadingRecentSeries || loadingUpdatedSeries || loadingRecentlyRead;

	const hasContent = (inProgressBooks?.length ?? 0) > 0 ||
		(recentlyAddedBooks?.length ?? 0) > 0 ||
		onDeckBooks.length > 0 ||
		(recentlyAddedSeries?.length ?? 0) > 0 ||
		(recentlyUpdatedSeries?.length ?? 0) > 0 ||
		(recentlyReadBooks?.length ?? 0) > 0;

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
			{onDeckBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>On Deck</Title>
					<Text size="sm" c="dimmed">
						Next book in series you've been reading
					</Text>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{onDeckBooks.map((book) => (
							<MediaCard key={book.id} type="book" data={book} />
						))}
					</div>
				</Stack>
			)}

			{/* Recently Added Books */}
			{recentlyAddedBooks && recentlyAddedBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Added Books</Title>
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

			{/* Recently Added Series */}
			{recentlyAddedSeries && recentlyAddedSeries.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Added Series</Title>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{recentlyAddedSeries.map((series) => (
							<MediaCard key={series.id} type="series" data={series} />
						))}
					</div>
				</Stack>
			)}

			{/* Recently Updated Series */}
			{recentlyUpdatedSeries && recentlyUpdatedSeries.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Updated Series</Title>
					<Text size="sm" c="dimmed">
						Series with new or updated content
					</Text>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{recentlyUpdatedSeries.map((series) => (
							<MediaCard key={series.id} type="series" data={series} />
						))}
					</div>
				</Stack>
			)}

			{/* Recently Read Books */}
			{recentlyReadBooks && recentlyReadBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Read Books</Title>
					<Text size="sm" c="dimmed">
						Books you've read recently
					</Text>
					<div
						style={{
							display: "grid",
							gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
							gap: "var(--mantine-spacing-md)",
							width: "100%",
						}}
					>
						{recentlyReadBooks.map((book) => (
							<MediaCard key={book.id} type="book" data={book} showProgress />
						))}
					</div>
				</Stack>
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
