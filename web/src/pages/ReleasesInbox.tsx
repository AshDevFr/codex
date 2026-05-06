import {
  Badge,
  Card,
  Group,
  Loader,
  Pagination,
  Select,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconRss } from "@tabler/icons-react";
import { useEffect, useMemo, useState } from "react";
import type {
  BulkReleaseAction,
  ReleaseFacets,
  ReleaseFacetsParams,
  ReleaseInboxParams,
  ReleaseSource,
} from "@/api/releases";
import { ReleasesBulkActionBar } from "@/components/releases/ReleasesBulkActionBar";
import { ReleasesBulkDeleteModal } from "@/components/releases/ReleasesBulkDeleteModal";
import { ReleasesTable } from "@/components/releases/ReleasesTable";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import {
  useBulkReleaseAction,
  useDeleteRelease,
  useDismissRelease,
  useMarkReleaseAcquired,
  useReleaseFacets,
  useReleaseInbox,
  useReleaseSources,
} from "@/hooks/useReleases";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";

const STATE_OPTIONS = [
  { value: "all", label: "All" },
  { value: "announced", label: "New" },
  { value: "marked_acquired", label: "Acquired" },
  { value: "dismissed", label: "Dismissed" },
  { value: "ignored", label: "Ignored" },
];

const PAGE_SIZE = 50;

const ALL_VALUE = "__all__";

/** Build the grouped, alphabetised series options for the Mantine Select. */
function buildSeriesOptions(facets: ReleaseFacets | undefined) {
  if (!facets) return [];
  const byLibrary = new Map<
    string,
    { libraryName: string; items: { value: string; label: string }[] }
  >();
  for (const s of facets.series) {
    // Fall back to the id when title/library are missing so the option
    // still renders something searchable instead of an empty string.
    const libraryName = s.libraryName || "Unknown library";
    const title = s.seriesTitle || `${s.seriesId.slice(0, 8)}…`;
    const label = `${title} (${s.count})`;
    const existing = byLibrary.get(s.libraryId);
    if (existing) {
      existing.items.push({ value: s.seriesId, label });
    } else {
      byLibrary.set(s.libraryId, {
        libraryName,
        items: [{ value: s.seriesId, label }],
      });
    }
  }
  const groups = Array.from(byLibrary.values()).sort((a, b) =>
    a.libraryName.localeCompare(b.libraryName),
  );
  for (const g of groups) {
    g.items.sort((a, b) => a.label.localeCompare(b.label));
  }
  return [
    { value: ALL_VALUE, label: "All series" },
    ...groups.map((g) => ({ group: g.libraryName, items: g.items })),
  ];
}

function buildLibraryOptions(facets: ReleaseFacets | undefined) {
  if (!facets) return [{ value: ALL_VALUE, label: "All libraries" }];
  const opts = facets.libraries
    .map((l) => ({
      value: l.libraryId,
      label: `${l.libraryName || "Unknown"} (${l.count})`,
    }))
    .sort((a, b) => a.label.localeCompare(b.label));
  return [{ value: ALL_VALUE, label: "All libraries" }, ...opts];
}

function buildLanguageOptions(facets: ReleaseFacets | undefined) {
  if (!facets) return [{ value: ALL_VALUE, label: "All languages" }];
  const opts = facets.languages
    .map((l) => ({
      value: l.language,
      label: `${l.language} (${l.count})`,
    }))
    .sort((a, b) => a.label.localeCompare(b.label));
  return [{ value: ALL_VALUE, label: "All languages" }, ...opts];
}

export function ReleasesInbox() {
  useDocumentTitle("Releases");

  const resetBadge = useReleaseAnnouncementsStore((s) => s.reset);
  // Visiting the inbox marks all unseen events as seen — the user has
  // landed where the events would have sent them anyway.
  useEffect(() => {
    resetBadge();
  }, [resetBadge]);

  const [state, setState] = useState<string>("announced");
  const [language, setLanguage] = useState<string>(ALL_VALUE);
  const [seriesId, setSeriesId] = useState<string>(ALL_VALUE);
  const [libraryId, setLibraryId] = useState<string>(ALL_VALUE);
  const [page, setPage] = useState<number>(1);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmBulkDelete, { open: openBulkDelete, close: closeBulkDelete }] =
    useDisclosure(false);

  const inboxParams: ReleaseInboxParams = {
    state,
    language: language === ALL_VALUE ? undefined : language,
    seriesId: seriesId === ALL_VALUE ? undefined : seriesId,
    libraryId: libraryId === ALL_VALUE ? undefined : libraryId,
    page,
    pageSize: PAGE_SIZE,
  };
  // Facet query mirrors the inbox filters minus pagination so each
  // dropdown reflects what's actually selectable under the current state.
  const facetsParams: ReleaseFacetsParams = {
    state,
    language: language === ALL_VALUE ? undefined : language,
    seriesId: seriesId === ALL_VALUE ? undefined : seriesId,
    libraryId: libraryId === ALL_VALUE ? undefined : libraryId,
  };

  const { data, isLoading, error } = useReleaseInbox(inboxParams);
  const { data: facets } = useReleaseFacets(facetsParams);
  const { data: sources } = useReleaseSources();
  const dismiss = useDismissRelease();
  const markAcquired = useMarkReleaseAcquired();
  const deleteRelease = useDeleteRelease();
  const bulk = useBulkReleaseAction();

  const entries = data?.data ?? [];
  const total = data?.total ?? 0;
  const totalPages = data?.totalPages ?? 1;

  // Reset bulk selection when the visible page or any filter changes —
  // selection IDs don't apply across different pages or filtered views.
  // The deps are *triggers*, not values used in the body, so biome's
  // exhaustive-deps rule flags them as extra; that's intentional here.
  // biome-ignore lint/correctness/useExhaustiveDependencies: deps are change-triggers, not consumed values
  useEffect(() => {
    setSelected(new Set());
  }, [page, state, language, seriesId, libraryId]);

  const seriesOptions = useMemo(() => buildSeriesOptions(facets), [facets]);
  const libraryOptions = useMemo(() => buildLibraryOptions(facets), [facets]);
  const languageOptions = useMemo(() => buildLanguageOptions(facets), [facets]);
  // Joining `sources` client-side keeps the inbox DTO lean: the source list
  // is small and already cached, so a per-row label costs no extra fetch.
  const sourceById = useMemo(() => {
    const map = new Map<string, ReleaseSource>();
    for (const s of sources ?? []) map.set(s.id, s);
    return map;
  }, [sources]);

  const allOnPageSelected =
    entries.length > 0 && entries.every((e) => selected.has(e.id));

  const toggleAllOnPage = () => {
    setSelected((prev) => {
      if (allOnPageSelected) {
        const next = new Set(prev);
        for (const e of entries) next.delete(e.id);
        return next;
      }
      const next = new Set(prev);
      for (const e of entries) next.add(e.id);
      return next;
    });
  };

  const toggleOne = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const runBulk = (action: BulkReleaseAction) => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    bulk.mutate(
      { ids, action },
      {
        onSuccess: () => setSelected(new Set()),
      },
    );
  };

  return (
    <Stack p="md" gap="md">
      <Group justify="space-between" wrap="wrap">
        <Group gap="sm">
          <IconRss size={26} />
          <Title order={2}>Releases</Title>
          <Badge size="md" variant="light">
            {total} total
          </Badge>
        </Group>
      </Group>

      <Card withBorder padding="md" radius="md">
        <Group gap="md" wrap="wrap" align="flex-end">
          <Select
            label="State"
            data={STATE_OPTIONS}
            value={state}
            onChange={(value) => {
              setState(value ?? "announced");
              setPage(1);
            }}
            w={160}
            allowDeselect={false}
            data-testid="releases-state-filter"
          />
          <Select
            label="Library"
            data={libraryOptions}
            value={libraryId}
            onChange={(value) => {
              setLibraryId(value ?? ALL_VALUE);
              setPage(1);
            }}
            w={220}
            allowDeselect={false}
            searchable
            comboboxProps={{ withinPortal: true }}
          />
          <Select
            label="Language"
            data={languageOptions}
            value={language}
            onChange={(value) => {
              setLanguage(value ?? ALL_VALUE);
              setPage(1);
            }}
            w={180}
            allowDeselect={false}
            searchable
            comboboxProps={{ withinPortal: true }}
          />
          <Select
            label="Series"
            data={seriesOptions}
            value={seriesId}
            onChange={(value) => {
              setSeriesId(value ?? ALL_VALUE);
              setPage(1);
            }}
            w={320}
            allowDeselect={false}
            searchable
            nothingFoundMessage="No series with releases"
            comboboxProps={{ withinPortal: true }}
          />
        </Group>
      </Card>

      {selected.size > 0 && (
        <ReleasesBulkActionBar
          count={selected.size}
          isPending={bulk.isPending}
          onAction={runBulk}
          onClear={() => setSelected(new Set())}
          onDeleteClick={openBulkDelete}
          sticky
        />
      )}

      {error && (
        <Card withBorder padding="md" radius="md">
          <Text size="sm" c="red">
            Failed to load releases:{" "}
            {error instanceof Error ? error.message : String(error)}
          </Text>
        </Card>
      )}

      {isLoading ? (
        <Group justify="center" py="md">
          <Loader />
        </Group>
      ) : entries.length === 0 ? (
        <Card withBorder padding="md" radius="md">
          <Text size="sm" c="dimmed">
            No releases match these filters. New chapters and volumes show up
            here once a release source picks them up.
          </Text>
        </Card>
      ) : (
        <Card withBorder padding={0} radius="md">
          <ReleasesTable
            entries={entries}
            sourceById={sourceById}
            selected={selected}
            onToggleOne={toggleOne}
            onToggleAll={toggleAllOnPage}
            onDismiss={(id) => dismiss.mutate(id)}
            onMarkAcquired={(id) => markAcquired.mutate(id)}
            onDelete={(id) => deleteRelease.mutate(id)}
            showSeriesColumn
            isDismissPending={dismiss.isPending}
            isMarkAcquiredPending={markAcquired.isPending}
            isDeletePending={deleteRelease.isPending}
            verticalSpacing="sm"
          />
        </Card>
      )}

      {totalPages > 1 && (
        <Group justify="center">
          <Pagination
            total={totalPages}
            value={page}
            onChange={setPage}
            size="sm"
          />
        </Group>
      )}

      <ReleasesBulkDeleteModal
        opened={confirmBulkDelete}
        onClose={closeBulkDelete}
        onConfirm={() => {
          runBulk("delete");
          closeBulkDelete();
        }}
        count={selected.size}
        isPending={bulk.isPending}
      />
    </Stack>
  );
}
