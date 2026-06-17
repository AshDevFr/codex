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
import { useDisclosure } from "@mantine/hooks";
import {
  IconAdjustmentsHorizontal,
  IconAlertTriangle,
  IconBolt,
  IconSearch,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { booksApi } from "@/api/books";
import { seriesApi } from "@/api/series";
import { settingsApi } from "@/api/settings";
import { BulkSelectionToolbar } from "@/components/library/BulkSelectionToolbar";
import { MediaCard } from "@/components/library/MediaCard";
import {
  type Condition,
  normalizeForEmit,
} from "@/components/search/filterBuilder/conditionUtils";
import { FilterBuilder } from "@/components/search/filterBuilder/FilterBuilder";
import { PresetsMenu } from "@/components/search/PresetsMenu";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import {
  selectCanSelectType,
  selectIsSelectionMode,
  useBulkSelectionStore,
} from "@/store/bulkSelectionStore";
import type { Book, Series } from "@/types";
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

  // Advanced search is submit-driven: drafts are local until the user hits
  // Enter or clicks "Search", at which point we write them to the URL.
  // Live syncing fought the inputs — a URL write on every keystroke could
  // overwrite the draft mid-edit and drop characters, especially in the
  // nested LeafEditor inputs.
  const [queryDraft, setQueryDraft] = useState(state.query);
  const [conditionDraft, setConditionDraft] = useState<Condition | undefined>(
    state.condition as Condition | undefined,
  );

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

  // Sync URL-driven changes (preset apply, history nav, deep links) into the
  // drafts. With submit-only writes, the URL never changes from typing, so
  // this can't fight an in-progress edit.
  useEffect(() => {
    setQueryDraft(state.query);
  }, [state.query]);

  useEffect(() => {
    setConditionDraft(state.condition as Condition | undefined);
  }, [state.condition]);

  // Cheap structural compare so submit can no-op when nothing changed.
  const conditionDraftSerialized = useMemo(
    () => JSON.stringify(conditionDraft ?? null),
    [conditionDraft],
  );
  const conditionStateSerialized = useMemo(
    () => JSON.stringify(state.condition ?? null),
    [state.condition],
  );

  const isDirty =
    queryDraft !== state.query ||
    conditionDraftSerialized !== conditionStateSerialized;

  const submitSearch = useCallback(() => {
    if (!isDirty) return;
    updateState({
      query: queryDraft,
      condition: conditionDraft as SeriesCondition | BookCondition | undefined,
      page: 1,
    });
  }, [isDirty, queryDraft, conditionDraft, updateState]);

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
  // Normalize per target so each query gets only the leaves valid for it.
  // Both tabs apply the filter so the inactive tab's badge reflects the
  // same condition the user is searching with, instead of showing the
  // unfiltered library total.
  const seriesCondition = useMemo(
    () =>
      normalizeForEmit(state.condition ?? { allOf: [] }, "series") as
        | SeriesCondition
        | undefined,
    [state.condition],
  );
  const booksCondition = useMemo(
    () =>
      normalizeForEmit(state.condition ?? { allOf: [] }, "books") as
        | BookCondition
        | undefined,
    [state.condition],
  );

  const seriesSort = useMemo(() => {
    if (state.tab === "series") return state.sort;
    return "";
  }, [state.tab, state.sort]);

  const booksSort = useMemo(() => {
    if (state.tab === "books") return state.sort;
    return "";
  }, [state.tab, state.sort]);

  // Advanced search needs *something* to act on: a non-empty query, or at
  // least one fully-configured filter leaf on either tab (normalizeForEmit
  // returns undefined when every leaf is incomplete, which keeps an empty
  // "+ Add filter" row from counting as a filter).
  const hasQuery = state.query.trim().length > 0;
  const hasFilter = !!seriesCondition || !!booksCondition;
  const canSearch = hasQuery || hasFilter;

  // Per-tab fetchability: don't fire a query that would degenerate to an
  // unfiltered "all rows" fetch when the filter only applies to the other
  // tab. The active tab's panel handles that case below with a specific
  // empty state.
  const seriesCanFetch = hasQuery || !!seriesCondition;
  const booksCanFetch = hasQuery || !!booksCondition;

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
    enabled: seriesCanFetch,
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
    enabled: booksCanFetch,
  });

  const seriesCount = seriesQuery.data?.total ?? 0;
  const booksCount = booksQuery.data?.total ?? 0;

  const clearSelection = useBulkSelectionStore((s) => s.clearSelection);
  const isSelectionMode = useBulkSelectionStore(selectIsSelectionMode);

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

        <form
          onSubmit={(event) => {
            event.preventDefault();
            submitSearch();
          }}
        >
          <Group gap="sm" wrap="nowrap" align="stretch">
            <TextInput
              size="md"
              leftSection={<IconSearch size={16} />}
              placeholder="Search by title, author, or text…"
              value={queryDraft}
              onChange={(e) => setQueryDraft(e.currentTarget.value)}
              aria-label="Search query"
              style={{ flex: 1 }}
            />
            <Button type="submit" size="md" disabled={!isDirty}>
              Search
            </Button>
          </Group>
        </form>

        <Group justify="space-between" wrap="wrap" gap="md">
          <Button
            variant="subtle"
            leftSection={<IconAdjustmentsHorizontal size={16} />}
            onClick={() =>
              builderOpened ? builderHandlers.close() : builderHandlers.open()
            }
          >
            {builderOpened ? "Hide filters" : "Show filters"}
          </Button>
          <Group gap="md" wrap="wrap">
            <Group gap="xs" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
              <Text size="sm" c="dimmed">
                Sort:
              </Text>
              <Select
                size="xs"
                data={sortOptionsWithRelevance}
                value={state.sort || null}
                onChange={(value) =>
                  updateState({ sort: value ?? "", page: 1 })
                }
                placeholder={state.query.trim() ? "Relevance" : "Default"}
                clearable
                style={{ flex: 1, minWidth: 0, maxWidth: 240 }}
              />
            </Group>
            <PresetsMenu
              target={state.tab}
              current={{
                query: queryDraft,
                sort: state.sort,
                condition: conditionDraft as
                  | SeriesCondition
                  | BookCondition
                  | undefined,
              }}
              onApply={(preset) => {
                const nextCondition =
                  (preset.condition as unknown as
                    | SeriesCondition
                    | BookCondition
                    | undefined) ?? undefined;
                // Update the drafts in lockstep with the URL so the editor
                // doesn't flash the old condition for one render before the
                // sync effect catches up.
                setQueryDraft(preset.query ?? "");
                setConditionDraft(nextCondition as Condition | undefined);
                updateState({
                  query: preset.query ?? "",
                  sort: preset.sort ?? "",
                  condition: nextCondition,
                  page: 1,
                });
              }}
            />
          </Group>
        </Group>

        <Collapse in={builderOpened}>
          <Card withBorder p="md">
            <FilterBuilder
              condition={conditionDraft}
              target={state.tab}
              onChange={(next) => setConditionDraft(next)}
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

        {/* Bulk selection toolbar - sits between the filters and the tabs and
            sticks below the app header so it stays reachable while scrolling
            through results. Only mounted while a selection is active so it
            never leaves an empty band. The body background hides result cards
            scrolling under the rounded bar. */}
        {isSelectionMode && (
          <Box
            style={{
              position: "sticky",
              top: "var(--app-shell-header-height, 64px)",
              zIndex: 3,
              backgroundColor: "var(--mantine-color-body)",
              paddingBottom: "var(--mantine-spacing-xs)",
            }}
          >
            <BulkSelectionToolbar />
          </Box>
        )}

        <Tabs
          value={state.tab}
          onChange={(value) => {
            if (value === "series" || value === "books") {
              // Selection is type-locked (series XOR books); clear on tab
              // switch so the toolbar always matches the visible tab.
              clearSelection();
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

          {canSearch ? (
            <>
              <Tabs.Panel value="series" pt="md">
                {seriesCanFetch ? (
                  <ResultsGrid
                    loading={seriesQuery.isLoading}
                    error={seriesQuery.error}
                    data={seriesQuery.data?.data ?? []}
                    total={seriesCount}
                    page={state.page}
                    pageSize={DEFAULT_SEARCH_PAGE_SIZE}
                    onPageChange={(p) => updateState({ page: p })}
                    type="series"
                    active={state.tab === "series"}
                  />
                ) : (
                  <FilterOnlyForOtherTab type="series" />
                )}
              </Tabs.Panel>

              <Tabs.Panel value="books" pt="md">
                {booksCanFetch ? (
                  <ResultsGrid
                    loading={booksQuery.isLoading}
                    error={booksQuery.error}
                    data={booksQuery.data?.data ?? []}
                    total={booksCount}
                    page={state.page}
                    pageSize={DEFAULT_SEARCH_PAGE_SIZE}
                    onPageChange={(p) => updateState({ page: p })}
                    type="book"
                    active={state.tab === "books"}
                  />
                ) : (
                  <FilterOnlyForOtherTab type="books" />
                )}
              </Tabs.Panel>
            </>
          ) : (
            <Card mt="md" p="xl" withBorder>
              <Stack align="center" gap="sm">
                <Text size="lg" fw={600}>
                  Nothing to search yet
                </Text>
                <Text size="sm" c="dimmed">
                  Enter a search term or add a filter to see results.
                </Text>
              </Stack>
            </Card>
          )}
        </Tabs>
      </Stack>
    </Container>
  );
}

interface ResultsGridProps {
  loading: boolean;
  error: unknown;
  // Series or Book rows; both satisfy MediaCard's `data` shape per `type`.
  data: (Book | Series)[];
  total: number;
  page: number;
  pageSize: number;
  onPageChange: (page: number) => void;
  type: "series" | "book";
  // Both tabs stay mounted (Mantine keeps inactive panels in the DOM), so only
  // the active grid registers page items and drives Select All to avoid the
  // two grids racing on the shared store.
  active: boolean;
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
  active,
}: ResultsGridProps) {
  // Bulk selection state - stable selectors to minimize re-renders.
  const isSelectionMode = useBulkSelectionStore(selectIsSelectionMode);
  const canSelect = useBulkSelectionStore(selectCanSelectType(type));
  const selectedIds = useBulkSelectionStore((s) => s.selectedIds);
  const toggleSelection = useBulkSelectionStore((s) => s.toggleSelection);
  const selectRange = useBulkSelectionStore((s) => s.selectRange);
  const getLastSelectedIndex = useBulkSelectionStore(
    (s) => s.getLastSelectedIndex,
  );
  const setPageItems = useBulkSelectionStore((s) => s.setPageItems);

  const gridId = `search-${type}`;

  // Ref holding current page data for range selection (avoids re-creating
  // handleSelect on every data change).
  const dataRef = useRef<(Book | Series)[]>([]);
  dataRef.current = data;

  // Register visible page items for Select All - only the active tab, so the
  // inactive (but still mounted) grid never clobbers the registration.
  useEffect(() => {
    if (!active) return;
    if (data.length > 0) {
      setPageItems({ ids: data.map((d) => d.id), type });
    } else {
      setPageItems(null);
    }
    return () => setPageItems(null);
  }, [active, data, type, setPageItems]);

  // Handle selection with shift+click range support. Stable across data
  // changes because it reads page data from a ref.
  const handleSelect = useCallback(
    (id: string, shiftKey: boolean, index?: number) => {
      if (shiftKey && isSelectionMode && index !== undefined) {
        const lastIndex = getLastSelectedIndex(gridId);
        if (lastIndex !== undefined && lastIndex !== index) {
          const start = Math.min(lastIndex, index);
          const end = Math.max(lastIndex, index);
          const rangeIds = dataRef.current
            .slice(start, end + 1)
            .map((item) => item.id);
          selectRange(rangeIds, type);
          return;
        }
      }
      toggleSelection(id, type, gridId, index);
    },
    [
      toggleSelection,
      selectRange,
      getLastSelectedIndex,
      gridId,
      type,
      isSelectionMode,
    ],
  );

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
          <MediaCard
            key={item.id}
            type={type}
            data={item}
            index={idx}
            onSelect={handleSelect}
            isSelected={selectedIds.has(item.id)}
            isSelectionMode={isSelectionMode}
            canBeSelected={canSelect}
          />
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

function FilterOnlyForOtherTab({ type }: { type: "series" | "books" }) {
  const otherTab = type === "series" ? "Books" : "Series";
  const selfLabel = type === "series" ? "series" : "books";
  return (
    <Card mt="md" p="xl" withBorder>
      <Stack align="center" gap="sm">
        <Text size="lg" fw={600}>
          Filter doesn't apply to {selfLabel}
        </Text>
        <Text size="sm" c="dimmed" ta="center">
          The current filter only has fields available on the {otherTab} tab.
        </Text>
      </Stack>
    </Card>
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
