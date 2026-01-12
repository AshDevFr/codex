import { useDebouncedValue } from "@mantine/hooks";
import { useQuery } from "@tanstack/react-query";
import { type SearchResults, searchApi } from "@/api/search";

export interface UseSearchOptions {
	/** Minimum characters before search is triggered */
	minChars?: number;
	/** Debounce delay in milliseconds */
	debounceMs?: number;
	/** Maximum results per type */
	limit?: number;
	/** Optional library filter */
	libraryId?: string;
	/** Enable/disable the search */
	enabled?: boolean;
}

export interface UseSearchResult {
	results: SearchResults;
	isLoading: boolean;
	error: Error | null;
}

const emptyResults: SearchResults = {
	series: [],
	books: [],
};

export function useSearch(
	query: string,
	options: UseSearchOptions = {},
): UseSearchResult {
	const {
		minChars = 2,
		debounceMs = 300,
		limit = 10,
		libraryId,
		enabled = true,
	} = options;

	// Debounce the query
	const [debouncedQuery] = useDebouncedValue(query, debounceMs);

	// Only search if query meets minimum length
	const shouldSearch = enabled && debouncedQuery.trim().length >= minChars;

	const { data, isLoading, error } = useQuery({
		queryKey: ["search", debouncedQuery, libraryId, limit],
		queryFn: () =>
			searchApi.search({
				query: debouncedQuery.trim(),
				libraryId,
				limit,
			}),
		enabled: shouldSearch,
		staleTime: 30000, // Cache for 30 seconds
	});

	return {
		results: data ?? emptyResults,
		isLoading: shouldSearch && isLoading,
		error: error as Error | null,
	};
}
