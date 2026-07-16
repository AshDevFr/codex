import {
  ActionIcon,
  Badge,
  Button,
  Card,
  Checkbox,
  Group,
  Loader,
  Modal,
  MultiSelect,
  Radio,
  SegmentedControl,
  Stack,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import {
  IconDownload,
  IconFileExport,
  IconPlus,
  IconRobot,
  IconTrash,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { librariesApi } from "@/api/libraries";
import type { ExportFieldDto, SeriesExportDto } from "@/api/seriesExports";
import { TableSkeleton } from "@/components/skeletons";
import { ResponsiveTable, type ResponsiveTableColumn } from "@/components/ui";
import {
  useCreateSeriesExport,
  useDeleteSeriesExport,
  useDownloadSeriesExport,
  useExportFieldCatalog,
  useSeriesExportsList,
} from "@/hooks/useSeriesExports";
import { useShowSkeleton } from "@/lib/motion/useShowSkeleton";

// =============================================================================
// Status badge
// =============================================================================

function StatusBadge({ status }: { status: string }) {
  const colorMap: Record<string, string> = {
    pending: "yellow",
    running: "blue",
    completed: "green",
    failed: "red",
    cancelled: "gray",
  };
  return (
    <Badge color={colorMap[status] || "gray"} variant="light" size="sm">
      {status}
    </Badge>
  );
}

// =============================================================================
// File size formatter
// =============================================================================

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes === 0) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let size = bytes;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i++;
  }
  return `${size.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

// =============================================================================
// Field groups for the modal checkboxes
// =============================================================================

interface FieldGroup {
  label: string;
  keys: string[];
}

const SERIES_FIELD_GROUPS: FieldGroup[] = [
  {
    label: "Identity",
    keys: [
      "series_id",
      "library_id",
      "library_name",
      "path",
      "created_at",
      "updated_at",
    ],
  },
  {
    label: "Metadata",
    keys: [
      "title",
      "summary",
      "publisher",
      "status",
      "year",
      "language",
      "authors",
      "genres",
      "tags",
      "alternate_titles",
      "collections",
    ],
  },
  {
    label: "Counts & Progress",
    keys: [
      "expected_book_count",
      "expected_chapter_count",
      "actual_book_count",
      "unread_book_count",
      "progress",
    ],
  },
  {
    label: "Ratings",
    keys: [
      "user_rating",
      "user_notes",
      "community_avg_rating",
      "external_ratings",
    ],
  },
];

const BOOK_FIELD_GROUPS: FieldGroup[] = [
  {
    label: "Identity",
    keys: ["book_id", "series_id", "library_id", "series_name", "library_name"],
  },
  {
    label: "File Info",
    keys: [
      "file_name",
      "file_path",
      "file_size",
      "book_format",
      "page_count",
      "number",
      "created_at",
      "updated_at",
    ],
  },
  {
    label: "Metadata",
    keys: [
      "title",
      "summary",
      "publisher",
      "year",
      "language",
      "authors",
      "genres",
      "tags",
      "read_lists",
    ],
  },
  {
    label: "Progress",
    keys: ["progress", "current_page", "completed", "completed_at"],
  },
];

// =============================================================================
// Create export modal
// =============================================================================

function CreateExportModal({
  opened,
  onClose,
}: {
  opened: boolean;
  onClose: () => void;
}) {
  const { data: catalog, isLoading: fieldsLoading } = useExportFieldCatalog();
  const { data: libraries, isLoading: librariesLoading } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });
  const createMutation = useCreateSeriesExport();

  const form = useForm({
    initialValues: {
      format: "json",
      exportType: "series",
      libraryIds: [] as string[],
      fields: [] as string[],
      bookFields: [] as string[],
    },
    validate: {
      libraryIds: (v) =>
        v.length === 0 ? "Select at least one library" : null,
      fields: (v, values) => {
        if (values.exportType !== "books" && v.length === 0)
          return "Select at least one series field";
        return null;
      },
      bookFields: (v, values) => {
        if (values.exportType !== "series" && v.length === 0)
          return "Select at least one book field";
        return null;
      },
    },
  });

  const handleSubmit = () => {
    const validation = form.validate();
    if (validation.hasErrors) return;

    createMutation.mutate(
      {
        format: form.values.format,
        exportType: form.values.exportType,
        libraryIds: form.values.libraryIds,
        fields: form.values.fields,
        bookFields: form.values.bookFields,
      },
      {
        onSuccess: () => {
          form.reset();
          onClose();
        },
      },
    );
  };

  const seriesFields = catalog?.fields || [];
  const bookFieldCatalog = catalog?.bookFields || [];

  // Build lookup maps
  const seriesFieldMap = new Map<string, ExportFieldDto>(
    seriesFields.map((f) => [f.key, f]),
  );
  const bookFieldMap = new Map<string, ExportFieldDto>(
    bookFieldCatalog.map((f) => [f.key, f]),
  );

  const showSeriesFields = form.values.exportType !== "books";
  const showBookFields = form.values.exportType !== "series";

  // Select helpers
  const selectAllSeriesFields = () => {
    const allKeys = seriesFields.filter((f) => !f.isAnchor).map((f) => f.key);
    form.setFieldValue("fields", allKeys);
  };

  const selectAllBookFields = () => {
    const allKeys = bookFieldCatalog
      .filter((f) => !f.isAnchor)
      .map((f) => f.key);
    form.setFieldValue("bookFields", allKeys);
  };

  const llmSelectSeries = () => {
    if (catalog?.presets?.llmSelect) {
      form.setFieldValue("fields", catalog.presets.llmSelect);
    }
  };

  const llmSelectBooks = () => {
    if (catalog?.presets?.llmSelectBooks) {
      form.setFieldValue("bookFields", catalog.presets.llmSelectBooks);
    }
  };

  const clearSeriesFields = () => form.setFieldValue("fields", []);
  const clearBookFields = () => form.setFieldValue("bookFields", []);

  const libraryOptions = (libraries || []).map((lib) => ({
    value: lib.id,
    label: lib.name,
  }));

  // Auto-switch format when "both" selected with CSV
  const handleExportTypeChange = (value: string) => {
    form.setFieldValue("exportType", value);
    if (value === "both" && form.values.format === "csv") {
      form.setFieldValue("format", "json");
    }
  };

  const anchorLabel =
    form.values.exportType === "books"
      ? "Book Name is always included."
      : "Series Name is always included.";

  return (
    <Modal opened={opened} onClose={onClose} title="Create Export" size="lg">
      {fieldsLoading || librariesLoading ? (
        <Group justify="center" py="xl">
          <Loader />
        </Group>
      ) : (
        <Stack gap="md">
          <Radio.Group
            label="Export Type"
            value={form.values.exportType}
            onChange={handleExportTypeChange}
          >
            <Group>
              <Radio value="series" label="Series" />
              <Radio value="books" label="Books" />
              <Radio value="both" label="Both" />
            </Group>
          </Radio.Group>

          <SegmentedControl
            data={[
              { label: "JSON", value: "json" },
              {
                label: "CSV",
                value: "csv",
                disabled: form.values.exportType === "both",
              },
              { label: "Markdown", value: "md" },
            ]}
            {...form.getInputProps("format")}
          />

          <MultiSelect
            label="Libraries"
            placeholder="Select libraries to export"
            data={libraryOptions}
            searchable
            required
            error={form.errors.libraryIds}
            {...form.getInputProps("libraryIds")}
          />

          <Text size="xs" c="dimmed">
            {anchorLabel}
          </Text>

          {/* Series Fields */}
          {showSeriesFields && (
            <FieldSection
              title="Series Fields"
              fieldGroups={SERIES_FIELD_GROUPS}
              fieldMap={seriesFieldMap}
              selectedFields={form.values.fields}
              onFieldsChange={(fields) => form.setFieldValue("fields", fields)}
              onSelectAll={selectAllSeriesFields}
              onLlmSelect={llmSelectSeries}
              onClear={clearSeriesFields}
              error={form.errors.fields as string | undefined}
            />
          )}

          {/* Book Fields */}
          {showBookFields && (
            <FieldSection
              title="Book Fields"
              fieldGroups={BOOK_FIELD_GROUPS}
              fieldMap={bookFieldMap}
              selectedFields={form.values.bookFields}
              onFieldsChange={(fields) =>
                form.setFieldValue("bookFields", fields)
              }
              onSelectAll={selectAllBookFields}
              onLlmSelect={llmSelectBooks}
              onClear={clearBookFields}
              error={form.errors.bookFields as string | undefined}
            />
          )}

          <Group justify="flex-end" mt="md">
            <Button variant="subtle" onClick={onClose}>
              Cancel
            </Button>
            <Button
              leftSection={<IconFileExport size={16} />}
              loading={createMutation.isPending}
              onClick={handleSubmit}
            >
              Start Export
            </Button>
          </Group>
        </Stack>
      )}
    </Modal>
  );
}

// =============================================================================
// Reusable field section component
// =============================================================================

function FieldSection({
  title,
  fieldGroups,
  fieldMap,
  selectedFields,
  onFieldsChange,
  onSelectAll,
  onLlmSelect,
  onClear,
  error,
}: {
  title: string;
  fieldGroups: FieldGroup[];
  fieldMap: Map<string, ExportFieldDto>;
  selectedFields: string[];
  onFieldsChange: (fields: string[]) => void;
  onSelectAll: () => void;
  onLlmSelect: () => void;
  onClear: () => void;
  error?: string;
}) {
  // Catalog fields not covered by any hardcoded group still need a home:
  // "Select all" and the LLM preset operate on the full catalog, so a field
  // without a checkbox could be selected invisibly.
  const coveredKeys = new Set(fieldGroups.flatMap((g) => g.keys));
  const uncoveredKeys = [...fieldMap.values()]
    .filter((f) => !f.isAnchor && !coveredKeys.has(f.key))
    .map((f) => f.key);
  const groups = uncoveredKeys.length
    ? [...fieldGroups, { label: "Other", keys: uncoveredKeys }]
    : fieldGroups;

  return (
    <div>
      <Group justify="space-between" mb="xs">
        <Text fw={500} size="sm">
          {title}
        </Text>
        <Group gap="xs">
          <Tooltip label="Select fields useful for LLM context">
            <Button
              variant="subtle"
              size="xs"
              leftSection={<IconRobot size={14} />}
              onClick={onLlmSelect}
            >
              LLM Select
            </Button>
          </Tooltip>
          <Button variant="subtle" size="xs" onClick={onSelectAll}>
            Select all
          </Button>
          <Button variant="subtle" size="xs" onClick={onClear}>
            Clear
          </Button>
        </Group>
      </Group>

      {error && (
        <Text size="xs" c="red" mb="xs">
          {error}
        </Text>
      )}

      <Stack gap="sm">
        {groups.map((group) => (
          <Card key={group.label} withBorder padding="xs">
            <Text size="xs" fw={600} c="dimmed" mb="xs">
              {group.label}
            </Text>
            <Checkbox.Group
              value={selectedFields.filter((f) => group.keys.includes(f))}
              onChange={(selected) => {
                const otherFields = selectedFields.filter(
                  (f) => !group.keys.includes(f),
                );
                onFieldsChange([...otherFields, ...selected]);
              }}
            >
              <Group gap="sm">
                {group.keys.flatMap((k) => {
                  const field = fieldMap.get(k);
                  return field
                    ? [
                        <Checkbox
                          key={k}
                          value={k}
                          label={field.label}
                          size="xs"
                        />,
                      ]
                    : [];
                })}
              </Group>
            </Checkbox.Group>
          </Card>
        ))}
      </Stack>
    </div>
  );
}

// =============================================================================
// Export type label
// =============================================================================

function ExportTypeBadge({ exportType }: { exportType: string }) {
  const colorMap: Record<string, string> = {
    series: "blue",
    books: "violet",
    both: "teal",
  };
  return (
    <Badge color={colorMap[exportType] || "gray"} variant="outline" size="sm">
      {exportType}
    </Badge>
  );
}

// =============================================================================
// Libraries cell with field tooltip
// =============================================================================

function LibrariesCell({
  libraryNames,
  totalCount,
  seriesFields,
  bookFields,
  exportType,
}: {
  libraryNames: string[];
  totalCount: number;
  seriesFields: string[];
  bookFields: string[];
  exportType: string;
}) {
  const namesLabel =
    libraryNames.length === 0
      ? totalCount === 0
        ? "-"
        : `${totalCount} libraries`
      : libraryNames.join(", ");

  const showSeries = exportType !== "books" && seriesFields.length > 0;
  const showBooks = exportType !== "series" && bookFields.length > 0;

  return (
    <Tooltip
      multiline
      w={320}
      withArrow
      label={
        <Stack gap={4}>
          <Text size="xs" fw={600}>
            Libraries
          </Text>
          <Text size="xs">
            {libraryNames.length > 0 ? libraryNames.join(", ") : "-"}
          </Text>
          {showSeries && (
            <>
              <Text size="xs" fw={600} mt={4}>
                Series fields ({seriesFields.length})
              </Text>
              <Text size="xs">{seriesFields.join(", ")}</Text>
            </>
          )}
          {showBooks && (
            <>
              <Text size="xs" fw={600} mt={4}>
                Book fields ({bookFields.length})
              </Text>
              <Text size="xs">{bookFields.join(", ")}</Text>
            </>
          )}
        </Stack>
      }
    >
      <Text size="sm" lineClamp={1} style={{ maxWidth: 220, cursor: "help" }}>
        {namesLabel}
      </Text>
    </Tooltip>
  );
}

// =============================================================================
// Main settings page
// =============================================================================

export function SeriesExportsSettings() {
  const [modalOpened, setModalOpened] = useState(false);
  const { data: exports, isLoading } = useSeriesExportsList();
  const showSkeleton = useShowSkeleton(isLoading);
  const deleteMutation = useDeleteSeriesExport();
  const downloadMutation = useDownloadSeriesExport();

  const { data: libraries } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });

  const libraryNameById = new Map(
    (libraries || []).map((lib) => [lib.id, lib.name]),
  );

  const getLibraryNames = (ids: string[]): string[] =>
    ids.map((id) => libraryNameById.get(id)).filter((n): n is string => !!n);

  const handleDownload = (exp: {
    id: string;
    format: string;
    createdAt: string;
    libraryIds: string[];
  }) => {
    downloadMutation.mutate({
      id: exp.id,
      format: exp.format,
      createdAt: exp.createdAt,
      libraryNames: getLibraryNames(exp.libraryIds),
    });
  };

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id);
  };

  const exportColumns: ResponsiveTableColumn<SeriesExportDto>[] = [
    {
      key: "created",
      header: "Created",
      mobilePrimary: true,
      accessor: (exp) => (
        <Text size="sm" fw={500}>
          {new Date(exp.createdAt).toLocaleString()}
        </Text>
      ),
    },
    {
      key: "type",
      header: "Type",
      accessor: (exp) => <ExportTypeBadge exportType={exp.exportType} />,
    },
    {
      key: "format",
      header: "Format",
      accessor: (exp) => (
        <Badge variant="outline" size="sm">
          {exp.format.toUpperCase()}
        </Badge>
      ),
    },
    {
      key: "status",
      header: "Status",
      accessor: (exp) => (
        <>
          <StatusBadge status={exp.status} />
          {exp.error && (
            <Tooltip label={exp.error}>
              <Text size="xs" c="red" lineClamp={1}>
                {exp.error}
              </Text>
            </Tooltip>
          )}
        </>
      ),
    },
    {
      key: "libraries",
      header: "Libraries",
      mobileFullWidth: true,
      accessor: (exp) => (
        <LibrariesCell
          libraryNames={getLibraryNames(exp.libraryIds)}
          totalCount={exp.libraryIds.length}
          seriesFields={exp.fields}
          bookFields={exp.bookFields}
          exportType={exp.exportType}
        />
      ),
    },
    {
      key: "rows",
      header: "Rows",
      accessor: (exp) => <Text size="sm">{exp.rowCount ?? "-"}</Text>,
    },
    {
      key: "size",
      header: "Size",
      accessor: (exp) => (
        <Text size="sm">{formatBytes(exp.fileSizeBytes ?? null)}</Text>
      ),
    },
    {
      key: "expires",
      header: "Expires",
      accessor: (exp) => (
        <Text size="sm">{new Date(exp.expiresAt).toLocaleDateString()}</Text>
      ),
    },
  ];

  return (
    <Stack gap="lg">
      <Group justify="space-between">
        <div>
          <Title order={2}>Data Exports</Title>
          <Text c="dimmed" size="sm">
            Export your library data as JSON, CSV, or Markdown files.
          </Text>
        </div>
        <Button
          leftSection={<IconPlus size={16} />}
          onClick={() => setModalOpened(true)}
        >
          New Export
        </Button>
      </Group>

      {isLoading ? (
        showSkeleton ? (
          <TableSkeleton
            rows={4}
            columnLabels={["Export", "Format", "Status", "Created"]}
            withMobilePrimary
          />
        ) : null
      ) : !exports || exports.length === 0 ? (
        <Card withBorder padding="xl">
          <Stack align="center" gap="sm">
            <IconFileExport size={48} color="gray" opacity={0.5} />
            <Text c="dimmed">No exports yet</Text>
            <Text c="dimmed" size="sm">
              Create your first export to download library data.
            </Text>
          </Stack>
        </Card>
      ) : (
        <Card withBorder p={{ base: 0, xs: 0 }}>
          <ResponsiveTable
            data={exports}
            columns={exportColumns}
            getRowKey={(exp) => exp.id}
            tableProps={{ striped: true, highlightOnHover: true }}
            rowActions={(exp) => (
              <>
                {exp.status === "completed" && (
                  <Tooltip label="Download">
                    <ActionIcon
                      variant="subtle"
                      color="blue"
                      loading={
                        downloadMutation.isPending &&
                        downloadMutation.variables?.id === exp.id
                      }
                      onClick={() => handleDownload(exp)}
                      aria-label="Download export"
                    >
                      <IconDownload size={16} />
                    </ActionIcon>
                  </Tooltip>
                )}
                <Tooltip label="Delete">
                  <ActionIcon
                    variant="subtle"
                    color="red"
                    loading={
                      deleteMutation.isPending &&
                      deleteMutation.variables === exp.id
                    }
                    onClick={() => handleDelete(exp.id)}
                    aria-label="Delete export"
                  >
                    <IconTrash size={16} />
                  </ActionIcon>
                </Tooltip>
              </>
            )}
          />
        </Card>
      )}

      <CreateExportModal
        opened={modalOpened}
        onClose={() => setModalOpened(false)}
      />
    </Stack>
  );
}
