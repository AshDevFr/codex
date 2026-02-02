import {
  Alert,
  Badge,
  Button,
  Center,
  Code,
  Group,
  Loader,
  Paper,
  ScrollArea,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconAlertCircle,
  IconBooks,
  IconFolder,
  IconRefresh,
} from "@tabler/icons-react";
import { useMutation } from "@tanstack/react-query";
import { librariesApi } from "@/api/libraries";
import type {
  DetectedSeries,
  PreviewScanResponse,
  SeriesStrategy,
} from "@/types";

interface PreviewScanPanelProps {
  path: string;
  seriesStrategy: SeriesStrategy;
  seriesConfig: Record<string, unknown>;
  onScanComplete?: (result: PreviewScanResponse) => void;
}

export function PreviewScanPanel({
  path,
  seriesStrategy,
  seriesConfig,
  onScanComplete,
}: PreviewScanPanelProps) {
  const previewMutation = useMutation({
    mutationFn: () =>
      librariesApi.previewScan({
        path,
        seriesStrategy,
        seriesConfig:
          Object.keys(seriesConfig).length > 0 ? seriesConfig : undefined,
      }),
    onSuccess: (result) => {
      onScanComplete?.(result);
    },
  });

  const handlePreview = () => {
    if (path.trim()) {
      previewMutation.mutate();
    }
  };

  if (!path.trim()) {
    return (
      <Alert
        icon={<IconAlertCircle size={16} />}
        color="yellow"
        variant="light"
      >
        <Text size="sm">
          Select a library path first to preview series detection.
        </Text>
      </Alert>
    );
  }

  return (
    <Stack gap="md">
      <Group justify="space-between" align="center">
        <Text fw={500}>Preview Scan Results</Text>
        <Button
          size="xs"
          variant="light"
          leftSection={<IconRefresh size={16} />}
          onClick={handlePreview}
          loading={previewMutation.isPending}
        >
          {previewMutation.data ? "Rescan" : "Preview"}
        </Button>
      </Group>

      {previewMutation.isPending && (
        <Center py="xl">
          <Stack align="center" gap="sm">
            <Loader size="md" />
            <Text size="sm" c="dimmed">
              Scanning folder structure...
            </Text>
          </Stack>
        </Center>
      )}

      {previewMutation.isError && (
        <Alert icon={<IconAlertCircle size={16} />} color="red" variant="light">
          <Text size="sm">
            {previewMutation.error instanceof Error
              ? previewMutation.error.message
              : "Failed to preview scan. Check that the path is accessible."}
          </Text>
        </Alert>
      )}

      {previewMutation.data && !previewMutation.isPending && (
        <PreviewResults data={previewMutation.data} />
      )}

      {!previewMutation.data && !previewMutation.isPending && (
        <Paper p="md" withBorder>
          <Stack align="center" gap="sm">
            <IconFolder size={32} style={{ opacity: 0.5 }} />
            <Text size="sm" c="dimmed" ta="center">
              Click "Preview" to see how your folder structure will be detected
              with the selected strategy.
            </Text>
          </Stack>
        </Paper>
      )}
    </Stack>
  );
}

interface PreviewResultsProps {
  data: PreviewScanResponse;
}

function PreviewResults({ data }: PreviewResultsProps) {
  const { detectedSeries, totalFiles } = data;

  return (
    <Stack gap="sm">
      <Group gap="md">
        <Badge color="blue" variant="light" size="lg">
          {detectedSeries.length} series detected
        </Badge>
        <Badge color="gray" variant="light" size="lg">
          {totalFiles} files found
        </Badge>
      </Group>

      {detectedSeries.length === 0 ? (
        <Alert
          icon={<IconAlertCircle size={16} />}
          color="yellow"
          variant="light"
        >
          <Text size="sm">
            No series detected. Try a different strategy or check your folder
            structure.
          </Text>
        </Alert>
      ) : (
        <ScrollArea h={300} type="auto">
          <Stack gap="xs">
            {detectedSeries.map((series, index) => (
              <SeriesPreviewCard
                key={`${series.name}-${index}`}
                series={series}
              />
            ))}
          </Stack>
        </ScrollArea>
      )}
    </Stack>
  );
}

interface SeriesPreviewCardProps {
  series: DetectedSeries;
}

function SeriesPreviewCard({ series }: SeriesPreviewCardProps) {
  const sampleBooks = series.sampleBooks || [];
  const hasMoreBooks = series.bookCount > sampleBooks.length;

  return (
    <Paper p="sm" withBorder>
      <Stack gap="xs">
        <Group justify="space-between" align="flex-start">
          <Group gap="xs" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
            <IconBooks size={16} style={{ flexShrink: 0 }} />
            <Text fw={500} truncate style={{ flex: 1 }}>
              {series.name}
            </Text>
          </Group>
          <Badge color="gray" variant="light" size="sm">
            {series.bookCount} books
          </Badge>
        </Group>

        <Tooltip label={series.path} multiline maw={400}>
          <Text size="xs" c="dimmed" truncate>
            {series.path}
          </Text>
        </Tooltip>

        {series.metadata && Object.keys(series.metadata).length > 0 && (
          <Group gap="xs">
            {series.metadata.publisher && (
              <Badge color="violet" variant="light" size="xs">
                {series.metadata.publisher}
              </Badge>
            )}
            {series.metadata.author && (
              <Badge color="teal" variant="light" size="xs">
                {series.metadata.author}
              </Badge>
            )}
          </Group>
        )}

        {sampleBooks.length > 0 && (
          <Stack gap={2}>
            <Text size="xs" c="dimmed">
              Sample books:
            </Text>
            {sampleBooks.slice(0, 3).map((book, idx) => (
              <Code
                // biome-ignore lint/suspicious/noArrayIndexKey: Static list, index is safe
                key={idx}
                style={{ fontSize: "11px" }}
              >
                {book}
              </Code>
            ))}
            {hasMoreBooks && (
              <Text size="xs" c="dimmed" fs="italic">
                ...and {series.bookCount - sampleBooks.length} more
              </Text>
            )}
          </Stack>
        )}
      </Stack>
    </Paper>
  );
}
