import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Card,
  Checkbox,
  Group,
  Loader,
  Modal,
  Pagination,
  Select,
  Stack,
  Table,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import {
  IconCheck,
  IconExternalLink,
  IconRss,
  IconTrash,
  IconX,
} from "@tabler/icons-react";
import { format } from "date-fns";
import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type {
  BulkReleaseAction,
  ReleaseFacets,
  ReleaseFacetsParams,
  ReleaseInboxParams,
  ReleaseLedgerEntry,
} from "@/api/releases";
import { MediaUrlIcon } from "@/components/releases/MediaUrlIcon";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import {
  useBulkReleaseAction,
  useDeleteRelease,
  useDismissRelease,
  useMarkReleaseAcquired,
  useReleaseFacets,
  useReleaseInbox,
} from "@/hooks/useReleases";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";

const STATE_OPTIONS = [
  { value: "all", label: "All" },
  { value: "announced", label: "New" },
  { value: "marked_acquired", label: "Acquired" },
  { value: "dismissed", label: "Dismissed" },
];

const STATE_BADGE: Record<string, { color: string; label: string }> = {
  announced: { color: "blue", label: "New" },
  marked_acquired: { color: "green", label: "Acquired" },
  dismissed: { color: "gray", label: "Dismissed" },
  hidden: { color: "gray", label: "Hidden" },
};

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

  const allOnPageSelected =
    entries.length > 0 && entries.every((e) => selected.has(e.id));
  const someOnPageSelected =
    entries.some((e) => selected.has(e.id)) && !allOnPageSelected;

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
    <Box p="md">
      <Stack gap="md">
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
          <Card
            withBorder
            padding="sm"
            radius="md"
            style={{ position: "sticky", top: 0, zIndex: 2 }}
          >
            <Group justify="space-between" wrap="wrap">
              <Text size="sm" fw={500}>
                {selected.size} selected
              </Text>
              <Group gap="xs">
                <Button
                  size="xs"
                  variant="light"
                  color="green"
                  leftSection={<IconCheck size={14} />}
                  loading={bulk.isPending}
                  onClick={() => runBulk("mark-acquired")}
                >
                  Mark acquired
                </Button>
                <Button
                  size="xs"
                  variant="light"
                  color="gray"
                  leftSection={<IconX size={14} />}
                  loading={bulk.isPending}
                  onClick={() => runBulk("dismiss")}
                >
                  Dismiss
                </Button>
                <Button
                  size="xs"
                  variant="light"
                  color="red"
                  leftSection={<IconTrash size={14} />}
                  loading={bulk.isPending}
                  onClick={openBulkDelete}
                >
                  Delete
                </Button>
                <Button
                  size="xs"
                  variant="subtle"
                  onClick={() => setSelected(new Set())}
                >
                  Clear
                </Button>
              </Group>
            </Group>
          </Card>
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
            <Table verticalSpacing="sm" highlightOnHover>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th w={36}>
                    <Checkbox
                      aria-label="Select all on page"
                      checked={allOnPageSelected}
                      indeterminate={someOnPageSelected}
                      onChange={toggleAllOnPage}
                    />
                  </Table.Th>
                  <Table.Th>Series</Table.Th>
                  <Table.Th>Ch / Vol</Table.Th>
                  <Table.Th>Source / Group</Table.Th>
                  <Table.Th>Lang</Table.Th>
                  <Table.Th>State</Table.Th>
                  <Table.Th>Observed</Table.Th>
                  <Table.Th aria-label="Actions" />
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {entries.map((entry: ReleaseLedgerEntry) => {
                  const stateInfo = STATE_BADGE[entry.state] ?? {
                    color: "gray",
                    label: entry.state,
                  };
                  const isSelected = selected.has(entry.id);
                  return (
                    <Table.Tr
                      key={entry.id}
                      bg={
                        isSelected
                          ? "var(--mantine-color-blue-light)"
                          : undefined
                      }
                    >
                      <Table.Td>
                        <Checkbox
                          aria-label={`Select release ${entry.id}`}
                          checked={isSelected}
                          onChange={() => toggleOne(entry.id)}
                        />
                      </Table.Td>
                      <Table.Td>
                        <Anchor
                          component={Link}
                          to={`/series/${entry.seriesId}#releases`}
                          size="sm"
                          lineClamp={1}
                        >
                          {entry.seriesTitle.length > 0
                            ? entry.seriesTitle
                            : `${entry.seriesId.slice(0, 8)}…`}
                        </Anchor>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" fw={500}>
                          {entry.chapter !== null && entry.chapter !== undefined
                            ? `Ch ${entry.chapter}`
                            : ""}
                          {entry.volume !== null && entry.volume !== undefined
                            ? entry.chapter !== null &&
                              entry.chapter !== undefined
                              ? ` · Vol ${entry.volume}`
                              : `Vol ${entry.volume}`
                            : ""}
                          {!entry.chapter && !entry.volume ? "—" : ""}
                        </Text>
                      </Table.Td>
                      <Table.Td>
                        <Stack gap={2}>
                          {entry.groupOrUploader && (
                            <Text size="sm">{entry.groupOrUploader}</Text>
                          )}
                          <Text size="xs" c="dimmed">
                            source: {entry.sourceId.slice(0, 8)}…
                          </Text>
                        </Stack>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">{entry.language ?? "—"}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Badge
                          color={stateInfo.color}
                          variant="light"
                          size="sm"
                        >
                          {stateInfo.label}
                        </Badge>
                      </Table.Td>
                      <Table.Td>
                        <Text size="xs" c="dimmed">
                          {format(new Date(entry.observedAt), "yyyy-MM-dd")}
                        </Text>
                      </Table.Td>
                      <Table.Td>
                        <Group gap={4} justify="flex-end" wrap="nowrap">
                          <Tooltip label="Open payload URL">
                            <ActionIcon
                              component="a"
                              href={entry.payloadUrl}
                              target="_blank"
                              rel="noopener noreferrer"
                              variant="subtle"
                              size="sm"
                              aria-label="Open payload URL"
                            >
                              <IconExternalLink size={16} />
                            </ActionIcon>
                          </Tooltip>
                          {entry.mediaUrl && (
                            <MediaUrlIcon
                              url={entry.mediaUrl}
                              kind={entry.mediaUrlKind}
                            />
                          )}
                          {entry.state === "announced" && (
                            <>
                              <Tooltip label="Mark acquired">
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  color="green"
                                  loading={markAcquired.isPending}
                                  onClick={() => markAcquired.mutate(entry.id)}
                                  aria-label="Mark acquired"
                                >
                                  <IconCheck size={16} />
                                </ActionIcon>
                              </Tooltip>
                              <Tooltip label="Dismiss">
                                <ActionIcon
                                  variant="subtle"
                                  size="sm"
                                  color="gray"
                                  loading={dismiss.isPending}
                                  onClick={() => dismiss.mutate(entry.id)}
                                  aria-label="Dismiss"
                                >
                                  <IconX size={16} />
                                </ActionIcon>
                              </Tooltip>
                            </>
                          )}
                          <Tooltip label="Delete (will reappear on next poll)">
                            <ActionIcon
                              variant="subtle"
                              size="sm"
                              color="red"
                              loading={deleteRelease.isPending}
                              onClick={() => deleteRelease.mutate(entry.id)}
                              aria-label="Delete"
                            >
                              <IconTrash size={16} />
                            </ActionIcon>
                          </Tooltip>
                        </Group>
                      </Table.Td>
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>
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
      </Stack>

      <Modal
        opened={confirmBulkDelete}
        onClose={closeBulkDelete}
        title="Delete releases?"
        centered
      >
        <Stack gap="md">
          <Text size="sm">
            This will hard-delete {selected.size}{" "}
            {selected.size === 1 ? "release" : "releases"} from the ledger and
            clear the affected sources' cache so they re-fetch on the next poll.
            The releases will reappear if the upstream still lists them.
          </Text>
          <Group justify="flex-end" gap="xs">
            <Button variant="subtle" onClick={closeBulkDelete}>
              Cancel
            </Button>
            <Button
              color="red"
              loading={bulk.isPending}
              onClick={() => {
                runBulk("delete");
                closeBulkDelete();
              }}
            >
              Delete {selected.size}{" "}
              {selected.size === 1 ? "release" : "releases"}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
