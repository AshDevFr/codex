import {
  Alert,
  Anchor,
  Badge,
  Button,
  Collapse,
  Drawer,
  Group,
  Stack,
  Text,
} from "@mantine/core";
import { useDisclosure } from "@mantine/hooks";
import { IconArrowUpCircle } from "@tabler/icons-react";
import { useEffect, useRef, useState } from "react";
import type { NewsEntryDto, NewsResponse } from "@/api/news";
import { MarkdownContent } from "@/components/common/MarkdownContent";
import { useMarkNewsSeen } from "@/hooks/useNews";
import { formatNewsDate } from "./format";

const ALL_UPDATES_URL = "https://updates.4sh.dev/codex/";

const PROJECT_LABELS: Record<string, { label: string; color: string }> = {
  codex: { label: "Codex", color: "orange" },
  "codex-ios": { label: "Codex Reader", color: "blue" },
};

interface NewsDrawerProps {
  opened: boolean;
  onClose: () => void;
  news: NewsResponse;
}

export function NewsDrawer({ opened, onClose, news }: NewsDrawerProps) {
  const markSeen = useMarkNewsSeen();
  // What was unread when the drawer opened. Marking seen refetches and
  // clears the server's flags, but the admin should still see what was new
  // while the drawer stays open.
  const [newIds, setNewIds] = useState<Set<string>>(new Set());
  const wasOpened = useRef(false);

  useEffect(() => {
    if (opened && !wasOpened.current) {
      setNewIds(new Set(news.entries.filter((e) => e.unread).map((e) => e.id)));
      markSeen.mutate();
    }
    wasOpened.current = opened;
  }, [opened, news.entries, markSeen]);

  const update = news.updateAvailable;

  return (
    <Drawer
      opened={opened}
      onClose={onClose}
      position="right"
      title={
        <Text fw={600} size="lg">
          News
        </Text>
      }
    >
      <Stack gap="md">
        {update && (
          <Alert
            color="orange"
            variant="light"
            icon={<IconArrowUpCircle size={18} />}
            title={`Codex ${update.latestVersion} is available`}
          >
            <Text size="sm">
              You are running {update.currentVersion}.{" "}
              {update.url && (
                <Anchor
                  href={update.url}
                  target="_blank"
                  rel="noopener noreferrer"
                  size="sm"
                >
                  Release notes
                </Anchor>
              )}
            </Text>
          </Alert>
        )}

        {news.entries.length === 0 ? (
          <Text c="dimmed" size="sm">
            No news yet.
          </Text>
        ) : (
          news.entries.map((entry) => (
            <NewsEntry
              key={entry.id}
              entry={entry}
              isNew={newIds.has(entry.id)}
            />
          ))
        )}

        <Anchor
          href={ALL_UPDATES_URL}
          target="_blank"
          rel="noopener noreferrer"
          size="sm"
        >
          All updates
        </Anchor>
      </Stack>
    </Drawer>
  );
}

function NewsEntry({ entry, isNew }: { entry: NewsEntryDto; isNew: boolean }) {
  const [expanded, { toggle }] = useDisclosure(false);
  const project = PROJECT_LABELS[entry.project] ?? {
    label: entry.project,
    color: "gray",
  };
  const published = formatNewsDate(entry.publishedAt);

  return (
    <Stack gap={4}>
      <Group gap="xs" justify="space-between" wrap="nowrap">
        <Group gap="xs" wrap="nowrap">
          <Badge size="xs" variant="light" color={project.color}>
            {project.label}
          </Badge>
          {isNew && (
            <Badge size="xs" variant="filled" color="blue">
              New
            </Badge>
          )}
        </Group>
        <Text size="xs" c="dimmed">
          {published}
        </Text>
      </Group>
      {entry.url ? (
        <Anchor
          href={entry.url}
          target="_blank"
          rel="noopener noreferrer"
          fw={600}
          size="sm"
        >
          {entry.title}
        </Anchor>
      ) : (
        <Text fw={600} size="sm">
          {entry.title}
        </Text>
      )}
      <Text size="sm" c="dimmed">
        {entry.summary}
      </Text>
      {entry.body.trim() !== "" &&
        entry.body.trim() !== entry.summary.trim() && (
          <>
            <Collapse in={expanded}>
              <MarkdownContent compact>{entry.body}</MarkdownContent>
            </Collapse>
            <Button
              variant="subtle"
              size="compact-xs"
              onClick={toggle}
              style={{ alignSelf: "flex-start" }}
            >
              {expanded ? "Show less" : "Show more"}
            </Button>
          </>
        )}
    </Stack>
  );
}
