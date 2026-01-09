import {
	ActionIcon,
	Card,
	Group,
	Menu,
	SimpleGrid,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import { IconAnalyze, IconDots } from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
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
				{showProgress && (
					<Text size="xs" c="blue">
						Continue reading
					</Text>
				)}
			</Stack>
		</Card>
	);
}

function SeriesCard({ series }: { series: Series }) {
	const queryClient = useQueryClient();

	const analyzeMutation = useMutation({
		mutationFn: () => seriesApi.analyze(series.id),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "All books in series queued for analysis",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start series analysis",
				color: "red",
			});
		},
	});

	const analyzeUnanalyzedMutation = useMutation({
		mutationFn: () => seriesApi.analyzeUnanalyzed(series.id),
		onSuccess: () => {
			notifications.show({
				title: "Analysis started",
				message: "Unanalyzed books queued for analysis",
				color: "blue",
			});
			queryClient.invalidateQueries({ queryKey: ["series"] });
		},
		onError: (error: Error) => {
			notifications.show({
				title: "Analysis failed",
				message: error.message || "Failed to start analysis",
				color: "red",
			});
		},
	});

	return (
		<Card shadow="sm" padding="lg" radius="md" withBorder>
			<Stack gap="xs">
				<Group justify="space-between" align="flex-start">
					<Text fw={500} lineClamp={2} style={{ flex: 1 }}>
						{series.name}
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
								{analyzeMutation.isPending ? "Analyzing..." : "Analyze All"}
							</Menu.Item>
							<Menu.Item
								leftSection={<IconAnalyze size={14} />}
								onClick={() => analyzeUnanalyzedMutation.mutate()}
								disabled={analyzeUnanalyzedMutation.isPending}
							>
								{analyzeUnanalyzedMutation.isPending
									? "Analyzing..."
									: "Analyze Unanalyzed"}
							</Menu.Item>
						</Menu.Dropdown>
					</Menu>
				</Group>
			</Stack>
		</Card>
	);
}
