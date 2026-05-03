import {
  ActionIcon,
  Box,
  Button,
  Center,
  Group,
  Loader,
  Modal,
  Select,
  Stack,
  Switch,
  Tabs,
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
  IconTag,
  IconTrash,
  IconUsers,
} from "@tabler/icons-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";
import {
  type BookDetailResponse,
  type BookMetadataLocks,
  booksApi,
} from "@/api/books";
import { genresApi } from "@/api/genres";
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
import type { BookTypeDto } from "@/types";
import type { BookAuthor, BookAuthorRole } from "@/types/book-metadata";
import { AUTHOR_ROLE_DISPLAY } from "@/types/book-metadata";

const BOOK_TYPE_OPTIONS = [
  { value: "comic", label: "Comic" },
  { value: "manga", label: "Manga" },
  { value: "novel", label: "Novel" },
  { value: "novella", label: "Novella" },
  { value: "anthology", label: "Anthology" },
  { value: "artbook", label: "Artbook" },
  { value: "oneshot", label: "Oneshot" },
  { value: "omnibus", label: "Omnibus" },
  { value: "graphic_novel", label: "Graphic Novel" },
  { value: "magazine", label: "Magazine" },
];

export interface BookMetadataEditModalProps {
  opened: boolean;
  onClose: () => void;
  bookId: string;
  bookTitle?: string;
}

interface FormState {
  // General
  title: string;
  number: string;
  summary: string;
  bookType: string | null;
  subtitle: string;
  releaseYear: string;
  releaseMonth: string;
  releaseDay: string;
  isbn: string;
  volume: string;
  chapter: string;
  count: string;
  // Publication
  edition: string;
  originalTitle: string;
  originalYear: string;
  seriesPosition: string;
  seriesTotal: string;
  translator: string;
  subjects: string;
  // Authors
  authors: BookAuthor[];
  // Publishing
  publisher: string;
  imprint: string;
  genre: string;
  languageIso: string;
  formatDetail: string;
  // Flags
  blackAndWhite: boolean | null;
  manga: boolean | null;
  // Genres & Tags (many-to-many)
  genres: string[];
  tags: string[];
  // Custom metadata
  customMetadata: Record<string, unknown> | null;
  // Links
  externalLinks: ListItem[];
}

interface LocksState {
  title: boolean;
  titleSort: boolean;
  number: boolean;
  summary: boolean;
  bookType: boolean;
  subtitle: boolean;
  publisher: boolean;
  imprint: boolean;
  genre: boolean;
  languageIso: boolean;
  formatDetail: boolean;
  blackAndWhite: boolean;
  manga: boolean;
  year: boolean;
  month: boolean;
  day: boolean;
  volume: boolean;
  chapter: boolean;
  count: boolean;
  isbns: boolean;
  edition: boolean;
  originalTitle: boolean;
  originalYear: boolean;
  seriesPosition: boolean;
  seriesTotal: boolean;
  translator: boolean;
  subjects: boolean;
  authorsJson: boolean;
  awardsJson: boolean;
  customMetadata: boolean;
  cover: boolean;
}

function buildAuthorsFromLegacy(
  metadata: BookDetailResponse["metadata"] | undefined,
): BookAuthor[] {
  if (!metadata) return [];
  const authors: BookAuthor[] = [];
  const addRole = (names: string[] | undefined, role: BookAuthorRole) => {
    for (const name of names ?? []) {
      authors.push({ name, role });
    }
  };
  addRole(metadata.writers, "writer");
  addRole(metadata.pencillers, "penciller");
  addRole(metadata.inkers, "inker");
  addRole(metadata.colorists, "colorist");
  addRole(metadata.letterers, "letterer");
  addRole(metadata.coverArtists, "cover_artist");
  addRole(metadata.editors, "editor");
  return authors;
}

function initializeFormState(
  detail: BookDetailResponse | undefined,
): FormState {
  const metadata = detail?.metadata;
  return {
    title: detail?.book.title || "",
    number: detail?.book.number?.toString() || "",
    summary: metadata?.summary || "",
    bookType: metadata?.bookType || null,
    subtitle: metadata?.subtitle || "",
    releaseYear: metadata?.year?.toString() || "",
    releaseMonth: metadata?.month?.toString() || "",
    releaseDay: metadata?.day?.toString() || "",
    isbn: metadata?.isbns || "",
    volume: metadata?.volume?.toString() || "",
    chapter: metadata?.chapter?.toString() || "",
    count: metadata?.count?.toString() || "",
    edition: metadata?.edition || "",
    originalTitle: metadata?.originalTitle || "",
    originalYear: metadata?.originalYear?.toString() || "",
    seriesPosition: metadata?.seriesPosition?.toString() || "",
    seriesTotal: metadata?.seriesTotal?.toString() || "",
    translator: metadata?.translator || "",
    subjects: metadata?.subjects?.join(", ") || "",
    authors: metadata?.authors ?? buildAuthorsFromLegacy(metadata),
    publisher: metadata?.publisher || "",
    imprint: metadata?.imprint || "",
    genre: metadata?.genre || "",
    languageIso: metadata?.languageIso || "",
    formatDetail: metadata?.formatDetail || "",
    blackAndWhite: metadata?.blackAndWhite ?? null,
    manga: metadata?.manga ?? null,
    genres: [],
    tags: [],
    customMetadata:
      (metadata?.customMetadata as Record<string, unknown>) ?? null,
    externalLinks: [],
  };
}

function initializeLocksState(
  locks: BookMetadataLocks | undefined,
): LocksState {
  return {
    title: locks?.titleLock || false,
    titleSort: locks?.titleSortLock || false,
    number: locks?.numberLock || false,
    summary: locks?.summaryLock || false,
    bookType: locks?.bookTypeLock || false,
    subtitle: locks?.subtitleLock || false,
    publisher: locks?.publisherLock || false,
    imprint: locks?.imprintLock || false,
    genre: locks?.genreLock || false,
    languageIso: locks?.languageIsoLock || false,
    formatDetail: locks?.formatDetailLock || false,
    blackAndWhite: locks?.blackAndWhiteLock || false,
    manga: locks?.mangaLock || false,
    year: locks?.yearLock || false,
    month: locks?.monthLock || false,
    day: locks?.dayLock || false,
    volume: locks?.volumeLock || false,
    chapter: locks?.chapterLock || false,
    count: locks?.countLock || false,
    isbns: locks?.isbnsLock || false,
    edition: locks?.editionLock || false,
    originalTitle: locks?.originalTitleLock || false,
    originalYear: locks?.originalYearLock || false,
    seriesPosition: locks?.seriesPositionLock || false,
    seriesTotal: locks?.seriesTotalLock || false,
    translator: locks?.translatorLock || false,
    subjects: locks?.subjectsLock || false,
    authorsJson: locks?.authorsJsonLock || false,
    awardsJson: locks?.awardsJsonLock || false,
    customMetadata: locks?.customMetadataLock || false,
    cover: locks?.coverLock || false,
  };
}

export function BookMetadataEditModal({
  opened,
  onClose,
  bookId,
  bookTitle,
}: BookMetadataEditModalProps) {
  const queryClient = useQueryClient();
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

  // Fetch book detail
  const { data: bookDetail, isLoading: isLoadingBook } = useQuery({
    queryKey: ["books", bookId, "detail"],
    queryFn: () => booksApi.getDetail(bookId),
    enabled: opened,
  });

  // Fetch locks
  const { data: locks, isLoading: isLoadingLocks } = useQuery({
    queryKey: ["books", bookId, "metadata", "locks"],
    queryFn: () => booksApi.getMetadataLocks(bookId),
    enabled: opened,
  });

  // Fetch external links
  const { data: externalLinks } = useQuery({
    queryKey: ["books", bookId, "external-links"],
    queryFn: () => booksApi.listExternalLinks(bookId),
    enabled: opened,
  });

  // Fetch existing covers
  const { data: existingCovers, refetch: refetchCovers } = useQuery({
    queryKey: ["books", bookId, "covers"],
    queryFn: () => booksApi.listCovers(bookId),
    enabled: opened,
  });

  // Fetch genres for this book
  const { data: bookGenres } = useQuery({
    queryKey: ["books", bookId, "genres"],
    queryFn: () => genresApi.getForBook(bookId),
    enabled: opened,
  });

  // Fetch tags for this book
  const { data: bookTags } = useQuery({
    queryKey: ["books", bookId, "tags"],
    queryFn: () => tagsApi.getForBook(bookId),
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

  const isLoading = isLoadingBook || isLoadingLocks;

  // Initialize form state when data loads
  useEffect(() => {
    if (bookDetail) {
      const newFormState = initializeFormState(bookDetail);
      // Populate external links if available
      if (externalLinks) {
        newFormState.externalLinks = externalLinks.map((l) => ({
          id: l.id,
          values: { label: l.sourceName, url: l.url },
          locked: false,
        }));
      }
      // Populate genres and tags from API
      if (bookGenres) {
        newFormState.genres = bookGenres.map((g) => g.name);
      }
      if (bookTags) {
        newFormState.tags = bookTags.map((t) => t.name);
      }
      setFormState(newFormState);
      setOriginalFormState(newFormState);
    }
  }, [bookDetail, externalLinks, bookGenres, bookTags]);

  useEffect(() => {
    if (locks) {
      setLocksState(initializeLocksState(locks));
    }
  }, [locks]);

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

  // Cover mutations
  const uploadCoverMutation = useMutation({
    mutationFn: (file: File) => booksApi.uploadCover(bookId, file),
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
    mutationFn: (coverId: string) => booksApi.selectCover(bookId, coverId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Cover selected",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["books", bookId] });
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
    mutationFn: () => booksApi.resetCover(bookId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Reset to default cover",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["books", bookId] });
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
    mutationFn: (coverId: string) => booksApi.deleteCover(bookId, coverId),
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Cover deleted",
        color: "green",
      });
      refetchCovers();
      queryClient.invalidateQueries({ queryKey: ["books", bookId] });
    },
    onError: (error: { message?: string }) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to delete cover",
        color: "red",
      });
    },
  });

  // Save mutation
  const saveMutation = useMutation({
    mutationFn: async () => {
      // Update book core fields (title, number) if changed
      const titleChanged = formState.title !== originalFormState?.title;
      const numberChanged = formState.number !== originalFormState?.number;

      if (titleChanged || numberChanged) {
        const patchData: { title?: string | null; number?: number | null } = {};
        if (titleChanged) {
          patchData.title = formState.title || null;
        }
        if (numberChanged) {
          patchData.number = formState.number
            ? Number.parseFloat(formState.number)
            : null;
        }
        await booksApi.patch(bookId, patchData);
      }

      // Update metadata
      await booksApi.patchMetadata(bookId, {
        summary: formState.summary || null,
        bookType: (formState.bookType as BookTypeDto) || null,
        subtitle: formState.subtitle || null,
        authors:
          formState.authors.length > 0
            ? formState.authors.filter((a) => a.name.trim())
            : null,
        publisher: formState.publisher || null,
        imprint: formState.imprint || null,
        genre: formState.genre || null,
        languageIso: formState.languageIso || null,
        formatDetail: formState.formatDetail || null,
        blackAndWhite: formState.blackAndWhite,
        manga: formState.manga,
        year: formState.releaseYear
          ? Number.parseInt(formState.releaseYear, 10)
          : null,
        month: formState.releaseMonth
          ? Number.parseInt(formState.releaseMonth, 10)
          : null,
        day: formState.releaseDay
          ? Number.parseInt(formState.releaseDay, 10)
          : null,
        volume: formState.volume ? Number.parseInt(formState.volume, 10) : null,
        chapter: formState.chapter
          ? Number.parseFloat(formState.chapter)
          : null,
        count: formState.count ? Number.parseInt(formState.count, 10) : null,
        isbns: formState.isbn || null,
        edition: formState.edition || null,
        originalTitle: formState.originalTitle || null,
        originalYear: formState.originalYear
          ? Number.parseInt(formState.originalYear, 10)
          : null,
        seriesPosition: formState.seriesPosition
          ? Number.parseFloat(formState.seriesPosition)
          : null,
        seriesTotal: formState.seriesTotal
          ? Number.parseInt(formState.seriesTotal, 10)
          : null,
        translator: formState.translator || null,
        subjects: formState.subjects
          ? formState.subjects
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean)
          : null,
        customMetadata: formState.customMetadata as Record<
          string,
          never
        > | null,
      });

      // Handle external links changes
      const currentSourceNames = new Set(
        formState.externalLinks.map((l) => l.values.label.toLowerCase().trim()),
      );

      // Delete removed links (by source name from original)
      for (const link of originalFormState?.externalLinks || []) {
        const originalSource = link.values.label.toLowerCase().trim();
        if (!currentSourceNames.has(originalSource)) {
          await booksApi.deleteExternalLink(bookId, originalSource);
        }
      }

      // Create/upsert new and modified links
      for (const link of formState.externalLinks) {
        const sourceName = link.values.label.trim();
        const url = link.values.url.trim();
        if (sourceName && url) {
          await booksApi.createExternalLink(bookId, {
            sourceName,
            url,
          });
        }
      }

      // Update genres if changed
      const genresChanged =
        JSON.stringify(formState.genres.slice().sort()) !==
        JSON.stringify((originalFormState?.genres || []).slice().sort());
      if (genresChanged) {
        await genresApi.setForBook(bookId, formState.genres);
      }

      // Update tags if changed
      const tagsChanged =
        JSON.stringify(formState.tags.slice().sort()) !==
        JSON.stringify((originalFormState?.tags || []).slice().sort());
      if (tagsChanged) {
        await tagsApi.setForBook(bookId, formState.tags);
      }

      // Update locks
      await booksApi.updateMetadataLocks(bookId, {
        titleLock: locksState.title,
        titleSortLock: locksState.titleSort,
        numberLock: locksState.number,
        summaryLock: locksState.summary,
        bookTypeLock: locksState.bookType,
        subtitleLock: locksState.subtitle,
        publisherLock: locksState.publisher,
        imprintLock: locksState.imprint,
        genreLock: locksState.genre,
        languageIsoLock: locksState.languageIso,
        formatDetailLock: locksState.formatDetail,
        blackAndWhiteLock: locksState.blackAndWhite,
        mangaLock: locksState.manga,
        yearLock: locksState.year,
        monthLock: locksState.month,
        dayLock: locksState.day,
        volumeLock: locksState.volume,
        chapterLock: locksState.chapter,
        countLock: locksState.count,
        isbnsLock: locksState.isbns,
        editionLock: locksState.edition,
        originalTitleLock: locksState.originalTitle,
        originalYearLock: locksState.originalYear,
        seriesPositionLock: locksState.seriesPosition,
        seriesTotalLock: locksState.seriesTotal,
        translatorLock: locksState.translator,
        subjectsLock: locksState.subjects,
        authorsJsonLock: locksState.authorsJson,
        awardsJsonLock: locksState.awardsJson,
        customMetadataLock: locksState.customMetadata,
        coverLock: locksState.cover,
      });
    },
    onSuccess: () => {
      notifications.show({
        title: "Success",
        message: "Book metadata updated successfully",
        color: "green",
      });
      queryClient.invalidateQueries({ queryKey: ["books", bookId] });
      queryClient.invalidateQueries({
        queryKey: ["books", bookId, "external-links"],
      });
      queryClient.invalidateQueries({
        queryKey: ["books", bookId, "genres"],
      });
      queryClient.invalidateQueries({
        queryKey: ["books", bookId, "tags"],
      });
      queryClient.invalidateQueries({ queryKey: ["genres"] });
      queryClient.invalidateQueries({ queryKey: ["tags"] });
      onClose();
    },
    onError: (error: Error) => {
      notifications.show({
        title: "Error",
        message: error.message || "Failed to update book metadata",
        color: "red",
      });
    },
  });

  const handleSave = () => {
    saveMutation.mutate();
  };

  // General tab
  const renderGeneralTab = () => (
    <Stack gap="md">
      <LockableInput
        label="Title"
        value={formState.title}
        onChange={(v) => updateField("title", v)}
        locked={locksState.title}
        onLockChange={(v) => updateLock("title", v)}
        originalValue={originalFormState?.title}
        placeholder="Book title"
        description="Display name for this book"
      />

      <LockableInput
        label="Subtitle"
        value={formState.subtitle}
        onChange={(v) => updateField("subtitle", v)}
        locked={locksState.subtitle}
        onLockChange={(v) => updateLock("subtitle", v)}
        originalValue={originalFormState?.subtitle}
        placeholder="e.g., A Novel"
      />

      <LockableSelect
        label="Book Type"
        value={formState.bookType}
        onChange={(v) => updateField("bookType", v)}
        locked={locksState.bookType}
        onLockChange={(v) => updateLock("bookType", v)}
        originalValue={originalFormState?.bookType}
        data={BOOK_TYPE_OPTIONS}
        placeholder="Select book type"
        clearable
      />

      <LockableInput
        label="Number"
        value={formState.number}
        onChange={(v) => updateField("number", v)}
        locked={locksState.number}
        onLockChange={(v) => updateLock("number", v)}
        originalValue={originalFormState?.number}
        placeholder="e.g., 1, 2.5, 10"
        description="Book number in series (decimals allowed for sorting)"
      />

      <LockableTextarea
        label="Summary"
        value={formState.summary}
        onChange={(v) => updateField("summary", v)}
        locked={locksState.summary}
        onLockChange={(v) => updateLock("summary", v)}
        originalValue={originalFormState?.summary}
        placeholder="Enter book summary..."
        minRows={4}
        autosize
      />

      <Group grow>
        <LockableInput
          label="Year"
          value={formState.releaseYear}
          onChange={(v) => updateField("releaseYear", v)}
          locked={locksState.year}
          onLockChange={(v) => updateLock("year", v)}
          originalValue={originalFormState?.releaseYear}
          placeholder="YYYY"
          type="number"
        />
        <LockableInput
          label="Month"
          value={formState.releaseMonth}
          onChange={(v) => updateField("releaseMonth", v)}
          locked={locksState.month}
          onLockChange={(v) => updateLock("month", v)}
          originalValue={originalFormState?.releaseMonth}
          placeholder="1-12"
          type="number"
        />
        <LockableInput
          label="Day"
          value={formState.releaseDay}
          onChange={(v) => updateField("releaseDay", v)}
          locked={locksState.day}
          onLockChange={(v) => updateLock("day", v)}
          originalValue={originalFormState?.releaseDay}
          placeholder="1-31"
          type="number"
        />
      </Group>

      <LockableInput
        label="ISBN"
        value={formState.isbn}
        onChange={(v) => updateField("isbn", v)}
        locked={locksState.isbns}
        onLockChange={(v) => updateLock("isbns", v)}
        originalValue={originalFormState?.isbn}
        placeholder="978-..."
      />
    </Stack>
  );

  // Publication tab
  const renderPublicationTab = () => (
    <Stack gap="md">
      <LockableInput
        label="Edition"
        value={formState.edition}
        onChange={(v) => updateField("edition", v)}
        locked={locksState.edition}
        onLockChange={(v) => updateLock("edition", v)}
        originalValue={originalFormState?.edition}
        placeholder="e.g., First Edition, Revised"
      />

      <LockableInput
        label="Original Title"
        value={formState.originalTitle}
        onChange={(v) => updateField("originalTitle", v)}
        locked={locksState.originalTitle}
        onLockChange={(v) => updateLock("originalTitle", v)}
        originalValue={originalFormState?.originalTitle}
        placeholder="Original title (for translated works)"
      />

      <LockableInput
        label="Original Year"
        value={formState.originalYear}
        onChange={(v) => updateField("originalYear", v)}
        locked={locksState.originalYear}
        onLockChange={(v) => updateLock("originalYear", v)}
        originalValue={originalFormState?.originalYear}
        placeholder="YYYY"
        type="number"
      />

      <Group grow>
        <LockableInput
          label="Series Position"
          value={formState.seriesPosition}
          onChange={(v) => updateField("seriesPosition", v)}
          locked={locksState.seriesPosition}
          onLockChange={(v) => updateLock("seriesPosition", v)}
          originalValue={originalFormState?.seriesPosition}
          placeholder="e.g., 1, 2.5"
          description="Position in a series (decimals allowed)"
        />
        <LockableInput
          label="Series Total"
          value={formState.seriesTotal}
          onChange={(v) => updateField("seriesTotal", v)}
          locked={locksState.seriesTotal}
          onLockChange={(v) => updateLock("seriesTotal", v)}
          originalValue={originalFormState?.seriesTotal}
          placeholder="Total books in series"
          type="number"
        />
      </Group>

      <Group grow>
        <LockableInput
          label="Volume"
          value={formState.volume}
          onChange={(v) => updateField("volume", v)}
          locked={locksState.volume}
          onLockChange={(v) => updateLock("volume", v)}
          originalValue={originalFormState?.volume}
          placeholder="Volume number"
          type="number"
        />
        <LockableInput
          label="Chapter"
          value={formState.chapter}
          onChange={(v) => updateField("chapter", v)}
          locked={locksState.chapter}
          onLockChange={(v) => updateLock("chapter", v)}
          originalValue={originalFormState?.chapter}
          placeholder="Chapter (e.g. 42 or 42.5)"
          type="number"
          step="any"
        />
        <LockableInput
          label="Count"
          value={formState.count}
          onChange={(v) => updateField("count", v)}
          locked={locksState.count}
          onLockChange={(v) => updateLock("count", v)}
          originalValue={originalFormState?.count}
          placeholder="Total in series"
          type="number"
        />
      </Group>

      <LockableInput
        label="Translator"
        value={formState.translator}
        onChange={(v) => updateField("translator", v)}
        locked={locksState.translator}
        onLockChange={(v) => updateLock("translator", v)}
        originalValue={originalFormState?.translator}
        placeholder="Translator name"
      />
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

  // Tags tab
  const renderTagsTab = () => (
    <Stack gap="md">
      <LockableChipInput
        label="Genres"
        value={formState.genres}
        onChange={(v) => updateField("genres", v)}
        locked={locksState.genre}
        onLockChange={(v) => updateLock("genre", v)}
        originalValue={originalFormState?.genres}
        placeholder="Add genres..."
        description="Press Enter to add a genre"
        data={allGenres?.map((g) => g.name) ?? []}
      />

      <LockableChipInput
        label="Tags"
        value={formState.tags}
        onChange={(v) => updateField("tags", v)}
        locked={false}
        onLockChange={() => {}}
        originalValue={originalFormState?.tags}
        placeholder="Add tags..."
        description="Press Enter to add a tag"
        data={allTags?.map((t) => t.name) ?? []}
      />

      <LockableInput
        label="Subjects"
        value={formState.subjects}
        onChange={(v) => updateField("subjects", v)}
        locked={locksState.subjects}
        onLockChange={(v) => updateLock("subjects", v)}
        originalValue={originalFormState?.subjects}
        placeholder="Comma-separated (e.g., Science Fiction, Space)"
        description="Topic tags for classification"
      />

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
        placeholder="Imprint (sub-publisher)"
      />

      <LockableInput
        label="Language"
        value={formState.languageIso}
        onChange={(v) => updateField("languageIso", v)}
        locked={locksState.languageIso}
        onLockChange={(v) => updateLock("languageIso", v)}
        originalValue={originalFormState?.languageIso}
        placeholder="e.g., en, ja, ko"
      />

      <LockableInput
        label="Format"
        value={formState.formatDetail}
        onChange={(v) => updateField("formatDetail", v)}
        locked={locksState.formatDetail}
        onLockChange={(v) => updateLock("formatDetail", v)}
        originalValue={originalFormState?.formatDetail}
        placeholder="e.g., Trade Paperback, Hardcover"
      />

      <Group>
        <Switch
          label="Black and White"
          checked={formState.blackAndWhite ?? false}
          onChange={(e) =>
            updateField("blackAndWhite", e.currentTarget.checked)
          }
        />
        <Switch
          label="Manga"
          checked={formState.manga ?? false}
          onChange={(e) => updateField("manga", e.currentTarget.checked)}
        />
      </Group>
    </Stack>
  );

  // Links tab
  const renderLinksTab = () => (
    <Stack gap="md">
      <Text size="sm" c="dimmed">
        Add external links to other sites (e.g., Open Library, Goodreads,
        Amazon).
      </Text>

      <LockableListEditor
        items={formState.externalLinks}
        onChange={(items) => updateField("externalLinks", items)}
        fields={[
          {
            key: "label",
            label: "Site Name",
            placeholder: "e.g., openlibrary",
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
          // Auto-fill site name when a URL is pasted/typed and label is empty
          if (fieldKey === "url" && !currentValues.label) {
            const source = extractSourceFromUrl(value);
            if (source) return { label: source };
          }
          return undefined;
        }}
      />
    </Stack>
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

  // Poster tab
  const getBookCoverSourceLabel = (source: string): string => {
    if (source === "upload") return "Custom Upload";
    if (source.startsWith("plugin:")) return source.replace("plugin:", "");
    if (source === "embedded") return "Embedded";
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
      getCoverImageUrl={(coverId) => booksApi.getCoverImageUrl(bookId, coverId)}
      getCoverSourceLabel={getBookCoverSourceLabel}
      resetButtonLabel="Reset to Default Cover"
      defaultCoverMessage="Using default (embedded cover)"
    />
  );

  return (
    <Modal
      opened={opened}
      onClose={onClose}
      title={
        <Group gap="xs">
          <IconEdit size={20} />
          <Text fw={500}>Edit {bookTitle || "Book"}</Text>
        </Group>
      }
      size={800}
      styles={{ content: { width: 800 } }}
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
              <Tabs.Tab
                value="publication"
                leftSection={<IconBook size={16} />}
              >
                Publication
              </Tabs.Tab>
              <Tabs.Tab value="authors" leftSection={<IconUsers size={16} />}>
                Authors
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
            </Tabs.List>

            <Tabs.Panel value="general" pt="md">
              {renderGeneralTab()}
            </Tabs.Panel>

            <Tabs.Panel value="publication" pt="md">
              {renderPublicationTab()}
            </Tabs.Panel>

            <Tabs.Panel value="authors" pt="md">
              {renderAuthorsTab()}
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
