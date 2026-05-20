import {
  Alert,
  Badge,
  Box,
  Button,
  Card,
  Collapse,
  Container,
  Group,
  Pagination,
  Select,
  Stack,
  Tabs,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDebouncedValue, useDisclosure } from "@mantine/hooks";
import {
  IconAdjustmentsHorizontal,
  IconAlertTriangle,
  IconBolt,
  IconSearch,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { settingsApi } from "@/api/settings";
import { MediaCard } from "@/components/library/MediaCard";
import {
  type Condition,
  normalizeForEmit,
} from "@/components/search/filterBuilder/conditionUtils";
import { FilterBuilder } from "@/components/search/filterBuilder/FilterBuilder";
import { PresetsSidebar } from "@/components/search/PresetsSidebar";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import type { BookCondition, SeriesCondition } from "@/types/filters";
import {
  DEFAULT_SEARCH_PAGE_SIZE,
  parseSearchUrl,
  type SearchUrlState,
  serializeSearchUrl,
} from "@/utils/searchUrl";

const SERIES_SORT_OPTIONS = [
  { value: "name,asc", label: "Title (A–Z)" },
  { value: "name,desc", label: "Title (Z–A)" },
  { value: "release_date,desc", label: "Year (newest)" },
  { value: "release_date,asc", label: "Year (oldest)" },
  { value: "date_added,desc", label: "Date added (newest)" },
  { value: "date_added,asc", label: "Date added (oldest)" },
  { value: "book_count,desc", label: "Book count (most)" },
  { value: "rating,desc", label: "Rating (best)" },
];

const BOOKS_SORT_OPTIONS = [
  { value: "title,asc", label: "Title (A–Z)" },
  { value: "title,desc", label: "Title (Z–A)" },
  { value: "release_date,desc", label: "Release date (newest)" },
  { value: "release_date,asc", label: "Release date (oldest)" },
  { value: "date_added,desc", label: "Date added (newest)" },
  { value: "date_added,asc", label: "Date added (oldest)" },
  { value: "page_count,desc", label: "Page count (most)" },
  { value: "file_size,desc", label: "File size (largest)" },
  { value: "last_read,desc", label: "Last read" },
];

export function SearchPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const state = useMemo(() => parseSearchUrl(searchParams), [searchParams]);

  useDocumentTitle(state.query ? `Search: ${state.query}` : "Advanced search");

  // Track the live query input so debouncing doesn't fight URL state.
  const [queryDraft, setQueryDraft] = useState(state.query);
  const [debouncedQuery] = useDebouncedValue(queryDraft, 300);

  const [conditionTooLarge, setConditionTooLarge] = useState(false);

  const updateState = useCallback(
    (patch: Partial<SearchUrlState>) => {
      const next: SearchUrlState = {
        ...state,
        ...patch,
      };
      // Page resets to 1 on any change that isn't an explicit page change.
      if (!("page" in patch) && shouldResetPage(state, next)) {
        next.page = 1;
      }
      const { params, conditionDropped } = serializeSearchUrl(next);
      setConditionTooLarge(conditionDropped);
      setSearchParams(params, { replace: true });
    },
    [state, setSearchParams],
  );

  // Sync URL-driven changes (preset apply, history nav) back into the draft.
  useEffect(() => {
    setQueryDraft(state.query);
  }, [state.query]);

  // When the debounced draft diverges from the URL, push it back. updateState
  // is excluded from deps on purpose — it captures `state`, and re-running the
  // effect on every render would re-trigger the URL write in a loop.
  // biome-ignore lint/correctness/useExhaustiveDependencies: see comment above
  useEffect(() => {
    if (debouncedQuery !== state.query) {
      updateState({ query: debouncedQuery, page: 1 });
    }
  }, [debouncedQuery]);

  // Fuzzy indicator: opt-in cosmetic that lets users know what's powering
  // the search. Read from the public settings map (no admin required).
  const { data: publicSettings } = useQuery({
    queryKey: ["public-settings"],
    queryFn: () => settingsApi.getPublicSettings(),
    staleTime: 5 * 60 * 1000,
  });
  const fuzzyEnabled = parseBool(
    publicSettings?.["search.fuzzy.enabled"]?.value,
  );

  // ---- Result queries (run in parallel; counts feed the tab labels) ----
  const targetForActiveTab = state.tab;
  const conditionForActiveTab = useMemo(
    () => normalizeForEmit(state.condition ?? { allOf: [] }),
    [state.condition],
  );

  const seriesCondition = useMemo(
    () =>
      targetForActiveTab === "series"
        ? (conditionForActiveTab as SeriesCondition | undefined)
        : undefined,
    [targetForActiveTab, conditionForActiveTab],
  );
  const booksCondition = useMemo(
    () =>
      targetForActiveTab === "books"
        ? (conditionForActiveTab as BookCondition | undefined)
        : undefined,
    [targetForActiveTab, conditionForActiveTab],
  );

  const seriesSort = useMemo(() => {
    if (state.tab === "series") return state.sort;
    return "";
  }, [state.tab, state.sort]);

  const booksSort = useMemo(() => {
    if (state.tab === "books") return state.sort;
    return "";
  }, [state.tab, state.sort]);

  const seriesQuery = useQuery({
    queryKey: [
      "search",
      "series",
      state.query,
      seriesSort,
      JSON.stringify(seriesCondition ?? null),
      state.tab === "series" ? state.page : 1,
    ],
    queryFn: () =>
      seriesApi.search("all", {
        condition: seriesCondition,
        search: state.query.trim() || undefined,
        page: state.tab === "series" ? state.page : 1,
        pageSize: DEFAULT_SEARCH_PAGE_SIZE,
        sort: seriesSort || undefined,
      }),
    staleTime: 30_000,
  });

  const booksQuery = useQuery({
    queryKey: [
      "search",
      "books",
      state.query,
      booksSort,
      JSON.stringify(booksCondition ?? null),
      state.tab === "books" ? state.page : 1,
    ],
    queryFn: () =>
      booksApi.search("all", {
        condition: booksCondition,
        search: state.query.trim() || undefined,
        page: state.tab === "books" ? state.page : 1,
        pageSize: DEFAULT_SEARCH_PAGE_SIZE,
        sort: booksSort || undefined,
      }),
    staleTime: 30_000,
  });

  const seriesCount = seriesQuery.data?.total ?? 0;
  const booksCount = booksQuery.data?.total ?? 0;

  const [builderOpened, builderHandlers] = useDisclosure(true);

  const activeSortOptions =
    state.tab === "series" ? SERIES_SORT_OPTIONS : BOOKS_SORT_OPTIONS;

  const sortOptionsWithRelevance = useMemo(() => {
    if (!state.query.trim()) return activeSortOptions;
    return [
      { value: "relevance", label: "Relevance (best match)" },
      ...activeSortOptions,
    ];
  }, [activeSortOptions, state.query]);

  return (
    <Container size="xl" py="xl">
      <Stack gap="md">
        <Group justify="space-between" align="flex-end">
          <Title order={1}>Search</Title>
          {fuzzyEnabled && (
            <Tooltip label="Search uses the fuzzy index for tolerant matching.">
              <Badge
                leftSection={<IconBolt size={12} />}
                variant="light"
                color="grape"
              >
                Fuzzy enabled
              </Badge>
            </Tooltip>
          )}
        </Group>

        <TextInput
          size="md"
          leftSection={<IconSearch size={16} />}
          placeholder="Search by title, author, or text…"
          value={queryDraft}
          onChange={(e) => setQueryDraft(e.currentTarget.value)}
          aria-label="Search query"
        />

        <Group justify="space-between">
          <Button
            variant="subtle"
            leftSection={<IconAdjustmentsHorizontal size={16} />}
            onClick={() =>
              builderOpened ? builderHandlers.close() : builderHandlers.open()
            }
          >
            {builderOpened ? "Hide filters" : "Show filters"}
          </Button>
          <Group gap="xs">
            <Text size="sm" c="dimmed">
              Sort:
            </Text>
            <Select
              size="xs"
              data={sortOptionsWithRelevance}
              value={state.sort || null}
              onChange={(value) => updateState({ sort: value ?? "", page: 1 })}
              placeholder={state.query.trim() ? "Relevance" : "Default"}
              clearable
              w={240}
            />
          </Group>
        </Group>

        <Collapse in={builderOpened}>
          <Card withBorder p="md">
            <FilterBuilder
              condition={state.condition as Condition | undefined}
              target={state.tab}
              onChange={(next) =>
                updateState({
                  condition: next as
                    | SeriesCondition
                    | BookCondition
                    | undefined,
                  page: 1,
                })
              }
            />
          </Card>
        </Collapse>

        {conditionTooLarge && (
          <Alert
            icon={<IconAlertTriangle size={16} />}
            color="yellow"
            variant="light"
            title="Filter too long for the URL"
          >
            Your filter is too complex to share via URL. Save it as a preset
            from the sidebar to reload it later without losing state.
          </Alert>
        )}

        <Box
          style={{
            display: "grid",
            gridTemplateColumns: "260px 1fr",
            gap: "1.5rem",
          }}
        >
          <PresetsSidebar
            target={state.tab}
            current={{
              query: state.query,
              sort: state.sort,
              condition: state.condition,
            }}
            onApply={(preset) => {
              updateState({
                query: preset.query ?? "",
                sort: preset.sort ?? "",
                condition:
                  (preset.condition as unknown as
                    | SeriesCondition
                    | BookCondition
                    | undefined) ?? undefined,
                page: 1,
              });
            }}
          />

          <Stack gap="md">
            <Tabs
              value={state.tab}
              onChange={(value) => {
                if (value === "series" || value === "books") {
                  updateState({ tab: value, page: 1 });
                }
              }}
            >
              <Tabs.List>
                <Tabs.Tab value="series">
                  Series{" "}
                  {seriesQuery.isSuccess && (
                    <Badge size="sm" ml={6} variant="light">
                      {seriesCount}
                    </Badge>
                  )}
                </Tabs.Tab>
                <Tabs.Tab value="books">
                  Books{" "}
                  {booksQuery.isSuccess && (
                    <Badge size="sm" ml={6} variant="light">
                      {booksCount}
                    </Badge>
                  )}
                </Tabs.Tab>
              </Tabs.List>

              <Tabs.Panel value="series" pt="md">
                <ResultsGrid
                  loading={seriesQuery.isLoading}
                  error={seriesQuery.error}
                  data={seriesQuery.data?.data ?? []}
                  total={seriesCount}
                  page={state.page}
                  pageSize={DEFAULT_SEARCH_PAGE_SIZE}
                  onPageChange={(p) => updateState({ page: p })}
                  type="series"
                />
              </Tabs.Panel>

              <Tabs.Panel value="books" pt="md">
                <ResultsGrid
                  loading={booksQuery.isLoading}
                  error={booksQuery.error}
                  data={booksQuery.data?.data ?? []}
                  total={booksCount}
                  page={state.page}
                  pageSize={DEFAULT_SEARCH_PAGE_SIZE}
                  onPageChange={(p) => updateState({ page: p })}
                  type="book"
                />
              </Tabs.Panel>
            </Tabs>
          </Stack>
        </Box>
      </Stack>
    </Container>
  );
}

interface ResultsGridProps {
  loading: boolean;
  error: unknown;
  // Series or Book rows; both satisfy MediaCard's `data` shape per `type`.
  data: { id: string }[];
  total: number;
  page: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  type: "series" | "book";
}

function ResultsGrid({
  loading,
  error,
  data,
  total,
  page,
  pageSize,
  onPageChange,
  type,
}: ResultsGridProps) {
  if (error) {
    return (
      <Alert color="red" icon={<IconAlertTriangle size={16} />}>
        {(error as Error).message ?? "Search failed"}
      </Alert>
    );
  }
  if (loading) {
    return (
      <Card p="xl" withBorder>
        <Text size="sm" c="dimmed" ta="center">
          Loading…
        </Text>
      </Card>
    );
  }
  if (data.length === 0) {
    return (
      <Card p="xl" withBorder>
        <Stack align="center" gap="sm">
          <Text size="lg" fw={600}>
            No results
          </Text>
          <Text size="sm" c="dimmed">
            Try a different query or simplify the filters.
          </Text>
        </Stack>
      </Card>
    );
  }

  const totalPages = Math.ceil(total / pageSize);
  const showPagination = total > pageSize;

  return (
    <Stack gap="md">
      {showPagination && (
        <Group justify="center">
          <Pagination value={page} onChange={onPageChange} total={totalPages} />
        </Group>
      )}
      <div
        data-stagger-grid="true"
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
          gap: "var(--mantine-spacing-md)",
          width: "100%",
        }}
      >
        {data.map((item, idx) => (
          <MediaCard key={item.id} type={type} data={item} index={idx} />
        ))}
      </div>
      {showPagination && (
        <Group justify="center" mt="md">
          <Pagination value={page} onChange={onPageChange} total={totalPages} />
        </Group>
      )}
      <Text size="sm" c="dimmed" ta="center">
        Showing {(page - 1) * pageSize + 1} to{" "}
        {Math.min(page * pageSize, total)} of {total} results
      </Text>
    </Stack>
  );
}

function shouldResetPage(prev: SearchUrlState, next: SearchUrlState): boolean {
  return (
    prev.query !== next.query ||
    prev.sort !== next.sort ||
    prev.tab !== next.tab ||
    JSON.stringify(prev.condition ?? null) !==
      JSON.stringify(next.condition ?? null)
  );
}

function parseBool(value: unknown): boolean {
  if (value === true) return true;
  if (value === "true") return true;
  return false;
}
