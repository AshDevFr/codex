import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Card,
  Group,
  Loader,
  Pagination,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
  Title,
  Tooltip,
} from "@mantine/core";
import {
  IconCheck,
  IconExternalLink,
  IconRss,
  IconX,
} from "@tabler/icons-react";
import { format } from "date-fns";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import type { ReleaseLedgerEntry } from "@/api/releases";
import { MediaUrlIcon } from "@/components/releases/MediaUrlIcon";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import {
  useDismissRelease,
  useMarkReleaseAcquired,
  useReleaseInbox,
} from "@/hooks/useReleases";
import { useReleaseAnnouncementsStore } from "@/store/releaseAnnouncementsStore";

const STATE_OPTIONS = [
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

export function ReleasesInbox() {
  useDocumentTitle("Releases");

  const resetBadge = useReleaseAnnouncementsStore((s) => s.reset);
  // Visiting the inbox marks all unseen events as seen — the user has
  // landed where the events would have sent them anyway.
  useEffect(() => {
    resetBadge();
  }, [resetBadge]);

  const [state, setState] = useState<string>("announced");
  const [language, setLanguage] = useState<string>("");
  const [seriesIdFilter, setSeriesIdFilter] = useState<string>("");
  const [page, setPage] = useState<number>(1);

  const params = {
    state,
    language: language.trim() || undefined,
    seriesId: seriesIdFilter.trim() || undefined,
    page,
    pageSize: PAGE_SIZE,
  };

  const { data, isLoading, error } = useReleaseInbox(params);
  const dismiss = useDismissRelease();
  const markAcquired = useMarkReleaseAcquired();

  const entries = data?.data ?? [];
  const total = data?.total ?? 0;
  const totalPages = data?.totalPages ?? 1;

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
          <Group gap="md" wrap="wrap">
            <Select
              label="State"
              data={STATE_OPTIONS}
              value={state}
              onChange={(value) => {
                setState(value ?? "announced");
                setPage(1);
              }}
              w={180}
            />
            <TextInput
              label="Language"
              placeholder="en"
              value={language}
              onChange={(e) => {
                setLanguage(e.currentTarget.value);
                setPage(1);
              }}
              w={140}
            />
            <TextInput
              label="Series ID"
              placeholder="Optional UUID"
              value={seriesIdFilter}
              onChange={(e) => {
                setSeriesIdFilter(e.currentTarget.value);
                setPage(1);
              }}
              w={320}
            />
          </Group>
        </Card>

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
                  return (
                    <Table.Tr key={entry.id}>
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
                                  color="red"
                                  loading={dismiss.isPending}
                                  onClick={() => dismiss.mutate(entry.id)}
                                  aria-label="Dismiss"
                                >
                                  <IconX size={16} />
                                </ActionIcon>
                              </Tooltip>
                            </>
                          )}
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
    </Box>
  );
}
