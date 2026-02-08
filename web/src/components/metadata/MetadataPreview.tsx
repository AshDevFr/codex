import {
  Badge,
  Button,
  Center,
  Checkbox,
  Group,
  Image,
  Loader,
  ScrollArea,
  Stack,
  Table,
  Text,
  ThemeIcon,
  Tooltip,
} from "@mantine/core";
import {
  IconArrowRight,
  IconCheck,
  IconEqual,
  IconLock,
  IconMinus,
  IconShieldOff,
} from "@tabler/icons-react";
import { useMutation } from "@tanstack/react-query";
import type {
  FieldApplyStatus,
  MetadataFieldPreview,
  MetadataPreviewResponse,
} from "@/api/plugins";
import { pluginActionsApi } from "@/api/plugins";

export interface MetadataPreviewProps {
  /** Series or Book ID */
  seriesId: string;
  /** Plugin ID */
  pluginId: string;
  /** External ID from search result */
  externalId: string;
  /** Plugin display name */
  pluginName: string;
  /** Content type */
  contentType?: "series" | "book";
  /** Callback when apply is complete */
  onApplyComplete?: (success: boolean, appliedFields: string[]) => void;
  /** Callback to go back to search */
  onBack?: () => void;
}

interface StatusConfig {
  icon: React.ReactNode;
  color: string;
  label: string;
}

const STATUS_CONFIG: Record<FieldApplyStatus, StatusConfig> = {
  will_apply: {
    icon: <IconCheck size={14} />,
    color: "green",
    label: "Will be applied",
  },
  locked: {
    icon: <IconLock size={14} />,
    color: "yellow",
    label: "Field is locked",
  },
  no_permission: {
    icon: <IconShieldOff size={14} />,
    color: "red",
    label: "Plugin lacks permission",
  },
  unchanged: {
    icon: <IconEqual size={14} />,
    color: "gray",
    label: "Value unchanged",
  },
  not_provided: {
    icon: <IconMinus size={14} />,
    color: "gray",
    label: "Not provided by plugin",
  },
};

const FIELD_LABELS: Record<string, string> = {
  // Series fields
  title: "Title",
  alternateTitles: "Alternate Titles",
  summary: "Summary",
  year: "Year",
  status: "Status",
  publisher: "Publisher",
  genres: "Genres",
  tags: "Tags",
  language: "Language",
  ageRating: "Age Rating",
  readingDirection: "Reading Direction",
  totalBookCount: "Total Books",
  externalLinks: "External Links",
  rating: "Rating",
  externalRatings: "External Ratings",
  externalIds: "External IDs",
  coverUrl: "Cover",
  // Book-specific fields
  bookType: "Book Type",
  subtitle: "Subtitle",
  authors: "Authors",
  translator: "Translator",
  edition: "Edition",
  originalTitle: "Original Title",
  originalYear: "Original Year",
  isbns: "ISBNs",
  seriesPosition: "Series Position",
  seriesTotal: "Series Total",
  subjects: "Subjects",
  awards: "Awards",
};

/**
 * Component to preview metadata changes before applying
 *
 * Shows a table with:
 * - Checkbox to select/deselect field
 * - Field name
 * - Current value
 * - Proposed value
 * - Status icon (will apply, locked, no permission, unchanged, not provided)
 */
export function MetadataPreview({
  seriesId,
  pluginId,
  externalId,
  pluginName,
  contentType = "series",
  onApplyComplete,
  onBack,
}: MetadataPreviewProps) {
  // Track which fields are selected for application
  const [selectedFields, setSelectedFields] = React.useState<Set<string>>(
    new Set(),
  );
  const [initialized, setInitialized] = React.useState(false);

  // Fetch preview data
  const previewMutation = useMutation({
    mutationFn: async () => {
      if (contentType === "book") {
        return pluginActionsApi.previewBookMetadata(
          seriesId,
          pluginId,
          externalId,
        );
      }
      return pluginActionsApi.previewSeriesMetadata(
        seriesId,
        pluginId,
        externalId,
      );
    },
  });

  // Initialize selected fields when preview data is loaded
  React.useEffect(() => {
    if (previewMutation.data && !initialized) {
      const applyableFields = previewMutation.data.fields
        .filter((f) => f.status === "will_apply")
        .map((f) => f.field);
      setSelectedFields(new Set(applyableFields));
      setInitialized(true);
    }
  }, [previewMutation.data, initialized]);

  // Toggle field selection
  const toggleField = (field: string) => {
    setSelectedFields((prev) => {
      const next = new Set(prev);
      if (next.has(field)) {
        next.delete(field);
      } else {
        next.add(field);
      }
      return next;
    });
  };

  // Apply metadata mutation
  const applyMutation = useMutation({
    mutationFn: async () => {
      const fieldsArray =
        selectedFields.size > 0 ? Array.from(selectedFields) : undefined;
      if (contentType === "book") {
        return pluginActionsApi.applyBookMetadata(
          seriesId,
          pluginId,
          externalId,
          fieldsArray,
        );
      }
      return pluginActionsApi.applySeriesMetadata(
        seriesId,
        pluginId,
        externalId,
        fieldsArray,
      );
    },
    onSuccess: (data) => {
      onApplyComplete?.(data.success, data.appliedFields);
    },
  });

  // Fetch preview on mount
  // biome-ignore lint/correctness/useExhaustiveDependencies: only run on mount
  React.useEffect(() => {
    previewMutation.mutate();
  }, []);

  if (previewMutation.isPending) {
    return (
      <Center py="xl">
        <Stack align="center" gap="md">
          <Loader size="lg" />
          <Text c="dimmed">Fetching metadata from {pluginName}...</Text>
        </Stack>
      </Center>
    );
  }

  if (previewMutation.isError) {
    return (
      <Center py="xl">
        <Stack align="center" gap="md">
          <Text c="red">
            {previewMutation.error?.message || "Failed to fetch metadata"}
          </Text>
          <Group gap="sm">
            {onBack && (
              <Button variant="light" onClick={onBack}>
                Back to Search
              </Button>
            )}
            <Button onClick={() => previewMutation.mutate()}>Retry</Button>
          </Group>
        </Stack>
      </Center>
    );
  }

  const preview = previewMutation.data;
  if (!preview) {
    return null;
  }

  const canApply = selectedFields.size > 0;

  return (
    <Stack gap="md">
      {/* Header */}
      <Group justify="space-between">
        <Stack gap={4}>
          <Text size="sm" c="dimmed">
            Preview changes from {pluginName}
          </Text>
          {preview.externalUrl && (
            <Text
              size="xs"
              c="blue"
              component="a"
              href={preview.externalUrl}
              target="_blank"
              rel="noopener noreferrer"
            >
              View on {pluginName} →
            </Text>
          )}
        </Stack>
        <SummaryBadges
          summary={preview.summary}
          selectedCount={selectedFields.size}
        />
      </Group>

      {/* Fields table */}
      <ScrollArea.Autosize mah="60vh">
        <Table highlightOnHover miw={700}>
          <Table.Thead>
            <Table.Tr>
              <Table.Th w={40}>Status</Table.Th>
              <Table.Th w={100} miw={100}>
                Field
              </Table.Th>
              <Table.Th miw={150}>Current</Table.Th>
              <Table.Th w={40} />
              <Table.Th miw={200}>New</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {preview.fields.map((field) => (
              <FieldRow
                key={field.field}
                field={field}
                isSelected={selectedFields.has(field.field)}
                onToggle={() => toggleField(field.field)}
                seriesId={seriesId}
                contentType={contentType}
              />
            ))}
          </Table.Tbody>
        </Table>
      </ScrollArea.Autosize>

      {/* Actions */}
      <Group justify="flex-end" gap="sm">
        {onBack && (
          <Button variant="light" onClick={onBack}>
            Back to Search
          </Button>
        )}
        <Button
          onClick={() => applyMutation.mutate()}
          loading={applyMutation.isPending}
          disabled={!canApply}
        >
          {canApply
            ? `Apply ${selectedFields.size} Field${selectedFields.size !== 1 ? "s" : ""}`
            : "No Changes to Apply"}
        </Button>
      </Group>
    </Stack>
  );
}

interface SummaryBadgesProps {
  summary: MetadataPreviewResponse["summary"];
  selectedCount: number;
}

function SummaryBadges({ summary, selectedCount }: SummaryBadgesProps) {
  return (
    <Group gap="xs">
      {selectedCount > 0 && (
        <Badge color="green" variant="light" size="sm">
          {selectedCount} to apply
        </Badge>
      )}
      {summary.locked > 0 && (
        <Badge color="yellow" variant="light" size="sm">
          {summary.locked} locked
        </Badge>
      )}
      {summary.noPermission > 0 && (
        <Badge color="red" variant="light" size="sm">
          {summary.noPermission} denied
        </Badge>
      )}
    </Group>
  );
}

interface FieldRowProps {
  field: MetadataFieldPreview;
  isSelected: boolean;
  onToggle: () => void;
  /** Entity ID for building cover thumbnail URL */
  seriesId: string;
  /** Content type to determine the correct thumbnail URL */
  contentType?: "series" | "book";
}

function FieldRow({
  field,
  isSelected,
  onToggle,
  seriesId,
  contentType = "series",
}: FieldRowProps) {
  const config = STATUS_CONFIG[field.status];
  const isApplyable = field.status === "will_apply";
  const isActive = isApplyable && isSelected;

  return (
    <Table.Tr
      style={{
        opacity: field.status === "not_provided" ? 0.5 : 1,
        cursor: isApplyable ? "pointer" : "default",
      }}
      onClick={isApplyable ? onToggle : undefined}
    >
      {/* Status icon / Checkbox */}
      <Table.Td>
        {isApplyable ? (
          <Checkbox
            checked={isSelected}
            onChange={onToggle}
            onClick={(e) => e.stopPropagation()}
            size="sm"
          />
        ) : (
          <Tooltip label={config.label}>
            <ThemeIcon
              size="sm"
              variant="light"
              color={config.color}
              radius="xl"
            >
              {config.icon}
            </ThemeIcon>
          </Tooltip>
        )}
      </Table.Td>

      {/* Field name */}
      <Table.Td>
        <Text size="sm" fw={isActive ? 500 : 400}>
          {FIELD_LABELS[field.field] || field.field}
        </Text>
      </Table.Td>

      {/* Current value */}
      <Table.Td>
        <ValueDisplay
          value={field.currentValue}
          fieldName={field.field}
          entityId={seriesId}
          contentType={contentType}
          isCurrent
        />
      </Table.Td>

      {/* Arrow */}
      <Table.Td>
        {isActive && <IconArrowRight size={14} color="gray" />}
      </Table.Td>

      {/* Proposed value */}
      <Table.Td>
        <ValueDisplay
          value={field.proposedValue}
          highlight={isActive}
          fieldName={field.field}
          contentType={contentType}
        />
      </Table.Td>
    </Table.Tr>
  );
}

interface ValueDisplayProps {
  value: unknown;
  highlight?: boolean;
  /** Field name to enable special rendering (e.g., coverUrl shows image) */
  fieldName?: string;
  /** Entity ID for building cover thumbnail URL (used for current cover) */
  entityId?: string;
  /** Content type to determine the correct thumbnail URL */
  contentType?: "series" | "book";
  /** Whether this is the current value (uses entity thumbnail) or proposed (uses external URL) */
  isCurrent?: boolean;
}

function ValueDisplay({
  value,
  highlight,
  fieldName,
  entityId,
  contentType = "series",
  isCurrent,
}: ValueDisplayProps) {
  // Handle cover URL - display as thumbnail image
  if (fieldName === "coverUrl") {
    // For current value, use the entity thumbnail from the server
    // For proposed value, use the external URL from the plugin
    const thumbnailPath = contentType === "book" ? "books" : "series";
    const coverSrc = isCurrent
      ? entityId
        ? `/api/v1/${thumbnailPath}/${entityId}/thumbnail`
        : undefined
      : typeof value === "string"
        ? value
        : undefined;
    const tooltipLabel = isCurrent
      ? "Current cover"
      : (value as string) || "No cover";

    return (
      <Tooltip label={tooltipLabel} multiline maw={300}>
        <Image
          src={coverSrc}
          alt="Cover"
          w={120}
          h={170}
          radius="xs"
          fallbackSrc="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='170'%3E%3Crect fill='%23333' width='120' height='170'/%3E%3Ctext fill='%23666' font-family='sans-serif' font-size='12' x='50%25' y='50%25' text-anchor='middle' dy='.3em'%3ENo Cover%3C/text%3E%3C/svg%3E"
          style={{
            border: highlight
              ? "2px solid var(--mantine-color-blue-6)"
              : "1px solid var(--mantine-color-dark-4)",
          }}
        />
      </Tooltip>
    );
  }

  if (value === null || value === undefined) {
    return (
      <Text size="sm" c="dimmed" fs="italic">
        —
      </Text>
    );
  }

  // Handle arrays (genres, tags, external links, ratings, etc.)
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return (
        <Text size="sm" c="dimmed" fs="italic">
          —
        </Text>
      );
    }
    // Check if it's an array of objects
    if (typeof value[0] === "object" && value[0] !== null) {
      const firstItem = value[0] as Record<string, unknown>;

      // Check if it's an array of ratings (has score property)
      if ("score" in firstItem) {
        const ratings = value as Array<{
          score: number;
          maxScore?: number;
          source?: string;
        }>;
        return (
          <Group gap={4} wrap="wrap">
            {ratings.slice(0, 3).map((rating, idx) => (
              <Badge
                key={`${rating.source || "rating"}-${idx}`}
                size="xs"
                variant={highlight ? "filled" : "light"}
              >
                {rating.source}: {Number(rating.score).toFixed(1)}/
                {rating.maxScore || 10}
              </Badge>
            ))}
            {ratings.length > 3 && (
              <Text size="xs" c="dimmed">
                +{ratings.length - 3}
              </Text>
            )}
          </Group>
        );
      }

      // Check if it's external IDs (has source and externalId properties)
      if ("source" in firstItem && "externalId" in firstItem) {
        const extIds = value as Array<{
          source: string;
          externalId: string;
        }>;
        return (
          <Group gap={4} wrap="wrap">
            {extIds.slice(0, 5).map((item, idx) => (
              <Badge
                key={`${item.source}-${idx}`}
                size="xs"
                variant={highlight ? "filled" : "light"}
                color="blue"
              >
                {item.source}: {item.externalId}
              </Badge>
            ))}
            {extIds.length > 5 && (
              <Text size="xs" c="dimmed">
                +{extIds.length - 5}
              </Text>
            )}
          </Group>
        );
      }

      // Check if it's alternate titles (has label and title properties, no url)
      if (
        "label" in firstItem &&
        "title" in firstItem &&
        !("url" in firstItem)
      ) {
        const altTitles = value as Array<{ label: string; title: string }>;
        return (
          <Group gap={4} wrap="wrap">
            {altTitles.slice(0, 3).map((item, idx) => (
              <Tooltip key={`${item.label}-${idx}`} label={item.label}>
                <Badge size="xs" variant={highlight ? "filled" : "light"}>
                  {item.title.length > 20
                    ? `${item.title.slice(0, 20)}...`
                    : item.title}
                </Badge>
              </Tooltip>
            ))}
            {altTitles.length > 3 && (
              <Text size="xs" c="dimmed">
                +{altTitles.length - 3}
              </Text>
            )}
          </Group>
        );
      }

      // Otherwise treat as external links (has label/url properties)
      const items = value as Array<{ label?: string; url?: string }>;
      return (
        <Group gap={4} wrap="wrap">
          {items.slice(0, 3).map((item, idx) => (
            <Badge
              key={`${item.label || item.url || "link"}-${idx}`}
              size="xs"
              variant={highlight ? "filled" : "light"}
            >
              {item.label || item.url || "Link"}
            </Badge>
          ))}
          {items.length > 3 && (
            <Text size="xs" c="dimmed">
              +{items.length - 3}
            </Text>
          )}
        </Group>
      );
    }
    // Handle simple arrays (strings)
    return (
      <Group gap={4} wrap="wrap">
        {value.slice(0, 5).map((item, idx) => (
          <Badge
            // biome-ignore lint/suspicious/noArrayIndexKey: items may have duplicate values
            key={`${item}-${idx}`}
            size="xs"
            variant={highlight ? "filled" : "light"}
          >
            {String(item)}
          </Badge>
        ))}
        {value.length > 5 && (
          <Text size="xs" c="dimmed">
            +{value.length - 5}
          </Text>
        )}
      </Group>
    );
  }

  // Handle rating objects
  if (typeof value === "object" && value !== null) {
    const obj = value as Record<string, unknown>;
    if ("score" in obj) {
      const score = Number(obj.score) || 0;
      const source = obj.source as string;
      return (
        <Text
          size="sm"
          fw={highlight ? 500 : 400}
          c={highlight ? undefined : "dimmed"}
        >
          {score.toFixed(1)}/100{source && ` (${source})`}
        </Text>
      );
    }
    // Generic object - show as JSON summary
    return (
      <Text size="sm" c="dimmed" fs="italic">
        {JSON.stringify(obj).slice(0, 50)}...
      </Text>
    );
  }

  // Handle strings/numbers
  const displayValue = String(value);
  const truncated =
    displayValue.length > 100
      ? `${displayValue.substring(0, 100)}...`
      : displayValue;

  return (
    <Tooltip
      label={displayValue}
      disabled={displayValue.length <= 100}
      multiline
      maw={300}
    >
      <Text
        size="sm"
        fw={highlight ? 500 : 400}
        c={highlight ? undefined : "dimmed"}
        lineClamp={2}
      >
        {truncated}
      </Text>
    </Tooltip>
  );
}

// Import React for useEffect
import React from "react";
