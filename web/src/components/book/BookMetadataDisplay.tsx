import { Badge, Grid, Group, Paper, Stack, Text, Title } from "@mantine/core";
import type { BookMetadata } from "@/api/books";

interface BookMetadataDisplayProps {
  metadata: BookMetadata | null | undefined;
}

// Map language codes to display names
const LANGUAGE_DISPLAY: Record<string, string> = {
  en: "English",
  ja: "Japanese",
  ko: "Korean",
  zh: "Chinese",
  fr: "French",
  de: "German",
  es: "Spanish",
  it: "Italian",
  pt: "Portuguese",
  ru: "Russian",
};

interface MetadataItemProps {
  label: string;
  value: string | number | null | undefined;
}

function MetadataItem({ label, value }: MetadataItemProps) {
  if (!value && value !== 0) return null;

  return (
    <Paper p="sm" radius="sm" withBorder>
      <Stack gap={2}>
        <Text size="xs" c="dimmed" tt="uppercase" fw={500}>
          {label}
        </Text>
        <Text size="sm" fw={500}>
          {value}
        </Text>
      </Stack>
    </Paper>
  );
}

interface CreatorListProps {
  label: string;
  creators: string[];
}

function CreatorList({ label, creators }: CreatorListProps) {
  if (creators.length === 0) return null;

  return (
    <Group gap="xs" wrap="wrap">
      <Text size="sm" fw={500} c="dimmed">
        {label}:
      </Text>
      {creators.map((creator, index) => (
        <Badge
          // biome-ignore lint/suspicious/noArrayIndexKey: Index needed for duplicate creator names
          key={`${creator}-${index}`}
          variant="light"
          size="sm"
        >
          {creator}
        </Badge>
      ))}
    </Group>
  );
}

export function BookMetadataDisplay({ metadata }: BookMetadataDisplayProps) {
  if (!metadata) {
    return null;
  }

  const languageDisplay = metadata.languageIso
    ? LANGUAGE_DISPLAY[metadata.languageIso] || metadata.languageIso
    : null;

  const releaseYear = metadata.releaseDate
    ? new Date(metadata.releaseDate).getFullYear()
    : null;

  // Basic metadata items
  const items = [
    { label: "Number", value: metadata.number },
    { label: "Year", value: releaseYear },
    { label: "Publisher", value: metadata.publisher },
    { label: "Imprint", value: metadata.imprint },
    { label: "Language", value: languageDisplay },
    { label: "Genre", value: metadata.genre },
  ].filter((item) => item.value !== null && item.value !== undefined);

  // Creator lists
  const creatorLists = [
    { label: "Writers", creators: metadata.writers || [] },
    { label: "Pencillers", creators: metadata.pencillers || [] },
    { label: "Inkers", creators: metadata.inkers || [] },
    { label: "Colorists", creators: metadata.colorists || [] },
    { label: "Letterers", creators: metadata.letterers || [] },
    { label: "Cover Artists", creators: metadata.coverArtists || [] },
    { label: "Editors", creators: metadata.editors || [] },
  ].filter((list) => list.creators.length > 0);

  const hasContent =
    items.length > 0 || creatorLists.length > 0 || metadata.summary;

  if (!hasContent) {
    return null;
  }

  return (
    <Stack gap="md">
      <Title order={4}>Metadata</Title>

      {items.length > 0 && (
        <Grid gutter="sm">
          {items.map((item) => (
            <Grid.Col key={item.label} span={{ base: 6, sm: 4, md: 3, lg: 2 }}>
              <MetadataItem label={item.label} value={item.value} />
            </Grid.Col>
          ))}
        </Grid>
      )}

      {creatorLists.length > 0 && (
        <Stack gap="xs">
          {creatorLists.map((list) => (
            <CreatorList
              key={list.label}
              label={list.label}
              creators={list.creators}
            />
          ))}
        </Stack>
      )}

      {metadata.summary && (
        <Stack gap="xs">
          <Text size="sm" fw={500} c="dimmed">
            Summary
          </Text>
          <Text style={{ whiteSpace: "pre-wrap" }}>{metadata.summary}</Text>
        </Stack>
      )}
    </Stack>
  );
}
