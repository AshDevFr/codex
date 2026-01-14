import {
	ActionIcon,
	Box,
	Center,
	Drawer,
	Loader,
	ScrollArea,
	Stack,
	Text,
	TextInput,
	Tooltip,
} from "@mantine/core";
import { useDebouncedCallback } from "@mantine/hooks";
import { IconSearch, IconX } from "@tabler/icons-react";
import { useCallback, useState } from "react";

export interface SearchResult {
	/** CFI location of the match */
	cfi: string;
	/** Text excerpt containing the match */
	excerpt: string;
	/** Chapter/section where the match was found */
	chapter?: string;
}

interface EpubSearchProps {
	/** Whether the search drawer is open */
	opened: boolean;
	/** Callback to toggle the drawer */
	onToggle: () => void;
	/** Callback to perform search - returns results */
	onSearch: (query: string) => Promise<SearchResult[]>;
	/** Callback when a search result is clicked */
	onNavigate: (cfi: string) => void;
}

/**
 * Highlight search term in text by wrapping matches in <mark> tags
 */
function highlightText(text: string, query: string): React.ReactNode {
	if (!query.trim()) return text;

	const regex = new RegExp(
		`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
		"gi",
	);
	const parts = text.split(regex);

	return parts.map((part, index) =>
		regex.test(part) ? (
			<mark
				// biome-ignore lint/suspicious/noArrayIndexKey: Split string parts have stable order
				key={index}
				style={{ backgroundColor: "var(--mantine-color-yellow-3)", padding: 0 }}
			>
				{part}
			</mark>
		) : (
			part
		),
	);
}

/**
 * Search drawer for EPUB reader.
 *
 * Provides full-text search within the EPUB document.
 * Shows results with highlighted excerpts and chapter information.
 */
export function EpubSearch({
	opened,
	onToggle,
	onSearch,
	onNavigate,
}: EpubSearchProps) {
	const [query, setQuery] = useState("");
	const [results, setResults] = useState<SearchResult[]>([]);
	const [isSearching, setIsSearching] = useState(false);
	const [hasSearched, setHasSearched] = useState(false);

	// Debounced search to avoid excessive API calls
	const debouncedSearch = useDebouncedCallback(async (searchQuery: string) => {
		if (!searchQuery.trim()) {
			setResults([]);
			setHasSearched(false);
			setIsSearching(false);
			return;
		}

		setIsSearching(true);
		setHasSearched(true);

		try {
			const searchResults = await onSearch(searchQuery);
			setResults(searchResults);
		} catch (error) {
			console.error("Search failed:", error);
			setResults([]);
		} finally {
			setIsSearching(false);
		}
	}, 300);

	const handleQueryChange = useCallback(
		(value: string) => {
			setQuery(value);
			debouncedSearch(value);
		},
		[debouncedSearch],
	);

	const handleClearQuery = useCallback(() => {
		setQuery("");
		setResults([]);
		setHasSearched(false);
	}, []);

	const handleResultClick = useCallback(
		(cfi: string) => {
			onNavigate(cfi);
			onToggle(); // Close drawer after navigation
		},
		[onNavigate, onToggle],
	);

	return (
		<>
			{/* Toggle button */}
			<Tooltip label="Search (Ctrl+F)" position="bottom">
				<ActionIcon
					variant="subtle"
					color="gray"
					size="lg"
					onClick={onToggle}
					aria-label="Search"
				>
					<IconSearch size={20} />
				</ActionIcon>
			</Tooltip>

			{/* Search Drawer */}
			<Drawer
				opened={opened}
				onClose={onToggle}
				title="Search"
				position="right"
				size="sm"
			>
				<Stack gap="md">
					{/* Search input */}
					<TextInput
						placeholder="Search in book..."
						value={query}
						onChange={(e) => handleQueryChange(e.target.value)}
						leftSection={<IconSearch size={16} />}
						rightSection={
							query && (
								<ActionIcon
									variant="subtle"
									size="sm"
									onClick={handleClearQuery}
									aria-label="Clear search"
								>
									<IconX size={14} />
								</ActionIcon>
							)
						}
						autoFocus
					/>

					{/* Results */}
					<ScrollArea h="calc(100vh - 160px)">
						{isSearching ? (
							<Center py="xl">
								<Loader size="sm" />
							</Center>
						) : hasSearched && results.length === 0 ? (
							<Text c="dimmed" size="sm" ta="center" py="xl">
								No results found for "{query}"
							</Text>
						) : results.length > 0 ? (
							<Stack gap="xs">
								<Text size="xs" c="dimmed">
									{results.length} result{results.length !== 1 ? "s" : ""} found
								</Text>
								{results.map((result, index) => (
									<Box
										key={`${result.cfi}-${index}`}
										p="sm"
										style={{
											borderRadius: "var(--mantine-radius-sm)",
											border: "1px solid var(--mantine-color-dark-4)",
											cursor: "pointer",
										}}
										onClick={() => handleResultClick(result.cfi)}
									>
										{result.chapter && (
											<Text size="xs" c="dimmed" mb={4}>
												{result.chapter}
											</Text>
										)}
										<Text size="sm" lineClamp={3}>
											{highlightText(result.excerpt, query)}
										</Text>
									</Box>
								))}
							</Stack>
						) : (
							<Text c="dimmed" size="sm" ta="center" py="xl">
								Enter a search term to find text in the book
							</Text>
						)}
					</ScrollArea>
				</Stack>
			</Drawer>
		</>
	);
}
