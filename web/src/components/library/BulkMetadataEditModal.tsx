import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Checkbox,
  Group,
  Modal,
  Select,
  SimpleGrid,
  Stack,
  Tabs,
  TagsInput,
  Text,
  TextInput,
  Tooltip,
} from "@mantine/core";
import { notifications } from "@mantine/notifications";
import {
  IconCode,
  IconEdit,
  IconLock,
  IconLockOpen,
  IconMinus,
  IconPlus,
  IconTag,
  IconTrash,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import { bulkMetadataApi } from "@/api/bulkMetadata";
import { genresApi } from "@/api/genres";
import { tagsApi } from "@/api/tags";
import { CustomMetadataEditor } from "@/components/forms/CustomMetadataEditor";
import {
  AUTHOR_ROLE_DISPLAY,
  BOOK_TYPE_DISPLAY,
  type BookAuthor,
  type BookAuthorRole,
} from "@/types/book-metadata";

// =============================================================================
// Types
// =============================================================================

export interface BulkMetadataEditModalProps {
  opened: boolean;
  onClose: () => void;
  selectedIds: string[];
  selectionType: "book" | "series";
  onSuccess?: () => void;
}

/** Sentinel for "multiple values exist across the selection" */
const MIXED = Symbol("mixed");
type MixedValue<T> = T | typeof MIXED | undefined;

// =============================================================================
// Constants
// =============================================================================

const STATUS_OPTIONS = [
  { value: "ongoing", label: "Ongoing" },
  { value: "ended", label: "Ended" },
  { value: "hiatus", label: "Hiatus" },
  { value: "abandoned", label: "Abandoned" },
  { value: "unknown", label: "Unknown" },
];

const READING_DIRECTION_OPTIONS = [
  { value: "ltr", label: "Left to Right (Comics)" },
  { value: "rtl", label: "Right to Left (Manga)" },
  { value: "ttb", label: "Top to Bottom" },
  { value: "webtoon", label: "Webtoon" },
];

const BOOK_TYPE_OPTIONS = Object.entries(BOOK_TYPE_DISPLAY).map(
  ([value, label]) => ({ value, label }),
);

const AUTHOR_ROLE_OPTIONS = Object.entries(AUTHOR_ROLE_DISPLAY).map(
  ([value, label]) => ({ value, label }),
);

// Series lock field definitions (field key, display label)
const SERIES_LOCK_FIELDS: Array<{ key: string; label: string }> = [
  { key: "title", label: "Title" },
  { key: "titleSort", label: "Title Sort" },
  { key: "summary", label: "Summary" },
  { key: "status", label: "Status" },
  { key: "language", label: "Language" },
  { key: "readingDirection", label: "Reading Direction" },
  { key: "publisher", label: "Publisher" },
  { key: "ageRating", label: "Age Rating" },
  { key: "imprint", label: "Imprint" },
  { key: "year", label: "Year" },
  { key: "totalBookCount", label: "Total Book Count" },
  { key: "genres", label: "Genres" },
  { key: "tags", label: "Tags" },
  { key: "customMetadata", label: "Custom Metadata" },
  { key: "authorsJsonLock", label: "Authors" },
  { key: "cover", label: "Cover" },
  { key: "alternateTitles", label: "Alternate Titles" },
];

// Book lock field definitions (field key, display label)
const BOOK_LOCK_FIELDS: Array<{ key: string; label: string }> = [
  { key: "titleLock", label: "Title" },
  { key: "titleSortLock", label: "Title Sort" },
  { key: "subtitleLock", label: "Subtitle" },
  { key: "summaryLock", label: "Summary" },
  { key: "numberLock", label: "Number" },
  { key: "publisherLock", label: "Publisher" },
  { key: "imprintLock", label: "Imprint" },
  { key: "genreLock", label: "Genre" },
  { key: "languageIsoLock", label: "Language" },
  { key: "bookTypeLock", label: "Book Type" },
  { key: "translatorLock", label: "Translator" },
  { key: "editionLock", label: "Edition" },
  { key: "originalTitleLock", label: "Original Title" },
  { key: "originalYearLock", label: "Original Year" },
  { key: "blackAndWhiteLock", label: "Black & White" },
  { key: "mangaLock", label: "Manga" },
  { key: "yearLock", label: "Year" },
  { key: "monthLock", label: "Month" },
  { key: "dayLock", label: "Day" },
  { key: "writerLock", label: "Writer" },
  { key: "pencillerLock", label: "Penciller" },
  { key: "inkerLock", label: "Inker" },
  { key: "coloristLock", label: "Colorist" },
  { key: "lettererLock", label: "Letterer" },
  { key: "coverArtistLock", label: "Cover Artist" },
  { key: "editorLock", label: "Editor" },
  { key: "authorsJsonLock", label: "Authors (JSON)" },
  { key: "awardsJsonLock", label: "Awards" },
  { key: "customMetadataLock", label: "Custom Metadata" },
  { key: "coverLock", label: "Cover" },
  { key: "isbnsLock", label: "ISBNs" },
  { key: "subjectsLock", label: "Subjects" },
  { key: "volumeLock", label: "Volume" },
  { key: "countLock", label: "Count" },
  { key: "formatDetailLock", label: "Format Detail" },
  { key: "seriesPositionLock", label: "Series Position" },
  { key: "seriesTotalLock", label: "Series Total" },
];

// =============================================================================
// Form state types
// =============================================================================

interface SeriesFormState {
  publisher: string;
  imprint: string;
  status: string | null;
  language: string;
  readingDirection: string | null;
  ageRating: string;
  year: string;
  totalBookCount: string;
  authors: BookAuthor[];
}

interface BookFormState {
  publisher: string;
  imprint: string;
  languageIso: string;
  bookType: string | null;
  translator: string;
  edition: string;
  originalTitle: string;
  originalYear: string;
  blackAndWhite: MixedValue<boolean>;
  manga: MixedValue<boolean>;
  authors: BookAuthor[];
}

/** Tracks which fields the user has explicitly touched */
type TouchedFields = Record<string, boolean>;

// =============================================================================
// Component
// =============================================================================

export function BulkMetadataEditModal({
  opened,
  onClose,
  selectedIds,
  selectionType,
  onSuccess,
}: BulkMetadataEditModalProps) {
  const queryClient = useQueryClient();
  const isSeries = selectionType === "series";
  const itemLabel = isSeries
    ? "series"
    : selectedIds.length === 1
      ? "book"
      : "books";
  const [activeTab, setActiveTab] = useState<string | null>("metadata");

  // Form state
  const [seriesForm, setSeriesForm] = useState<SeriesFormState>({
    publisher: "",
    imprint: "",
    status: null,
    language: "",
    readingDirection: null,
    ageRating: "",
    year: "",
    totalBookCount: "",
    authors: [],
  });

  const [bookForm, setBookForm] = useState<BookFormState>({
    publisher: "",
    imprint: "",
    languageIso: "",
    bookType: null,
    translator: "",
    edition: "",
    originalTitle: "",
    originalYear: "",
    blackAndWhite: undefined,
    manga: undefined,
    authors: [],
  });

  const [touchedFields, setTouchedFields] = useState<TouchedFields>({});

  // Tags/genres add/remove state
  const [tagsToAdd, setTagsToAdd] = useState<string[]>([]);
  const [tagsToRemove, setTagsToRemove] = useState<string[]>([]);
  const [genresToAdd, setGenresToAdd] = useState<string[]>([]);
  const [genresToRemove, setGenresToRemove] = useState<string[]>([]);

  // Lock state: null = don't change, true = lock, false = unlock
  const [lockChanges, setLockChanges] = useState<
    Record<string, boolean | null>
  >({});

  // Custom metadata state (separate from form since it uses its own tab)
  const [customMetadata, setCustomMetadata] = useState<Record<
    string,
    unknown
  > | null>(null);
  const [customMetadataLocked, setCustomMetadataLocked] = useState(false);
  const [customMetadataTouched, setCustomMetadataTouched] = useState(false);

  // Fetch all genres for autocomplete
  const { data: allGenres } = useQuery({
    queryKey: ["genres"],
    queryFn: () => genresApi.getAll(),
    enabled: opened,
  });

  // Fetch all tags for autocomplete
  const { data: allTags } = useQuery({
    queryKey: ["tags"],
    queryFn: () => tagsApi.getAll(),
    enabled: opened,
  });

  const genreNames = useMemo(
    () => allGenres?.map((g) => g.name) ?? [],
    [allGenres],
  );
  const tagNames = useMemo(() => allTags?.map((t) => t.name) ?? [], [allTags]);

  // Reset form when modal opens/closes
  useEffect(() => {
    if (opened) {
      setSeriesForm({
        publisher: "",
        imprint: "",
        status: null,
        language: "",
        readingDirection: null,
        ageRating: "",
        year: "",
        totalBookCount: "",
        authors: [],
      });
      setBookForm({
        publisher: "",
        imprint: "",
        languageIso: "",
        bookType: null,
        translator: "",
        edition: "",
        originalTitle: "",
        originalYear: "",
        blackAndWhite: undefined,
        manga: undefined,
        authors: [],
      });
      setTouchedFields({});
      setTagsToAdd([]);
      setTagsToRemove([]);
      setGenresToAdd([]);
      setGenresToRemove([]);
      setLockChanges({});
      setCustomMetadata(null);
      setCustomMetadataLocked(false);
      setCustomMetadataTouched(false);
      setActiveTab("metadata");
    }
  }, [opened]);

  // Mark a field as touched
  const touch = useCallback((field: string) => {
    setTouchedFields((prev) => ({ ...prev, [field]: true }));
  }, []);

  // ==========================================================================
  // Mutations
  // ==========================================================================

  const patchMetadataMutation = useMutation({
    mutationFn: async () => {
      if (isSeries) {
        const data: Record<string, unknown> = {
          seriesIds: selectedIds,
        };
        if (touchedFields.publisher)
          data.publisher = seriesForm.publisher || null;
        if (touchedFields.imprint) data.imprint = seriesForm.imprint || null;
        if (touchedFields.status) data.status = seriesForm.status || null;
        if (touchedFields.language) data.language = seriesForm.language || null;
        if (touchedFields.readingDirection)
          data.readingDirection = seriesForm.readingDirection || null;
        if (touchedFields.ageRating)
          data.ageRating = seriesForm.ageRating
            ? parseInt(seriesForm.ageRating, 10)
            : null;
        if (touchedFields.year)
          data.year = seriesForm.year ? parseInt(seriesForm.year, 10) : null;
        if (touchedFields.totalBookCount)
          data.totalBookCount = seriesForm.totalBookCount
            ? parseInt(seriesForm.totalBookCount, 10)
            : null;
        if (touchedFields.authors) {
          const filtered = seriesForm.authors.filter((a) => a.name.trim());
          data.authors = filtered.length > 0 ? filtered : null;
        }
        if (customMetadataTouched) {
          data.customMetadata = customMetadata;
        }
        return bulkMetadataApi.patchSeriesMetadata(
          data as Parameters<typeof bulkMetadataApi.patchSeriesMetadata>[0],
        );
      } else {
        const data: Record<string, unknown> = {
          bookIds: selectedIds,
        };
        if (touchedFields.publisher)
          data.publisher = bookForm.publisher || null;
        if (touchedFields.imprint) data.imprint = bookForm.imprint || null;
        if (touchedFields.languageIso)
          data.languageIso = bookForm.languageIso || null;
        if (touchedFields.bookType) data.bookType = bookForm.bookType || null;
        if (touchedFields.translator)
          data.translator = bookForm.translator || null;
        if (touchedFields.edition) data.edition = bookForm.edition || null;
        if (touchedFields.originalTitle)
          data.originalTitle = bookForm.originalTitle || null;
        if (touchedFields.originalYear)
          data.originalYear = bookForm.originalYear
            ? parseInt(bookForm.originalYear, 10)
            : null;
        if (touchedFields.blackAndWhite && bookForm.blackAndWhite !== MIXED)
          data.blackAndWhite = bookForm.blackAndWhite ?? null;
        if (touchedFields.manga && bookForm.manga !== MIXED)
          data.manga = bookForm.manga ?? null;
        if (touchedFields.authors) {
          const filtered = bookForm.authors.filter((a) => a.name.trim());
          data.authors = filtered.length > 0 ? filtered : null;
        }
        if (customMetadataTouched) {
          data.customMetadata = customMetadata;
        }
        return bulkMetadataApi.patchBookMetadata(
          data as Parameters<typeof bulkMetadataApi.patchBookMetadata>[0],
        );
      }
    },
  });

  const modifyTagsMutation = useMutation({
    mutationFn: async () => {
      if (isSeries) {
        return bulkMetadataApi.modifySeriesTags({
          seriesIds: selectedIds,
          add: tagsToAdd,
          remove: tagsToRemove,
        });
      } else {
        return bulkMetadataApi.modifyBookTags({
          bookIds: selectedIds,
          add: tagsToAdd,
          remove: tagsToRemove,
        });
      }
    },
  });

  const modifyGenresMutation = useMutation({
    mutationFn: async () => {
      if (isSeries) {
        return bulkMetadataApi.modifySeriesGenres({
          seriesIds: selectedIds,
          add: genresToAdd,
          remove: genresToRemove,
        });
      } else {
        return bulkMetadataApi.modifyBookGenres({
          bookIds: selectedIds,
          add: genresToAdd,
          remove: genresToRemove,
        });
      }
    },
  });

  const updateLocksMutation = useMutation({
    mutationFn: async () => {
      // Build the locks object with only changed locks
      const lockEntries = Object.entries(lockChanges).filter(
        ([, v]) => v !== null,
      );
      if (lockEntries.length === 0)
        return { updatedCount: 0, message: "No lock changes" };

      const locks = Object.fromEntries(lockEntries);
      if (isSeries) {
        return bulkMetadataApi.updateSeriesLocks({
          seriesIds: selectedIds,
          ...locks,
        } as Parameters<typeof bulkMetadataApi.updateSeriesLocks>[0]);
      } else {
        return bulkMetadataApi.updateBookLocks({
          bookIds: selectedIds,
          ...locks,
        } as Parameters<typeof bulkMetadataApi.updateBookLocks>[0]);
      }
    },
  });

  // ==========================================================================
  // Save handler
  // ==========================================================================

  const hasTouchedMetadata =
    Object.values(touchedFields).some(Boolean) || customMetadataTouched;
  const hasTagChanges = tagsToAdd.length > 0 || tagsToRemove.length > 0;
  const hasGenreChanges = genresToAdd.length > 0 || genresToRemove.length > 0;
  const hasLockChanges = Object.values(lockChanges).some((v) => v !== null);
  const hasCustomMetadataChanges = customMetadataTouched;
  const hasAnyChanges =
    hasTouchedMetadata ||
    hasTagChanges ||
    hasGenreChanges ||
    hasLockChanges ||
    hasCustomMetadataChanges;

  const isSaving =
    patchMetadataMutation.isPending ||
    modifyTagsMutation.isPending ||
    modifyGenresMutation.isPending ||
    updateLocksMutation.isPending;

  const handleSave = async () => {
    const results: string[] = [];
    const errors: string[] = [];

    try {
      // Execute all applicable mutations in parallel
      const promises: Promise<unknown>[] = [];

      if (hasTouchedMetadata) {
        promises.push(
          patchMetadataMutation
            .mutateAsync()
            .then((r) => results.push(r.message))
            .catch((e: Error) =>
              errors.push(e.message || "Failed to update metadata"),
            ),
        );
      }

      if (hasTagChanges) {
        promises.push(
          modifyTagsMutation
            .mutateAsync()
            .then((r) => results.push(r.message))
            .catch((e: Error) =>
              errors.push(e.message || "Failed to update tags"),
            ),
        );
      }

      if (hasGenreChanges) {
        promises.push(
          modifyGenresMutation
            .mutateAsync()
            .then((r) => results.push(r.message))
            .catch((e: Error) =>
              errors.push(e.message || "Failed to update genres"),
            ),
        );
      }

      if (hasLockChanges) {
        promises.push(
          updateLocksMutation
            .mutateAsync()
            .then((r) => {
              if (r.updatedCount > 0) results.push(r.message);
            })
            .catch((e: Error) =>
              errors.push(e.message || "Failed to update locks"),
            ),
        );
      }

      await Promise.all(promises);

      if (errors.length > 0) {
        notifications.show({
          title: "Partial update",
          message: `${results.length} operations succeeded, ${errors.length} failed: ${errors.join("; ")}`,
          color: "yellow",
        });
      } else if (results.length > 0) {
        notifications.show({
          title: "Metadata updated",
          message: results.join(". "),
          color: "green",
        });
      }

      // Invalidate relevant queries
      queryClient.refetchQueries({
        predicate: (query) => {
          const key = query.queryKey[0] as string;
          return (
            key === "books" ||
            key === "series" ||
            key === "series-books" ||
            key === "book-detail" ||
            key === "genres" ||
            key === "tags"
          );
        },
      });

      onSuccess?.();
      onClose();
    } catch {
      notifications.show({
        title: "Update failed",
        message: "An unexpected error occurred",
        color: "red",
      });
    }
  };

  // ==========================================================================
  // Lock helpers
  // ==========================================================================

  const toggleLock = (key: string) => {
    setLockChanges((prev) => {
      const current = prev[key];
      if (current === null || current === undefined) {
        // First click: set to lock
        return { ...prev, [key]: true };
      } else if (current === true) {
        // Second click: set to unlock
        return { ...prev, [key]: false };
      } else {
        // Third click: reset to unchanged
        const next = { ...prev };
        delete next[key];
        return next;
      }
    });
  };

  const lockFields = isSeries ? SERIES_LOCK_FIELDS : BOOK_LOCK_FIELDS;

  const setAllLocks = (value: boolean) => {
    const newChanges: Record<string, boolean> = {};
    for (const field of lockFields) {
      newChanges[field.key] = value;
    }
    setLockChanges(newChanges);
  };

  const clearAllLockChanges = () => {
    setLockChanges({});
  };

  // ==========================================================================
  // Render
  // ==========================================================================

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconEdit size={20} />
          <Text fw={600}>
            Bulk Edit Metadata ({selectedIds.length} {itemLabel})
          </Text>
        </Group>
      }
      size="lg"
      centered
      zIndex={1000}
      overlayProps={{ backgroundOpacity: 0.55, blur: 3 }}
      styles={{
        content: { width: 700 },
      }}
    >
      <Tabs value={activeTab} onChange={setActiveTab}>
        <Tabs.List>
          <Tabs.Tab value="metadata" leftSection={<IconEdit size={16} />}>
            Metadata
          </Tabs.Tab>
          <Tabs.Tab value="tags" leftSection={<IconTag size={16} />}>
            Tags & Genres
          </Tabs.Tab>
          <Tabs.Tab value="locks" leftSection={<IconLock size={16} />}>
            Locks
          </Tabs.Tab>
          <Tabs.Tab value="customMetadata" leftSection={<IconCode size={16} />}>
            Custom Metadata
          </Tabs.Tab>
        </Tabs.List>

        {/* ================================================================ */}
        {/* Metadata Tab                                                      */}
        {/* ================================================================ */}
        <Tabs.Panel value="metadata" pt="md">
          <Text size="sm" c="dimmed" mb="md">
            Only fields you modify will be applied. Empty fields left untouched
            will not clear existing values.
          </Text>

          {isSeries ? (
            <SeriesMetadataFields
              form={seriesForm}
              onChange={setSeriesForm}
              onTouch={touch}
              touchedFields={touchedFields}
            />
          ) : (
            <BookMetadataFields
              form={bookForm}
              onChange={setBookForm}
              onTouch={touch}
              touchedFields={touchedFields}
            />
          )}
        </Tabs.Panel>

        {/* ================================================================ */}
        {/* Tags & Genres Tab                                                 */}
        {/* ================================================================ */}
        <Tabs.Panel value="tags" pt="md">
          <Text size="sm" c="dimmed" mb="md">
            Add tags/genres to all selected items, or remove specific ones. This
            uses add/remove semantics (not replace).
          </Text>

          <Stack gap="lg">
            {/* Tags section */}
            <Box>
              <Text fw={500} mb="xs">
                Tags
              </Text>
              <SimpleGrid cols={2} spacing="md">
                <TagsInput
                  label="Add tags"
                  placeholder="Type to add tags..."
                  value={tagsToAdd}
                  onChange={setTagsToAdd}
                  data={tagNames}
                  clearable
                  comboboxProps={{ zIndex: 1100 }}
                />
                <TagsInput
                  label="Remove tags"
                  placeholder="Type tags to remove..."
                  value={tagsToRemove}
                  onChange={setTagsToRemove}
                  data={tagNames}
                  clearable
                  comboboxProps={{ zIndex: 1100 }}
                />
              </SimpleGrid>
            </Box>

            {/* Genres section */}
            <Box>
              <Text fw={500} mb="xs">
                Genres
              </Text>
              <SimpleGrid cols={2} spacing="md">
                <TagsInput
                  label="Add genres"
                  placeholder="Type to add genres..."
                  value={genresToAdd}
                  onChange={setGenresToAdd}
                  data={genreNames}
                  clearable
                  comboboxProps={{ zIndex: 1100 }}
                />
                <TagsInput
                  label="Remove genres"
                  placeholder="Type genres to remove..."
                  value={genresToRemove}
                  onChange={setGenresToRemove}
                  data={genreNames}
                  clearable
                  comboboxProps={{ zIndex: 1100 }}
                />
              </SimpleGrid>
            </Box>
          </Stack>
        </Tabs.Panel>

        {/* ================================================================ */}
        {/* Locks Tab                                                         */}
        {/* ================================================================ */}
        <Tabs.Panel value="locks" pt="md">
          <Group justify="space-between" mb="md">
            <Text size="sm" c="dimmed">
              Toggle locks for metadata fields across all selected items. Click
              once to lock, again to unlock, again to leave unchanged.
            </Text>
            <Group gap="xs">
              <Button
                variant="light"
                size="xs"
                leftSection={<IconLock size={14} />}
                onClick={() => setAllLocks(true)}
              >
                Lock All
              </Button>
              <Button
                variant="light"
                size="xs"
                leftSection={<IconLockOpen size={14} />}
                onClick={() => setAllLocks(false)}
              >
                Unlock All
              </Button>
              <Button
                variant="subtle"
                size="xs"
                leftSection={<IconMinus size={14} />}
                onClick={clearAllLockChanges}
              >
                Clear
              </Button>
            </Group>
          </Group>

          <SimpleGrid cols={3} spacing="xs">
            {lockFields.map((field) => {
              const state = lockChanges[field.key];
              return (
                <LockToggleRow
                  key={field.key}
                  label={field.label}
                  state={state === undefined ? null : state}
                  onChange={() => toggleLock(field.key)}
                />
              );
            })}
          </SimpleGrid>
        </Tabs.Panel>

        {/* ================================================================ */}
        {/* Custom Metadata Tab                                               */}
        {/* ================================================================ */}
        <Tabs.Panel value="customMetadata" pt="md">
          <Text size="sm" c="dimmed" mb="md">
            Edit custom JSON metadata. Uses merge patch semantics: provided keys
            are merged into existing metadata. Set a key to null to remove it.
          </Text>

          <CustomMetadataEditor
            value={customMetadata}
            onChange={(v) => {
              setCustomMetadata(v);
              setCustomMetadataTouched(true);
            }}
            locked={customMetadataLocked}
            onLockChange={setCustomMetadataLocked}
            autoLock={false}
          />
        </Tabs.Panel>
      </Tabs>

      {/* Footer */}
      <Group justify="flex-end" mt="xl">
        <Button variant="default" onClick={onClose} disabled={isSaving}>
          Cancel
        </Button>
        <Button
          onClick={handleSave}
          disabled={!hasAnyChanges || isSaving}
          loading={isSaving}
        >
          Apply to {selectedIds.length} {itemLabel}
        </Button>
      </Group>
    </Modal>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

/** Series metadata fields */
function SeriesMetadataFields({
  form,
  onChange,
  onTouch,
  touchedFields,
}: {
  form: SeriesFormState;
  onChange: (form: SeriesFormState) => void;
  onTouch: (field: string) => void;
  touchedFields: TouchedFields;
}) {
  const update = <K extends keyof SeriesFormState>(
    key: K,
    value: SeriesFormState[K],
  ) => {
    onChange({ ...form, [key]: value });
    onTouch(key);
  };

  return (
    <Stack gap="sm">
      <SimpleGrid cols={2} spacing="md">
        <BulkField label="Publisher" touched={touchedFields.publisher}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.publisher}
            onChange={(e) => update("publisher", e.target.value)}
          />
        </BulkField>

        <BulkField label="Imprint" touched={touchedFields.imprint}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.imprint}
            onChange={(e) => update("imprint", e.target.value)}
          />
        </BulkField>

        <BulkField label="Status" touched={touchedFields.status}>
          <Select
            placeholder="Leave unchanged"
            data={STATUS_OPTIONS}
            value={form.status}
            onChange={(v) => update("status", v)}
            clearable
            comboboxProps={{ zIndex: 1100 }}
          />
        </BulkField>

        <BulkField
          label="Reading Direction"
          touched={touchedFields.readingDirection}
        >
          <Select
            placeholder="Leave unchanged"
            data={READING_DIRECTION_OPTIONS}
            value={form.readingDirection}
            onChange={(v) => update("readingDirection", v)}
            clearable
            comboboxProps={{ zIndex: 1100 }}
          />
        </BulkField>

        <BulkField label="Language" touched={touchedFields.language}>
          <TextInput
            placeholder="e.g., en, ja, ko"
            value={form.language}
            onChange={(e) => update("language", e.target.value)}
          />
        </BulkField>

        <BulkField label="Age Rating" touched={touchedFields.ageRating}>
          <TextInput
            placeholder="e.g., 13, 16, 18"
            value={form.ageRating}
            onChange={(e) => update("ageRating", e.target.value)}
          />
        </BulkField>

        <BulkField label="Year" touched={touchedFields.year}>
          <TextInput
            placeholder="e.g., 2024"
            value={form.year}
            onChange={(e) => update("year", e.target.value)}
          />
        </BulkField>

        <BulkField
          label="Total Book Count"
          touched={touchedFields.totalBookCount}
        >
          <TextInput
            placeholder="Expected total"
            value={form.totalBookCount}
            onChange={(e) => update("totalBookCount", e.target.value)}
          />
        </BulkField>
      </SimpleGrid>

      <AuthorsEditor
        authors={form.authors}
        onChange={(authors) => update("authors", authors)}
        touched={touchedFields.authors}
      />
    </Stack>
  );
}

/** Book metadata fields */
function BookMetadataFields({
  form,
  onChange,
  onTouch,
  touchedFields,
}: {
  form: BookFormState;
  onChange: (form: BookFormState) => void;
  onTouch: (field: string) => void;
  touchedFields: TouchedFields;
}) {
  const update = <K extends keyof BookFormState>(
    key: K,
    value: BookFormState[K],
  ) => {
    onChange({ ...form, [key]: value });
    onTouch(key);
  };

  return (
    <Stack gap="sm">
      <SimpleGrid cols={2} spacing="md">
        <BulkField label="Publisher" touched={touchedFields.publisher}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.publisher}
            onChange={(e) => update("publisher", e.target.value)}
          />
        </BulkField>

        <BulkField label="Imprint" touched={touchedFields.imprint}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.imprint}
            onChange={(e) => update("imprint", e.target.value)}
          />
        </BulkField>

        <BulkField label="Language" touched={touchedFields.languageIso}>
          <TextInput
            placeholder="e.g., en, ja, ko"
            value={form.languageIso}
            onChange={(e) => update("languageIso", e.target.value)}
          />
        </BulkField>

        <BulkField label="Book Type" touched={touchedFields.bookType}>
          <Select
            placeholder="Leave unchanged"
            data={BOOK_TYPE_OPTIONS}
            value={form.bookType}
            onChange={(v) => update("bookType", v)}
            clearable
            comboboxProps={{ zIndex: 1100 }}
          />
        </BulkField>

        <BulkField label="Translator" touched={touchedFields.translator}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.translator}
            onChange={(e) => update("translator", e.target.value)}
          />
        </BulkField>

        <BulkField label="Edition" touched={touchedFields.edition}>
          <TextInput
            placeholder='e.g., "Deluxe", "Omnibus"'
            value={form.edition}
            onChange={(e) => update("edition", e.target.value)}
          />
        </BulkField>

        <BulkField label="Original Title" touched={touchedFields.originalTitle}>
          <TextInput
            placeholder="Leave empty to skip"
            value={form.originalTitle}
            onChange={(e) => update("originalTitle", e.target.value)}
          />
        </BulkField>

        <BulkField label="Original Year" touched={touchedFields.originalYear}>
          <TextInput
            placeholder="e.g., 2020"
            value={form.originalYear}
            onChange={(e) => update("originalYear", e.target.value)}
          />
        </BulkField>
      </SimpleGrid>

      <SimpleGrid cols={2} spacing="md" mt="sm">
        <Checkbox
          label="Black & White"
          checked={form.blackAndWhite === true}
          indeterminate={form.blackAndWhite === MIXED}
          onChange={(e) => {
            update("blackAndWhite", e.target.checked);
          }}
        />
        <Checkbox
          label="Manga"
          checked={form.manga === true}
          indeterminate={form.manga === MIXED}
          onChange={(e) => {
            update("manga", e.target.checked);
          }}
        />
      </SimpleGrid>

      <AuthorsEditor
        authors={form.authors}
        onChange={(authors) => update("authors", authors)}
        touched={touchedFields.authors}
      />
    </Stack>
  );
}

/** A field wrapper that shows a "modified" indicator */
function BulkField({
  label,
  touched,
  children,
}: {
  label: string;
  touched?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Box>
      <Group gap={4} mb={2}>
        <Text size="sm" fw={500}>
          {label}
        </Text>
        {touched && (
          <Badge size="xs" variant="light" color="blue">
            modified
          </Badge>
        )}
      </Group>
      {children}
    </Box>
  );
}

/** Inline author editor for bulk operations (replaces all authors on selected items) */
function AuthorsEditor({
  authors,
  onChange,
  touched,
}: {
  authors: BookAuthor[];
  onChange: (authors: BookAuthor[]) => void;
  touched?: boolean;
}) {
  const updateAuthor = (index: number, updates: Partial<BookAuthor>) => {
    const newAuthors = [...authors];
    newAuthors[index] = { ...newAuthors[index], ...updates };
    onChange(newAuthors);
  };

  const removeAuthor = (index: number) => {
    onChange(authors.filter((_, i) => i !== index));
  };

  const addAuthor = () => {
    onChange([...authors, { name: "", role: "author" as BookAuthorRole }]);
  };

  return (
    <Box mt="sm">
      <Group gap={4} mb="xs">
        <Text size="sm" fw={500}>
          Authors
        </Text>
        {touched && (
          <Badge size="xs" variant="light" color="blue">
            modified
          </Badge>
        )}
      </Group>
      <Text size="xs" c="dimmed" mb="xs">
        This will replace all authors on the selected items.
      </Text>

      <Stack gap="xs">
        {authors.map((author, index) => (
          <Group
            key={`${author.role}-${index}`}
            gap="xs"
            wrap="nowrap"
            align="flex-end"
          >
            <Select
              label={index === 0 ? "Role" : undefined}
              data={AUTHOR_ROLE_OPTIONS}
              value={author.role}
              onChange={(value) => {
                if (value) {
                  updateAuthor(index, { role: value as BookAuthorRole });
                }
              }}
              style={{ flex: 1 }}
              comboboxProps={{ zIndex: 1100 }}
            />
            <TextInput
              label={index === 0 ? "Name" : undefined}
              placeholder="Author name"
              value={author.name}
              onChange={(e) => {
                updateAuthor(index, { name: e.target.value });
              }}
              style={{ flex: 2 }}
            />
            <ActionIcon
              variant="subtle"
              color="red"
              onClick={() => removeAuthor(index)}
              aria-label="Remove author"
            >
              <IconTrash size={18} />
            </ActionIcon>
          </Group>
        ))}
      </Stack>

      <Box mt="xs">
        <Button
          variant="subtle"
          leftSection={<IconPlus size={16} />}
          onClick={addAuthor}
          size="sm"
        >
          Add Author
        </Button>
      </Box>
    </Box>
  );
}

/** A tri-state lock toggle row: null (unchanged), true (lock), false (unlock) */
function LockToggleRow({
  label,
  state,
  onChange,
}: {
  label: string;
  state: boolean | null;
  onChange: () => void;
}) {
  let icon: React.ReactNode;
  let color: string;
  let stateLabel: string;

  if (state === null) {
    icon = <IconMinus size={16} />;
    color = "gray";
    stateLabel = "Unchanged";
  } else if (state === true) {
    icon = <IconLock size={16} />;
    color = "orange";
    stateLabel = "Will lock";
  } else {
    icon = <IconLockOpen size={16} />;
    color = "blue";
    stateLabel = "Will unlock";
  }

  return (
    <Group
      gap="xs"
      py={4}
      px="xs"
      style={{
        borderRadius: "var(--mantine-radius-sm)",
        cursor: "pointer",
        backgroundColor:
          state !== null ? `var(--mantine-color-${color}-light)` : undefined,
      }}
      onClick={onChange}
    >
      <Tooltip label={stateLabel} zIndex={1100}>
        <ActionIcon variant="subtle" color={color} size="sm">
          {icon}
        </ActionIcon>
      </Tooltip>
      <Text size="sm">{label}</Text>
      {state !== null && (
        <Badge size="xs" variant="light" color={color} ml="auto">
          {stateLabel}
        </Badge>
      )}
    </Group>
  );
}
