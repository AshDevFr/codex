import { Group, List, Text } from "@mantine/core";
import { useState } from "react";
import type { AlternateTitle } from "@/api/seriesMetadata";

/** Labels that are always shown when in compact mode */
const PRIORITY_LABELS = new Set(["en", "english", "native", "romaji"]);

interface AlternateTitlesProps {
  titles: AlternateTitle[];
  compact?: boolean;
}

export function AlternateTitles({ titles, compact }: AlternateTitlesProps) {
  const [expanded, setExpanded] = useState(false);

  if (titles.length === 0) {
    return null;
  }

  // Compact mode: inline display like Komga, with collapsible overflow
  if (compact) {
    const priorityTitles = titles.filter((t) =>
      PRIORITY_LABELS.has(t.label.toLowerCase()),
    );
    const otherTitles = titles.filter(
      (t) => !PRIORITY_LABELS.has(t.label.toLowerCase()),
    );
    const displayTitles = expanded ? titles : priorityTitles;
    const hiddenCount = otherTitles.length;

    return (
      <Group gap="md">
        {displayTitles.map((title) => (
          <Text key={title.id} size="xs" c="dimmed">
            <Text component="span" tt="uppercase" fw={500}>
              {title.label}
            </Text>{" "}
            {title.title}
          </Text>
        ))}
        {!expanded && hiddenCount > 0 && (
          <Text
            size="xs"
            c="dimmed"
            style={{ cursor: "pointer" }}
            onClick={() => setExpanded(true)}
          >
            +{hiddenCount} more
          </Text>
        )}
        {expanded && hiddenCount > 0 && (
          <Text
            size="xs"
            c="dimmed"
            style={{ cursor: "pointer" }}
            onClick={() => setExpanded(false)}
          >
            Show less
          </Text>
        )}
      </Group>
    );
  }

  return (
    <List size="sm" spacing="xs">
      {titles.map((title) => (
        <List.Item key={title.id}>
          <Text component="span" size="sm" c="dimmed">
            {title.label}:{" "}
          </Text>
          <Text component="span" size="sm">
            {title.title}
          </Text>
        </List.Item>
      ))}
    </List>
  );
}
