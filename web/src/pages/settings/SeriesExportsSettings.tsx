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
  SegmentedControl,
  Stack,
  Table,
  Text,
  Title,
  Tooltip,
} from "@mantine/core";
import { useForm } from "@mantine/form";
import {
  IconDownload,
  IconFileExport,
  IconPlus,
  IconTrash,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { librariesApi } from "@/api/libraries";
import type { ExportFieldDto } from "@/api/seriesExports";
import {
  useCreateSeriesExport,
  useDeleteSeriesExport,
  useDownloadSeriesExport,
  useExportFieldCatalog,
  useSeriesExportsList,
} from "@/hooks/useSeriesExports";

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

const FIELD_GROUPS: FieldGroup[] = [
  {
    label: "Identity",
    keys: ["library_name", "path", "created_at", "updated_at"],
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
    ],
  },
  {
    label: "Counts",
    keys: ["expected_book_count", "actual_book_count", "unread_book_count"],
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
  const { data: fieldCatalog, isLoading: fieldsLoading } =
    useExportFieldCatalog();
  const { data: libraries, isLoading: librariesLoading } = useQuery({
    queryKey: ["libraries"],
    queryFn: librariesApi.getAll,
  });
  const createMutation = useCreateSeriesExport();

  const form = useForm({
    initialValues: {
      format: "json",
      libraryIds: [] as string[],
      fields: [] as string[],
    },
    validate: {
      libraryIds: (v) =>
        v.length === 0 ? "Select at least one library" : null,
      fields: (v) => (v.length === 0 ? "Select at least one field" : null),
    },
  });

  const handleSubmit = () => {
    const validation = form.validate();
    if (validation.hasErrors) return;

    createMutation.mutate(form.values, {
      onSuccess: () => {
        form.reset();
        onClose();
      },
    });
  };

  const selectAllFields = () => {
    if (fieldCatalog) {
      // Exclude anchor fields (always included server-side)
      const allKeys = fieldCatalog
        .filter(
          (f) => !["series_id", "series_name", "library_id"].includes(f.key),
        )
        .map((f) => f.key);
      form.setFieldValue("fields", allKeys);
    }
  };

  const clearAllFields = () => {
    form.setFieldValue("fields", []);
  };

  // Build a lookup map for field catalog
  const fieldMap = new Map<string, ExportFieldDto>(
    (fieldCatalog || []).map((f) => [f.key, f]),
  );

  const libraryOptions = (libraries || []).map((lib) => ({
    value: lib.id,
    label: lib.name,
  }));

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title="Create Series Export"
      size="lg"
    >
      {fieldsLoading || librariesLoading ? (
        <Group justify="center" py="xl">
          <Loader />
        </Group>
      ) : (
        <Stack gap="md">
          <SegmentedControl
            data={[
              { label: "JSON", value: "json" },
              { label: "CSV", value: "csv" },
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

          <div>
            <Group justify="space-between" mb="xs">
              <Text fw={500} size="sm">
                Fields
              </Text>
              <Group gap="xs">
                <Button variant="subtle" size="xs" onClick={selectAllFields}>
                  Select all
                </Button>
                <Button variant="subtle" size="xs" onClick={clearAllFields}>
                  Clear
                </Button>
              </Group>
            </Group>

            <Text size="xs" c="dimmed" mb="sm">
              Series ID, Name, and Library ID are always included.
            </Text>

            {form.errors.fields && (
              <Text size="xs" c="red" mb="xs">
                {form.errors.fields}
              </Text>
            )}

            <Stack gap="sm">
              {FIELD_GROUPS.map((group) => (
                <Card key={group.label} withBorder padding="xs">
                  <Text size="xs" fw={600} c="dimmed" mb="xs">
                    {group.label}
                  </Text>
                  <Checkbox.Group
                    value={form.values.fields.filter((f) =>
                      group.keys.includes(f),
                    )}
                    onChange={(selected) => {
                      // Merge: keep fields from other groups, replace this group
                      const otherFields = form.values.fields.filter(
                        (f) => !group.keys.includes(f),
                      );
                      form.setFieldValue("fields", [
                        ...otherFields,
                        ...selected,
                      ]);
                    }}
                  >
                    <Group gap="sm">
                      {group.keys
                        .filter((k) => fieldMap.has(k))
                        .map((k) => {
                          const field = fieldMap.get(k)!;
                          return (
                            <Checkbox
                              key={k}
                              value={k}
                              label={field.label}
                              size="xs"
                            />
                          );
                        })}
                    </Group>
                  </Checkbox.Group>
                </Card>
              ))}
            </Stack>
          </div>

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
// Main settings page
// =============================================================================

export function SeriesExportsSettings() {
  const [modalOpened, setModalOpened] = useState(false);
  const { data: exports, isLoading } = useSeriesExportsList();
  const deleteMutation = useDeleteSeriesExport();
  const downloadMutation = useDownloadSeriesExport();

  const handleDownload = (exp: {
    id: string;
    format: string;
    createdAt: string;
  }) => {
    downloadMutation.mutate({
      id: exp.id,
      format: exp.format,
      createdAt: exp.createdAt,
    });
  };

  const handleDelete = (id: string) => {
    deleteMutation.mutate(id);
  };

  return (
    <Stack gap="lg">
      <Group justify="space-between">
        <div>
          <Title order={2}>Series Exports</Title>
          <Text c="dimmed" size="sm">
            Export your series data as JSON or CSV files.
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
        <Group justify="center" py="xl">
          <Loader />
        </Group>
      ) : !exports || exports.length === 0 ? (
        <Card withBorder padding="xl">
          <Stack align="center" gap="sm">
            <IconFileExport size={48} color="gray" opacity={0.5} />
            <Text c="dimmed">No exports yet</Text>
            <Text c="dimmed" size="sm">
              Create your first export to download series data.
            </Text>
          </Stack>
        </Card>
      ) : (
        <Card withBorder padding={0}>
          <Table striped highlightOnHover>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Created</Table.Th>
                <Table.Th>Format</Table.Th>
                <Table.Th>Status</Table.Th>
                <Table.Th>Rows</Table.Th>
                <Table.Th>Size</Table.Th>
                <Table.Th>Expires</Table.Th>
                <Table.Th style={{ width: 100 }}>Actions</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {exports.map((exp) => (
                <Table.Tr key={exp.id}>
                  <Table.Td>
                    <Text size="sm">
                      {new Date(exp.createdAt).toLocaleString()}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <Badge variant="outline" size="sm">
                      {exp.format.toUpperCase()}
                    </Badge>
                  </Table.Td>
                  <Table.Td>
                    <StatusBadge status={exp.status} />
                    {exp.error && (
                      <Tooltip label={exp.error}>
                        <Text size="xs" c="red" lineClamp={1}>
                          {exp.error}
                        </Text>
                      </Tooltip>
                    )}
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm">{exp.rowCount ?? "-"}</Text>
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm">
                      {formatBytes(exp.fileSizeBytes ?? null)}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <Text size="sm">
                      {new Date(exp.expiresAt).toLocaleDateString()}
                    </Text>
                  </Table.Td>
                  <Table.Td>
                    <Group gap="xs" wrap="nowrap">
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
                        >
                          <IconTrash size={16} />
                        </ActionIcon>
                      </Tooltip>
                    </Group>
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Card>
      )}

      <CreateExportModal
        opened={modalOpened}
        onClose={() => setModalOpened(false)}
      />
    </Stack>
  );
}
