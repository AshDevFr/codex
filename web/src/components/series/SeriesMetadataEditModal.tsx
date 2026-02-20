import {
  ActionIcon,
  Box,
  Button,
  Center,
  Group,
  Loader,
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
  IconBook,
  IconCode,
  IconEdit,
  IconLink,
  IconList,
  IconLock,
  IconLockOpen,
  IconPhoto,
  IconPlus,
  IconShare,
  IconTag,
  IconTrash,
  IconTypography,
  IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import { genresApi } from "@/api/genres";
import { seriesApi } from "@/api/series";
import {
  type FullSeriesMetadata,
  type MetadataLocks,
  seriesMetadataApi,
} from "@/api/seriesMetadata";
import { sharingTagsApi } from "@/api/sharingTags";
import { tagsApi } from "@/api/tags";
import { CoverEditor } from "@/components/forms/CoverEditor";
import { CustomMetadataEditor } from "@/components/forms/CustomMetadataEditor";
import {
  type ListItem,
  LockableChipInput,
  LockableInput,
  LockableListEditor,
  LockableSelect,
  LockableTextarea,
} from "@/components/forms/lockable";
import { extractSourceFromUrl } from "@/components/series/ExternalLinks";
import { usePermissions } from "@/hooks/usePermissions";
import type { BookAuthor, BookAuthorRole } from "@/types/book-metadata";
import { AUTHOR_ROLE_DISPLAY } from "@/types/book-metadata";

export interface SeriesMetadataEditModalProps {
  opened: boolean;
  onClose: () => void;
  seriesId: string;
  seriesTitle?: string;
}

interface FormState {
  title: string;
  titleSort: string;
  summary: string;
  status: string | null;
  language: string;
  readingDirection: string | null;
  publisher: string;
  ageRating: string;
  imprint: string;
  year: string;
  totalBookCount: string;
  genres: string[];
  tags: string[];
  sharingTags: string[];
  authors: BookAuthor[];
  alternateTitles: ListItem[];
  externalLinks: ListItem[];
  customMetadata: Record<string, unknown> | null;
}

interface LocksState {
  title: boolean;
  titleSort: boolean;
  summary: boolean;
  status: boolean;
  language: boolean;
  readingDirection: boolean;
  publisher: boolean;
  ageRating: boolean;
  imprint: boolean;
  year: boolean;
  totalBookCount: boolean;
  genres: boolean;
  tags: boolean;
  customMetadata: boolean;
  authorsJson: boolean;
  cover: boolean;
  alternateTitles: boolean;
}

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

// Reserved for future use in alternate title type selection
export const ALTERNATE_TITLE_LABELS = [
  { value: "native", label: "Native" },
  { value: "roman", label: "Roman" },
  { value: "english", label: "English" },
  { value: "japanese", label: "Japanese" },
  { value: "korean", label: "Korean" },
  { value: "chinese", label: "Chinese" },
  { value: "other", label: "Other" },
];

function initializeFormState(
  metadata: FullSeriesMetadata | undefined,
): FormState {
  return {
    title: metadata?.title || "",
    titleSort: metadata?.titleSort || "",
    summary: metadata?.summary || "",
    status: metadata?.status || null,
    language: metadata?.language || "",
    readingDirection: metadata?.readingDirection || null,
    publisher: metadata?.publisher || "",
    ageRating: metadata?.ageRating?.toString() || "",
    imprint: metadata?.imprint || "",
    year: metadata?.year?.toString() || "",
    totalBookCount: metadata?.totalBookCount?.toString() || "",
    genres: metadata?.genres.map((g) => g.name) || [],
    tags: metadata?.tags?.map((t) => t.name) || [],
    authors: (metadata?.authors as BookAuthor[] | undefined) ?? [],
    sharingTags: [], // Populated separately from seriesSharingTags query
    alternateTitles:
      metadata?.alternateTitles.map((t) => ({
        id: t.id,
        values: { label: t.label || "other", title: t.title },
        locked: false,
      })) || [],
    externalLinks:
      metadata?.externalLinks.map((l) => ({
        id: l.id,
        values: { label: l.sourceName, url: l.url },
        locked: false,
      })) || [],
    customMetadata:
      (metadata?.customMetadata as Record<string, unknown>) ?? null,
  };
}

function initializeLocksState(locks: MetadataLocks | undefined): LocksState {
  return {
    title: locks?.title || false,
    titleSort: locks?.titleSort || false,
    summary: locks?.summary || false,
    status: locks?.status || false,
    language: locks?.language || false,
    readingDirection: locks?.readingDirection || false,
    publisher: locks?.publisher || false,
    ageRating: locks?.ageRating || false,
    imprint: locks?.imprint || false,
    year: locks?.year || false,
    totalBookCount: locks?.totalBookCount || false,
    genres: locks?.genres || false,
    tags: locks?.tags || false,
    customMetadata: locks?.customMetadata || false,
    authorsJson: locks?.authorsJsonLock || false,
    cover: (locks as LocksState | undefined)?.cover || false,
    alternateTitles: locks?.alternateTitles || false,
  };
}

export function SeriesMetadataEditModal({
  opened,
  onClose,
  seriesId,
  seriesTitle,
}: SeriesMetadataEditModalProps) {
  const queryClient = useQueryClient();
  const { isAdmin } = usePermissions();
  const [activeTab, setActiveTab] = useState<string | null>("general");
  const [formState, setFormState] = useState<FormState>(
    initializeFormState(undefined),
  );
  const [locksState, setLocksState] = useState<LocksState>(
    initializeLocksState(undefined),
  );
  const [originalFormState, setOriginalFormState] = useState<FormState | null>(
    null,
  );

  // Fetch full metadata
  const { data: metadata, isLoading } = useQuery({
    queryKey: ["series", seriesId, "metadata", "full"],
    queryFn: () => seriesMetadataApi.getFullMetadata(seriesId),
    enabled: opened,
  });

  // Fetch all genres for suggestions
  const { data: allGenres } = useQuery({
    queryKey: ["genres"],
    queryFn: () => genresApi.getAll(),
    enabled: opened,
  });

  // Fetch all tags for suggestions
  const { data: allTags } = useQuery({
    queryKey: ["tags"],
    queryFn: () => tagsApi.getAll(),
    enabled: opened,
  });

  // Fetch existing covers for this series
  const { data: existingCovers, refetch: refetchCovers } = useQuery({
    queryKey: ["series", seriesId, "covers"],
    queryFn: () => seriesMetadataApi.listCovers(seriesId),
    enabled: opened,
  });

  // Fetch all sharing tags (admin only)
  const { data: allSharingTags } = useQuery({
    queryKey: ["sharing-tags"],
    queryFn: sharingTagsApi.list,
    enabled: opened && isAdmin,
  });

  // Fetch current series sharing tags (admin only)
  const { data: seriesSharingTags } = useQuery({
    queryKey: ["series-sharing-tags", seriesId],
    queryFn: () => sharingTagsApi.getForSeries(seriesId),
    enabled: opened && isAdmin,
  });

  // Cover mutations
  const uploadCoverMutation = useMutation({
    mutationFn: (file: File) => seriesMetadataApi.uploadCover(seriesId, file),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Cover uploaded successfully",
        color: "green",
      });
      refetchCovers();
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to upload cover",
        color: "red",
      });
    },
  });

  const selectCoverMutation = useMutation({
    mutationFn: (coverId: string) =>
      seriesMetadataApi.selectCover(seriesId, coverId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Cover selected",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to select cover",
        color: "red",
      });
    },
  });

  const resetCoverMutation = useMutation({
    mutationFn: () => seriesMetadataApi.resetCover(seriesId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Reset to default cover",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to reset cover",
        color: "red",
      });
    },
  });

  const deleteCoverMutation = useMutation({
    mutationFn: (coverId: string) =>
      seriesMetadataApi.deleteCover(seriesId, coverId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Cover deleted",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete cover",
        color: "red",
      });
    },
  });

  // Initialize form state when metadata and sharing tags load
  useEffect(() => {
    if (metadata) {
      const newFormState = initializeFormState(metadata);
      // Include sharing tags if available
      const sharingTagNames = seriesSharingTags?.map((t) => t.name) || [];
      newFormState.sharingTags = sharingTagNames;
      setFormState(newFormState);
      setOriginalFormState({ ...newFormState });
      setLocksState(initializeLocksState(metadata.locks));
    }
  }, [metadata, seriesSharingTags]);

  // Update field helper
  const updateField = useCallback(
    <K extends keyof FormState>(field: K, value: FormState[K]) => {
      setFormState((prev) => ({ ...prev, [field]: value }));
    },
    [],
  );

  // Update lock helper
  const updateLock = useCallback(
    <K extends keyof LocksState>(field: K, value: boolean) => {
      setLocksState((prev) => ({ ...prev, [field]: value }));
    },
    [],
  );

  // Save mutation
  const saveMutation = useMutation({
    mutationFn: async () => {
      // Update series name if changed
      const titleChanged = formState.title !== originalFormState?.title;
      if (titleChanged && formState.title) {
        await seriesApi.patch(seriesId, { title: formState.title });
      }

      // Update metadata
      await seriesMetadataApi.patchMetadata(seriesId, {
        titleSort: formState.titleSort || null,
        summary: formState.summary || null,
        status: formState.status || null,
        readingDirection: formState.readingDirection || null,
        publisher: formState.publisher || null,
        imprint: formState.imprint || null,
        language: formState.language || null,
        ageRating: formState.ageRating
          ? Number.parseInt(formState.ageRating, 10)
          : null,
        year: formState.year ? Number.parseInt(formState.year, 10) : null,
        totalBookCount: formState.totalBookCount
          ? Number.parseInt(formState.totalBookCount, 10)
          : null,
        authors:
          formState.authors.length > 0
            ? formState.authors.filter((a) => a.name.trim())
            : null,
        // Cast needed due to OpenAPI type generation quirk (Record<string, never> vs Record<string, unknown>)
        customMetadata: formState.customMetadata as Record<
          string,
          never
        > | null,
      });

      // Update locks
      await seriesMetadataApi.updateLocks(seriesId, {
        ...locksState,
        authorsJsonLock: locksState.authorsJson,
      });

      // Update genres if changed
      const genresChanged =
        JSON.stringify(formState.genres.slice().sort()) !==
        JSON.stringify((originalFormState?.genres || []).slice().sort());
      if (genresChanged) {
        await genresApi.setForSeries(seriesId, formState.genres);
      }

      // Update tags if changed
      const tagsChanged =
        JSON.stringify(formState.tags.slice().sort()) !==
        JSON.stringify((originalFormState?.tags || []).slice().sort());
      if (tagsChanged) {
        await tagsApi.setForSeries(seriesId, formState.tags);
      }

      // Update sharing tags if changed (admin only)
      const sharingTagsChanged =
        JSON.stringify(formState.sharingTags.slice().sort()) !==
        JSON.stringify((originalFormState?.sharingTags || []).slice().sort());
      if (sharingTagsChanged && isAdmin) {
        // Find which tags need to be created (names that don't exist yet)
        const existingNames = new Set(
          allSharingTags?.map((t) => t.name.toLowerCase()) || [],
        );
        const tagsToCreate = formState.sharingTags.filter(
          (name) => !existingNames.has(name.toLowerCase()),
        );

        // Create new tags first
        for (const name of tagsToCreate) {
          await sharingTagsApi.create({ name });
        }

        // Refetch to get updated tag list with new IDs
        const updatedTags = await sharingTagsApi.list();
        const tagNameToId = new Map(
          updatedTags.map((t) => [t.name.toLowerCase(), t.id]),
        );

        // Map names to IDs
        const tagIds = formState.sharingTags
          .map((name) => tagNameToId.get(name.toLowerCase()))
          .filter((id): id is string => id !== undefined);

        await sharingTagsApi.setForSeries(seriesId, tagIds);
      }

      // Handle alternate titles changes
      const originalTitleIds = new Set(
        originalFormState?.alternateTitles.map((t) => t.id) || [],
      );
      const currentTitleIds = new Set(
        formState.alternateTitles
          .filter((t) => !t.id.startsWith("new-"))
          .map((t) => t.id),
      );

      // Delete removed titles
      for (const title of originalFormState?.alternateTitles || []) {
        if (!currentTitleIds.has(title.id)) {
          await seriesMetadataApi.deleteAlternateTitle(seriesId, title.id);
        }
      }

      // Create new titles and update existing ones
      for (const title of formState.alternateTitles) {
        if (title.id.startsWith("new-")) {
          // Create new title
          await seriesMetadataApi.createAlternateTitle(
            seriesId,
            title.values.title,
            title.values.label,
          );
        } else if (originalTitleIds.has(title.id)) {
          // Update existing title
          const original = originalFormState?.alternateTitles.find(
            (t) => t.id === title.id,
          );
          if (
            original &&
            (original.values.title !== title.values.title ||
              original.values.label !== title.values.label)
          ) {
            await seriesMetadataApi.updateAlternateTitle(
              seriesId,
              title.id,
              title.values.title,
              title.values.label,
            );
          }
        }
      }

      // Handle external links changes
      const originalLinkIds = new Set(
        originalFormState?.externalLinks.map((l) => l.id) || [],
      );
      void originalLinkIds; // Reserved for future link update logic
      const currentLinkIds = new Set(
        formState.externalLinks
          .filter((l) => !l.id.startsWith("new-"))
          .map((l) => l.id),
      );

      // Delete removed links
      for (const link of originalFormState?.externalLinks || []) {
        if (!currentLinkIds.has(link.id)) {
          await seriesMetadataApi.deleteExternalLink(seriesId, link.id);
        }
      }

      // Create new links
      for (const link of formState.externalLinks) {
        if (link.id.startsWith("new-")) {
          await seriesMetadataApi.createExternalLink(
            seriesId,
            link.values.label,
            link.values.url,
          );
        }
      }
    },
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Series metadata updated successfully",
        color: "green",
      });
      queryClient.invalidateQueries({ queryKey: ["series", seriesId] });
      queryClient.invalidateQueries({
        queryKey: ["series-metadata", seriesId],
      });
      queryClient.invalidateQueries({ queryKey: ["sharing-tags"] });
      queryClient.invalidateQueries({
        queryKey: ["series-sharing-tags", seriesId],
      });
      onClose();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update series metadata",
        color: "red",
      });
    },
  });

  const handleSave = () => {
    saveMutation.mutate();
  };

  // General tab - Title, Sort Title, Summary
  const renderGeneralTab = () => (
    <Stack gap="md">
      <LockableInput
        label="Title"
        value={formState.title}
        onChange={(v) => updateField("title", v)}
        locked={locksState.title}
        onLockChange={(v) => updateLock("title", v)}
        originalValue={originalFormState?.title}
        placeholder="Series title"
        description="Display name for this series"
      />

      <LockableInput
        label="Sort Title"
        value={formState.titleSort}
        onChange={(v) => updateField("titleSort", v)}
        locked={locksState.titleSort}
        onLockChange={(v) => updateLock("titleSort", v)}
        originalValue={originalFormState?.titleSort}
        placeholder="Sort title (e.g., 'Avengers, The')"
      />

      <LockableTextarea
        label="Summary"
        value={formState.summary}
        onChange={(v) => updateField("summary", v)}
        locked={locksState.summary}
        onLockChange={(v) => updateLock("summary", v)}
        originalValue={originalFormState?.summary}
        placeholder="Enter series summary..."
        minRows={4}
        autosize
      />
    </Stack>
  );

  // Details tab - Status, Language, Reading Direction, Publisher, etc.
  const renderDetailsTab = () => (
    <Stack gap="md">
      <SimpleGrid cols={2}>
        <LockableSelect
          label="Status"
          value={formState.status}
          onChange={(v) => updateField("status", v)}
          locked={locksState.status}
          onLockChange={(v) => updateLock("status", v)}
          originalValue={originalFormState?.status}
          data={STATUS_OPTIONS}
          placeholder="Select status"
          clearable
        />

        <LockableInput
          label="Language"
          value={formState.language}
          onChange={(v) => updateField("language", v)}
          locked={locksState.language}
          onLockChange={(v) => updateLock("language", v)}
          originalValue={originalFormState?.language}
          placeholder="e.g., en, ja, ko"
        />
      </SimpleGrid>

      <LockableSelect
        label="Reading Direction"
        value={formState.readingDirection}
        onChange={(v) => updateField("readingDirection", v)}
        locked={locksState.readingDirection}
        onLockChange={(v) => updateLock("readingDirection", v)}
        originalValue={originalFormState?.readingDirection}
        data={READING_DIRECTION_OPTIONS}
        placeholder="Select reading direction"
        clearable
      />

      <SimpleGrid cols={2}>
        <LockableInput
          label="Publisher"
          value={formState.publisher}
          onChange={(v) => updateField("publisher", v)}
          locked={locksState.publisher}
          onLockChange={(v) => updateLock("publisher", v)}
          originalValue={originalFormState?.publisher}
          placeholder="Publisher name"
        />

        <LockableInput
          label="Imprint"
          value={formState.imprint}
          onChange={(v) => updateField("imprint", v)}
          locked={locksState.imprint}
          onLockChange={(v) => updateLock("imprint", v)}
          originalValue={originalFormState?.imprint}
          placeholder="Sub-publisher"
        />
      </SimpleGrid>

      <SimpleGrid cols={3}>
        <LockableInput
          label="Year"
          value={formState.year}
          onChange={(v) => updateField("year", v)}
          locked={locksState.year}
          onLockChange={(v) => updateLock("year", v)}
          originalValue={originalFormState?.year}
          placeholder="Year"
          type="number"
        />

        <LockableInput
          label="Total Books"
          value={formState.totalBookCount}
          onChange={(v) => updateField("totalBookCount", v)}
          locked={locksState.totalBookCount}
          onLockChange={(v) => updateLock("totalBookCount", v)}
          originalValue={originalFormState?.totalBookCount}
          placeholder="Count"
          type="number"
        />

        <LockableInput
          label="Age Rating"
          value={formState.ageRating}
          onChange={(v) => updateField("ageRating", v)}
          locked={locksState.ageRating}
          onLockChange={(v) => updateLock("ageRating", v)}
          originalValue={originalFormState?.ageRating}
          placeholder="e.g., 13"
          type="number"
        />
      </SimpleGrid>
    </Stack>
  );

  // Authors tab
  const AUTHOR_ROLE_OPTIONS = Object.entries(AUTHOR_ROLE_DISPLAY).map(
    ([value, label]) => ({ value, label }),
  );

  const renderAuthorsTab = () => (
    <Stack gap="md">
      <Group justify="space-between">
        <Text size="sm" fw={500}>
          Authors
        </Text>
        <Tooltip
          label={
            locksState.authorsJson
              ? "Locked: Protected from automatic updates"
              : "Unlocked: Can be updated automatically"
          }
          position="left"
          zIndex={1100}
        >
          <ActionIcon
            variant="subtle"
            color={locksState.authorsJson ? "orange" : "gray"}
            onClick={() => updateLock("authorsJson", !locksState.authorsJson)}
            aria-label={
              locksState.authorsJson ? "Unlock authors" : "Lock authors"
            }
          >
            {locksState.authorsJson ? (
              <IconLock size={18} />
            ) : (
              <IconLockOpen size={18} />
            )}
          </ActionIcon>
        </Tooltip>
      </Group>

      {formState.authors.map((author, index) => (
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
                const newAuthors = [...formState.authors];
                newAuthors[index] = {
                  ...newAuthors[index],
                  role: value as BookAuthorRole,
                };
                updateField("authors", newAuthors);
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
              const newAuthors = [...formState.authors];
              newAuthors[index] = {
                ...newAuthors[index],
                name: e.target.value,
              };
              updateField("authors", newAuthors);
            }}
            style={{ flex: 2 }}
          />
          <ActionIcon
            variant="subtle"
            color="red"
            onClick={() => {
              const newAuthors = formState.authors.filter(
                (_, i) => i !== index,
              );
              updateField("authors", newAuthors);
            }}
            aria-label="Remove author"
          >
            <IconTrash size={18} />
          </ActionIcon>
        </Group>
      ))}

      <Box>
        <Button
          variant="subtle"
          leftSection={<IconPlus size={16} />}
          onClick={() => {
            updateField("authors", [
              ...formState.authors,
              { name: "", role: "author" as BookAuthorRole },
            ]);
          }}
          size="sm"
        >
          Add Author
        </Button>
      </Box>
    </Stack>
  );

  // Alternate titles tab
  const renderAlternateTitlesTab = () => (
    <Stack gap="md">
      <Group justify="space-between">
        <Text size="sm" c="dimmed">
          Add alternate titles for this series (e.g., native title, romanized
          title).
        </Text>
        <Tooltip
          label={
            locksState.alternateTitles
              ? "Locked: Protected from automatic updates"
              : "Unlocked: Can be updated automatically"
          }
          position="left"
          zIndex={1100}
        >
          <ActionIcon
            variant="subtle"
            color={locksState.alternateTitles ? "orange" : "gray"}
            onClick={() =>
              updateLock("alternateTitles", !locksState.alternateTitles)
            }
            aria-label={
              locksState.alternateTitles
                ? "Unlock alternate titles"
                : "Lock alternate titles"
            }
          >
            {locksState.alternateTitles ? (
              <IconLock size={18} />
            ) : (
              <IconLockOpen size={18} />
            )}
          </ActionIcon>
        </Tooltip>
      </Group>

      <LockableListEditor
        items={formState.alternateTitles}
        onChange={(items) => updateField("alternateTitles", items)}
        fields={[
          {
            key: "label",
            label: "Type",
            placeholder: "Select type",
            flex: 1,
          },
          {
            key: "title",
            label: "Title",
            placeholder: "Alternate title",
            flex: 2,
          },
        ]}
        originalItems={originalFormState?.alternateTitles}
        addButtonLabel="Add Alternate Title"
        generateId={() => `new-${crypto.randomUUID()}`}
      />
    </Stack>
  );

  // Tags tab
  const renderTagsTab = () => (
    <Stack gap="md">
      <LockableChipInput
        label="Genres"
        value={formState.genres}
        onChange={(v) => updateField("genres", v)}
        locked={locksState.genres}
        onLockChange={(v) => updateLock("genres", v)}
        originalValue={originalFormState?.genres}
        placeholder="Add genres..."
        description="Press Enter to add a genre"
        data={allGenres?.map((g) => g.name) ?? []}
      />

      <LockableChipInput
        label="Tags"
        value={formState.tags}
        onChange={(v) => updateField("tags", v)}
        locked={locksState.tags}
        onLockChange={(v) => updateLock("tags", v)}
        originalValue={originalFormState?.tags}
        placeholder="Add tags..."
        description="Press Enter to add a tag"
        data={allTags?.map((t) => t.name) ?? []}
      />
    </Stack>
  );

  // Links tab
  const renderLinksTab = () => (
    <Stack gap="md">
      <Text size="sm" c="dimmed">
        Add external links to other sites (e.g., MyAnimeList, AniList).
      </Text>

      <LockableListEditor
        items={formState.externalLinks}
        onChange={(items) => updateField("externalLinks", items)}
        fields={[
          {
            key: "label",
            label: "Site Name",
            placeholder: "e.g., MyAnimeList",
            flex: 1,
          },
          {
            key: "url",
            label: "URL",
            placeholder: "https://...",
            flex: 2,
          },
        ]}
        originalItems={originalFormState?.externalLinks}
        addButtonLabel="Add Link"
        generateId={() => `new-${crypto.randomUUID()}`}
        deriveValues={(fieldKey, value, currentValues) => {
          if (fieldKey === "url" && !currentValues.label) {
            const source = extractSourceFromUrl(value);
            if (source) return { label: source };
          }
          return undefined;
        }}
      />
    </Stack>
  );

  // Poster tab
  const getSeriesCoverSourceLabel = (source: string): string => {
    if (source === "custom") return "Custom Upload";
    if (source.startsWith("book:")) return "From Book";
    return source;
  };

  const renderPosterTab = () => (
    <CoverEditor
      covers={existingCovers ?? []}
      coverLocked={locksState.cover}
      onCoverLockChange={(v) => updateLock("cover", v)}
      onUpload={(file) => uploadCoverMutation.mutate(file)}
      isUploading={uploadCoverMutation.isPending}
      onSelect={(coverId) => selectCoverMutation.mutate(coverId)}
      isSelecting={selectCoverMutation.isPending}
      onReset={() => resetCoverMutation.mutate()}
      isResetting={resetCoverMutation.isPending}
      onDelete={(coverId) => deleteCoverMutation.mutate(coverId)}
      isDeleting={deleteCoverMutation.isPending}
      getCoverImageUrl={(coverId) =>
        `/api/v1/series/${seriesId}/covers/${coverId}/image`
      }
      getCoverSourceLabel={getSeriesCoverSourceLabel}
      resetButtonLabel="Reset to Default (Use First Book Cover)"
      defaultCoverMessage="Using default (first book cover)"
    />
  );

  // Custom metadata tab
  const renderCustomTab = () => (
    <CustomMetadataEditor
      value={formState.customMetadata}
      onChange={(v) => updateField("customMetadata", v)}
      locked={locksState.customMetadata}
      onLockChange={(v) => updateLock("customMetadata", v)}
      originalValue={originalFormState?.customMetadata}
    />
  );

  // Sharing tags tab (admin only)
  const renderSharingTab = () => {
    // Get all existing tag names for suggestions
    const existingTagNames = allSharingTags?.map((tag) => tag.name) || [];

    return (
      <Stack gap="md">
        <TagsInput
          label="Sharing Tags"
          description="Users with a 'deny' grant for these tags won't see this series"
          data={existingTagNames}
          value={formState.sharingTags}
          onChange={(value) => updateField("sharingTags", value)}
          placeholder="Add sharing tags..."
          comboboxProps={{ zIndex: 1100 }}
        />
      </Stack>
    );
  };

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconEdit size={20} />
          <Text fw={500}>Edit {seriesTitle || "Series"}</Text>
        </Group>
      }
      size={800}
      centered
      zIndex={1000}
      overlayProps={{
        backgroundOpacity: 0.55,
        blur: 3,
      }}
    >
      {isLoading ? (
        <Center h={300}>
          <Loader />
        </Center>
      ) : (
        <Stack gap="md">
          <Tabs value={activeTab} onChange={setActiveTab}>
            <Tabs.List>
              <Tabs.Tab value="general" leftSection={<IconList size={16} />}>
                General
              </Tabs.Tab>
              <Tabs.Tab value="details" leftSection={<IconBook size={16} />}>
                Details
              </Tabs.Tab>
              <Tabs.Tab value="authors" leftSection={<IconUsers size={16} />}>
                Authors
              </Tabs.Tab>
              <Tabs.Tab
                value="alternateTitles"
                leftSection={<IconTypography size={16} />}
              >
                Titles
              </Tabs.Tab>
              <Tabs.Tab value="tags" leftSection={<IconTag size={16} />}>
                Tags
              </Tabs.Tab>
              <Tabs.Tab value="links" leftSection={<IconLink size={16} />}>
                Links
              </Tabs.Tab>
              <Tabs.Tab value="poster" leftSection={<IconPhoto size={16} />}>
                Cover
              </Tabs.Tab>
              <Tabs.Tab value="custom" leftSection={<IconCode size={16} />}>
                Custom
              </Tabs.Tab>
              {isAdmin && (
                <Tabs.Tab value="sharing" leftSection={<IconShare size={16} />}>
                  Sharing
                </Tabs.Tab>
              )}
            </Tabs.List>

            <Tabs.Panel value="general" pt="md">
              {renderGeneralTab()}
            </Tabs.Panel>

            <Tabs.Panel value="details" pt="md">
              {renderDetailsTab()}
            </Tabs.Panel>

            <Tabs.Panel value="authors" pt="md">
              {renderAuthorsTab()}
            </Tabs.Panel>

            <Tabs.Panel value="alternateTitles" pt="md">
              {renderAlternateTitlesTab()}
            </Tabs.Panel>

            <Tabs.Panel value="tags" pt="md">
              {renderTagsTab()}
            </Tabs.Panel>

            <Tabs.Panel value="links" pt="md">
              {renderLinksTab()}
            </Tabs.Panel>

            <Tabs.Panel value="poster" pt="md">
              {renderPosterTab()}
            </Tabs.Panel>

            <Tabs.Panel value="custom" pt="md">
              {renderCustomTab()}
            </Tabs.Panel>

            {isAdmin && (
              <Tabs.Panel value="sharing" pt="md">
                {renderSharingTab()}
              </Tabs.Panel>
            )}
          </Tabs>

          <Group justify="flex-end" mt="md">
            <Button variant="subtle" onClick={onClose}>
              Cancel
            </Button>
            <Button onClick={handleSave} loading={saveMutation.isPending}>
              Save Changes
            </Button>
          </Group>
        </Stack>
      )}
    </Modal>
  );
}
