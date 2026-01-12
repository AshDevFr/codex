import { Box, Card, Loader, Stack, Text, Title } from "@mantine/core";
import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router-dom";
import { searchApi } from "@/api/search";
import { HorizontalCarousel } from "@/components/library/HorizontalCarousel";
import { MediaCard } from "@/components/library/MediaCard";

export function SearchResults() {
	const [searchParams] = useSearchParams();
	const query = searchParams.get("q") ?? "";

	const { data, isLoading, error } = useQuery({
		queryKey: ["search", "results", query],
		queryFn: () =>
			searchApi.search({
				query,
				limit: 50, // Show more results on the full page
			}),
		enabled: query.trim().length >= 2,
	});

	// Defensive checks for undefined results
	const series = data?.series ?? [];
	const books = data?.books ?? [];
	const hasResults = series.length > 0 || books.length > 0;

	if (!query || query.trim().length < 2) {
		return (
			<Box py="xl" px="md">
				<Stack gap="xl">
					<Title order={1}>Search</Title>
					<Card p="xl" withBorder>
						<Stack align="center" gap="sm">
							<Text size="lg" fw={600}>
								Enter a search term
							</Text>
							<Text size="sm" c="dimmed">
								Search for series or books by name
							</Text>
						</Stack>
					</Card>
				</Stack>
			</Box>
		);
	}

	return (
		<Box py="xl" px="md">
			<Stack gap="xl">
				<Title order={1}>Search results for "{query}"</Title>

				{isLoading && (
					<Card p="xl" withBorder>
						<Stack align="center" gap="sm">
							<Loader size="lg" />
							<Text size="sm" c="dimmed">
								Searching...
							</Text>
						</Stack>
					</Card>
				)}

				{error && (
					<Card p="xl" withBorder>
						<Stack align="center" gap="sm">
							<Text size="lg" fw={600} c="red">
								Search failed
							</Text>
							<Text size="sm" c="dimmed">
								{(error as Error).message ??
									"An error occurred while searching"}
							</Text>
						</Stack>
					</Card>
				)}

				{!isLoading && !error && !hasResults && (
					<Card p="xl" withBorder>
						<Stack align="center" gap="sm">
							<Text size="lg" fw={600}>
								No results found
							</Text>
							<Text size="sm" c="dimmed">
								Try a different search term
							</Text>
						</Stack>
					</Card>
				)}

				{!isLoading && hasResults && (
					<>
						{/* Series Results */}
						{series.length > 0 && (
							<HorizontalCarousel
								title="Series"
								subtitle={`${series.length} result${series.length !== 1 ? "s" : ""}`}
							>
								{series.map((s) => (
									<MediaCard key={s.id} type="series" data={s} />
								))}
							</HorizontalCarousel>
						)}

						{/* Book Results */}
						{books.length > 0 && (
							<HorizontalCarousel
								title="Books"
								subtitle={`${books.length} result${books.length !== 1 ? "s" : ""}`}
							>
								{books.map((book) => (
									<MediaCard key={book.id} type="book" data={book} />
								))}
							</HorizontalCarousel>
						)}
					</>
				)}
			</Stack>
		</Box>
	);
}
