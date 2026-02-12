import {
  Badge,
  Box,
  Button,
  Center,
  Group,
  Image,
  Loader,
  Modal,
  ScrollArea,
  Stack,
  Text,
  TextInput,
} from "@mantine/core";
import { useDebouncedValue } from "@mantine/hooks";
import {
  IconExternalLink,
  IconHash,
  IconSearch,
  IconUser,
  IconX,
} from "@tabler/icons-react";
import { useMutation } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  type PluginActionDto,
  type PluginSearchResultDto,
  pluginsApi,
} from "@/api/plugins";

export interface MetadataSearchModalProps {
  /** Whether the modal is open */
  opened: boolean;
  /** Callback to close the modal */
  onClose: () => void;
  /** The plugin to search with */
  plugin: PluginActionDto;
  /** Initial search query (e.g., series title) */
  initialQuery?: string;
  /** Author name to refine search results (for book searches) */
  author?: string;
  /** Content type to search for */
  contentType?: "series" | "book";
  /** Callback when a result is selected */
  onSelect: (result: PluginSearchResultDto) => void;
}

/**
 * Modal for searching metadata from a plugin
 *
 * Features:
 * - Debounced search input
 * - Results list with cover thumbnails
 * - Loading and error states
 */
export function MetadataSearchModal({
  opened,
  onClose,
  plugin,
  initialQuery = "",
  author: initialAuthor,
  contentType = "series",
  onSelect,
}: MetadataSearchModalProps) {
  const [query, setQuery] = useState(initialQuery);
  const [authorQuery, setAuthorQuery] = useState(initialAuthor ?? "");
  const [externalId, setExternalId] = useState("");
  const [debouncedQuery] = useDebouncedValue(query, 400);
  const [debouncedAuthor] = useDebouncedValue(authorQuery, 400);
  const [debouncedExternalId] = useDebouncedValue(externalId, 400);
  const [results, setResults] = useState<PluginSearchResultDto[]>([]);

  // Track request ID to prevent race conditions in debounced search.
  // Each search gets a unique ID, and we only update results if the response
  // matches the latest request ID.
  const requestIdRef = useRef(0);
  const lastSearchedQueryRef = useRef<string | null>(null);
  const lastSearchedAuthorRef = useRef<string | null>(null);

  // Perform search with race condition protection
  const performSearch = useCallback(
    async (searchQuery: string, searchAuthor?: string) => {
      // Increment request ID for this search
      const currentRequestId = ++requestIdRef.current;
      lastSearchedQueryRef.current = searchQuery;

      try {
        const response = await pluginsApi.searchMetadata(
          plugin.pluginId,
          searchQuery,
          contentType,
          searchAuthor || undefined,
        );

        // Only update results if this is still the latest request
        if (currentRequestId !== requestIdRef.current) {
          return; // Stale request, ignore results
        }

        if (!response.success || !response.result) {
          throw new Error(response.error || "Search failed");
        }

        const data = response.result as { results: PluginSearchResultDto[] };
        setResults(data.results || []);
      } catch (error) {
        // Only propagate error if this is still the latest request
        if (currentRequestId !== requestIdRef.current) {
          return; // Stale request, ignore error
        }
        throw error;
      }
    },
    [plugin.pluginId, contentType],
  );

  // Perform lookup by external ID (get instead of search)
  const performLookup = useCallback(
    async (id: string) => {
      const currentRequestId = ++requestIdRef.current;

      try {
        const response = await pluginsApi.getMetadata(
          plugin.pluginId,
          id,
          contentType,
        );

        if (currentRequestId !== requestIdRef.current) return;

        if (!response.success || !response.result) {
          throw new Error(response.error || "Lookup failed");
        }

        // Convert get response to a search result for uniform display
        const metadata = response.result as Record<string, unknown>;
        const result: PluginSearchResultDto = {
          externalId: (metadata.externalId as string) ?? id,
          title: (metadata.title as string) ?? id,
          alternateTitles: Array.isArray(metadata.alternateTitles)
            ? metadata.alternateTitles.map((t: unknown) =>
                typeof t === "string" ? t : (t as { title: string }).title,
              )
            : [],
          year: (metadata.year as number) ?? undefined,
          coverUrl: (metadata.coverUrl as string) ?? undefined,
          preview: {
            status: metadata.status as string | undefined,
            genres: Array.isArray(metadata.genres)
              ? (metadata.genres as string[])
              : [],
            bookCount:
              contentType === "series"
                ? (metadata.totalBookCount as number | undefined)
                : undefined,
            authors: Array.isArray(metadata.authors)
              ? (metadata.authors as unknown[]).map((a: unknown) =>
                  typeof a === "string" ? a : (a as { name: string }).name,
                )
              : [],
          },
        };
        setResults([result]);
      } catch (error) {
        if (currentRequestId !== requestIdRef.current) return;
        throw error;
      }
    },
    [plugin.pluginId, contentType],
  );

  // Search mutation with race condition protection
  const searchMutation = useMutation({
    mutationFn: (params: {
      query: string;
      author?: string;
      externalId?: string;
    }) =>
      params.externalId
        ? performLookup(params.externalId)
        : performSearch(params.query, params.author),
  });

  // Whether external ID mode is active (ID takes priority over title)
  const isIdMode = debouncedExternalId.trim().length > 0;

  // Reset state and trigger search when modal opens
  // biome-ignore lint/correctness/useExhaustiveDependencies: mutate is stable, only trigger on open/query change
  useEffect(() => {
    if (opened) {
      setQuery(initialQuery);
      setAuthorQuery(initialAuthor ?? "");
      setExternalId("");
      setResults([]);
      // Reset request tracking
      lastSearchedQueryRef.current = null;
      lastSearchedAuthorRef.current = null;
      // Trigger search immediately if we have a valid initial query
      if (initialQuery.trim().length >= 2) {
        searchMutation.mutate({
          query: initialQuery,
          author: initialAuthor,
        });
      }
    }
  }, [opened, initialQuery, initialAuthor]);

  // Auto-search when debounced query or author changes (for user typing)
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally only trigger on query/author/externalId change
  useEffect(() => {
    // Skip if modal just opened (handled by the effect above)
    if (!opened) return;

    // External ID takes priority over title search
    const trimmedId = debouncedExternalId.trim();
    if (trimmedId.length > 0) {
      searchMutation.mutate({ query: "", externalId: trimmedId });
      return;
    }

    const trimmedQuery = debouncedQuery.trim();
    if (trimmedQuery.length >= 2) {
      // Only search if the query or author is different from what we last searched.
      // This prevents duplicate searches when debounced value catches up.
      if (
        trimmedQuery !== lastSearchedQueryRef.current ||
        debouncedAuthor !== lastSearchedAuthorRef.current
      ) {
        lastSearchedAuthorRef.current = debouncedAuthor;
        searchMutation.mutate({
          query: debouncedQuery,
          author: debouncedAuthor || undefined,
        });
      }
    } else {
      setResults([]);
    }
  }, [debouncedQuery, debouncedAuthor, debouncedExternalId]);

  const handleSelect = (result: PluginSearchResultDto) => {
    onSelect(result);
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconSearch size={20} />
          <Text fw={600}>Search {plugin.pluginDisplayName}</Text>
        </Group>
      }
      size="lg"
      scrollAreaComponent={ScrollArea.Autosize}
    >
      <Stack gap="md">
        {/* Search inputs */}
        <Stack gap="xs">
          <TextInput
            label="External ID"
            description="Paste a provider ID to look up directly (takes priority over title)"
            placeholder="e.g. 12345"
            value={externalId}
            onChange={(e) => setExternalId(e.currentTarget.value)}
            leftSection={<IconHash size={16} />}
            rightSection={
              externalId && (
                <IconX
                  size={16}
                  style={{ cursor: "pointer" }}
                  onClick={() => setExternalId("")}
                />
              )
            }
          />

          <TextInput
            label="Title"
            placeholder={`Search for ${contentType}...`}
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            leftSection={<IconSearch size={16} />}
            rightSection={
              query && (
                <IconX
                  size={16}
                  style={{ cursor: "pointer" }}
                  onClick={() => setQuery("")}
                />
              )
            }
            disabled={isIdMode}
            autoFocus={!isIdMode}
          />

          {/* Author filter (for book searches) */}
          {contentType === "book" && (
            <TextInput
              label="Author"
              placeholder="Filter by author..."
              value={authorQuery}
              onChange={(e) => setAuthorQuery(e.currentTarget.value)}
              leftSection={<IconUser size={16} />}
              rightSection={
                authorQuery && (
                  <IconX
                    size={16}
                    style={{ cursor: "pointer" }}
                    onClick={() => setAuthorQuery("")}
                  />
                )
              }
              disabled={isIdMode}
            />
          )}
        </Stack>

        {/* External search link */}
        {plugin.searchUriTemplate && query.trim().length >= 2 && (
          <Button
            component="a"
            href={plugin.searchUriTemplate.replace(
              "<title>",
              encodeURIComponent(query.trim()),
            )}
            target="_blank"
            rel="noopener noreferrer"
            variant="light"
            leftSection={<IconExternalLink size={16} />}
            fullWidth
          >
            Search on {plugin.pluginDisplayName}
          </Button>
        )}

        {/* Loading state */}
        {searchMutation.isPending && (
          <Center py="xl">
            <Loader size="md" />
          </Center>
        )}

        {/* Error state */}
        {searchMutation.isError && (
          <Center py="xl">
            <Stack align="center" gap="xs">
              <Text c="red" size="sm">
                {searchMutation.error?.message || "Search failed"}
              </Text>
              <Button
                size="xs"
                variant="light"
                onClick={() =>
                  searchMutation.mutate({
                    query: debouncedQuery,
                    author: debouncedAuthor || undefined,
                  })
                }
              >
                Retry
              </Button>
            </Stack>
          </Center>
        )}

        {/* No results */}
        {!searchMutation.isPending &&
          !searchMutation.isError &&
          (isIdMode || debouncedQuery.trim().length >= 2) &&
          results.length === 0 && (
            <Center py="xl">
              <Text c="dimmed" size="sm">
                {isIdMode
                  ? `No results found for ID: ${debouncedExternalId.trim()}`
                  : `No results found for "${debouncedQuery}"`}
              </Text>
            </Center>
          )}

        {/* Results list */}
        {results.length > 0 && (
          <Stack gap="xs">
            <Text size="sm" c="dimmed">
              {results.length} result{results.length !== 1 ? "s" : ""} found
            </Text>
            {results.map((result) => (
              <SearchResultCard
                key={result.externalId}
                result={result}
                onSelect={handleSelect}
              />
            ))}
          </Stack>
        )}

        {/* Initial state hint */}
        {!searchMutation.isPending &&
          !searchMutation.isError &&
          !isIdMode &&
          debouncedQuery.trim().length < 2 &&
          results.length === 0 && (
            <Center py="xl">
              <Text c="dimmed" size="sm">
                Enter at least 2 characters to search, or paste an external ID
              </Text>
            </Center>
          )}
      </Stack>
    </Modal>
  );
}

interface SearchResultCardProps {
  result: PluginSearchResultDto;
  onSelect: (result: PluginSearchResultDto) => void;
}

function SearchResultCard({ result, onSelect }: SearchResultCardProps) {
  return (
    <Box
      p="sm"
      style={(theme) => ({
        border: `1px solid ${theme.colors.dark[4]}`,
        borderRadius: theme.radius.sm,
        cursor: "pointer",
        transition: "background-color 150ms ease",
        "&:hover": {
          backgroundColor: theme.colors.dark[6],
        },
      })}
      onClick={() => onSelect(result)}
    >
      <Group gap="md" wrap="nowrap" align="flex-start">
        {/* Cover image */}
        <Image
          src={result.coverUrl}
          alt={result.title}
          w={60}
          h={85}
          radius="xs"
          fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='60' height='85'%3E%3Crect fill='%23333' width='60' height='85'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='8' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
          style={{ flexShrink: 0 }}
        />

        {/* Info */}
        <Stack gap={4} style={{ flex: 1, minWidth: 0 }}>
          <Text fw={500} lineClamp={1}>
            {result.title}
          </Text>

          {result.preview?.authors && result.preview.authors.length > 0 && (
            <Text size="xs" c="dimmed" lineClamp={1}>
              {result.preview.authors.join(", ")}
            </Text>
          )}

          {result.year && (
            <Text size="xs" c="dimmed">
              {result.year}
            </Text>
          )}

          {result.alternateTitles && result.alternateTitles.length > 0 && (
            <Text size="xs" c="dimmed" lineClamp={1}>
              {result.alternateTitles.slice(0, 2).join(" / ")}
              {result.alternateTitles.length > 2 &&
                ` +${result.alternateTitles.length - 2}`}
            </Text>
          )}

          {result.preview && (
            <Group gap="xs" mt={4}>
              {result.preview.status && (
                <Badge size="xs" variant="outline">
                  {result.preview.status}
                </Badge>
              )}
              {result.preview.bookCount != null && (
                <Badge size="xs" variant="filled" color="blue">
                  {result.preview.bookCount}{" "}
                  {result.preview.bookCount === 1 ? "book" : "books"}
                </Badge>
              )}
              {result.preview.genres?.slice(0, 3).map((genre) => (
                <Badge key={genre} size="xs" variant="light">
                  {genre}
                </Badge>
              ))}
            </Group>
          )}
        </Stack>
      </Group>
    </Box>
  );
}
