import { Stack, Title, Text, SimpleGrid, Card, Group } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import type { Book } from "@/types/api";

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
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4, xl: 5 }}>
						{inProgressBooks.map((book) => (
							<BookCard key={book.id} book={book} showProgress />
						))}
					</SimpleGrid>
				</Stack>
			)}

			{/* On Deck Section */}
			{startedSeries && startedSeries.length > 0 && (
				<Stack gap="md">
					<Title order={2}>On Deck</Title>
					<Text size="sm" c="dimmed">
						Continue reading these series
					</Text>
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4 }}>
						{startedSeries.map((series) => (
							<SeriesCard key={series.id} series={series} />
						))}
					</SimpleGrid>
				</Stack>
			)}

			{/* Recently Added Books */}
			{recentlyAddedBooks && recentlyAddedBooks.length > 0 && (
				<Stack gap="md">
					<Title order={2}>Recently Added</Title>
					<SimpleGrid cols={{ base: 1, sm: 2, md: 3, lg: 4, xl: 5 }}>
						{recentlyAddedBooks.map((book) => (
							<BookCard key={book.id} book={book} />
						))}
					</SimpleGrid>
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

// Placeholder components - these should be implemented properly
function BookCard({
	book,
	showProgress,
}: {
	book: Book;
	showProgress?: boolean;
}) {
	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Stack gap="xs">
				<Text fw={500} lineClamp={2}>
					{book.title}
				</Text>
				{showProgress && (
					<Text size="xs" c="blue">
						Continue reading
					</Text>
				)}
				{book.chapter_number && (
					<Text size="xs" c="dimmed">
						Chapter {book.chapter_number}
					</Text>
				)}
			</Stack>
		</Card>
	);
}

function SeriesCard({ series }: { series: { id: string; name: string } }) {
	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Group justify="space-between">
				<Text fw={500} lineClamp={2}>
					{series.name}
				</Text>
			</Group>
		</Card>
	);
}
