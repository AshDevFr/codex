import {
	Card,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { booksApi } from "@/api/books";
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
	const { data: recentlyAddedBooks, isLoading: loadingRecent } = useQuery({
		queryKey: ["books", "recently-added", libraryId],
		queryFn: () => booksApi.getRecentlyAdded(libraryId, 50),
	});

	// Fetch on-deck books (next unread book in series where user has completed books)
	const { data: onDeckResponse, isLoading: loadingOnDeck } = useQuery({
		queryKey: ["books", "on-deck", libraryId],
		queryFn: () => booksApi.getOnDeck(libraryId),
	});

	const onDeckBooks = onDeckResponse?.data ?? [];

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
				!loadingOnDeck &&
				!inProgressBooks?.length &&
				!recentlyAddedBooks?.length &&
				!onDeckBooks.length && (
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

