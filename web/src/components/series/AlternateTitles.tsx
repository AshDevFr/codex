import { Group, List, Text } from "@mantine/core";
import type { AlternateTitle } from "@/api/seriesMetadata";

interface AlternateTitlesProps {
  titles: AlternateTitle[];
  compact?: boolean;
}

export function AlternateTitles({ titles, compact }: AlternateTitlesProps) {
  if (titles.length === 0) {
    return null;
  }

  // Compact mode: inline display like Komga
  if (compact) {
    return (
      <Group gap="md">
        {titles.map((title) => (
          <Text key={title.id} size="xs" c="dimmed">
            <Text component="span" tt="uppercase" fw={500}>
              {title.label}
            </Text>{" "}
            {title.title}
          </Text>
        ))}
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
