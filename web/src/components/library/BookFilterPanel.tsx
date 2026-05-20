import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Divider,
  Drawer,
  Group,
  Indicator,
  Loader,
  ScrollArea,
  Stack,
  Switch,
  Text,
  Title,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAdjustments,
  IconAlertTriangle,
  IconBookmark,
  IconCategory,
  IconTag,
  IconX,
  type TablerIcon,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useSearchParams } from "react-router-dom";
import type { FilterPresetDto } from "@/api/filterPresets";
import { genresApi } from "@/api/genres";
import { tagsApi } from "@/api/tags";
import { useBookFilterState } from "@/hooks/useBookFilterState";
import { useDraftBookFilterState } from "@/hooks/useDraftBookFilterState";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import {
  BOOK_FILTER_PARAM_KEYS,
  type BookCondition,
  conditionToBookFilterState,
  serializeBookFilters,
} from "@/types/filters";
import { FilterBottomSheet } from "./FilterBottomSheet";
import { FilterGroup } from "./FilterGroup";
import classes from "./FilterPanel.module.css";
import { ListPresetControls } from "./ListPresetControls";

// Read status options (user's reading progress)
const READ_STATUS_OPTIONS = [
  { value: "unread", label: "Unread" },
  { value: "in_progress", label: "In Progress" },
  { value: "read", label: "Read" },
];

// Book type options (classification of the book)
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

function SectionHeader({
  icon: Icon,
  children,
}: {
  icon: TablerIcon;
  children: ReactNode;
}) {
  return (
    <div className={classes.sectionHeader}>
      <span className={classes.sectionHeaderIcon} aria-hidden>
        <Icon size={14} />
      </span>
      {children}
    </div>
  );
}

/**
 * Filter panel component for books that displays filter groups in a drawer.
 *
 * Features:
 * - Fetches available genres and tags from the API
 * - Displays filter groups with tri-state chips
 * - Shows active filter count on the trigger button
 * - URL-synchronized filter state
 * - Book-specific filters: Read Status, Has Error
 */
export interface BookFilterPanelProps {
  /** UUID of the library being filtered, or null when scope is "all libraries". */
  libraryId?: string | null;
}

export function BookFilterPanel({ libraryId }: BookFilterPanelProps = {}) {
  const [opened, { open, close }] = useDisclosure(false);
  // Use committed state for the indicator badge (shows what's actually applied)
  const {
    activeFilterCount: committedFilterCount,
    hasActiveFilters: hasCommittedFilters,
    condition: committedCondition,
  } = useBookFilterState();
  // Use draft state for editing within the drawer
  const draftState = useDraftBookFilterState();
  const isMobile = useMediaQuery("(max-width: 768px)");
  const [searchParams, setSearchParams] = useSearchParams();

  const normalizedLibraryId =
    !libraryId || libraryId === "all" ? null : libraryId;

  const handleApplyPreset = (preset: FilterPresetDto) => {
    const condition = preset.condition as unknown as BookCondition | undefined;
    const next = conditionToBookFilterState(condition);
    if (!next) {
      notifications.show({
        title: "Preset uses advanced filters",
        message:
          "This preset can't be applied here. Open it in the advanced search page.",
        color: "yellow",
      });
      return;
    }

    const filterParams = serializeBookFilters(next);
    const newParams = new URLSearchParams(searchParams);
    newParams.delete(BOOK_FILTER_PARAM_KEYS.genres);
    newParams.delete(BOOK_FILTER_PARAM_KEYS.tags);
    newParams.delete(BOOK_FILTER_PARAM_KEYS.readStatus);
    newParams.delete(BOOK_FILTER_PARAM_KEYS.bookType);
    newParams.delete(BOOK_FILTER_PARAM_KEYS.hasError);
    for (const [key, value] of filterParams) {
      newParams.set(key, value);
    }
    newParams.set("page", "1");
    setSearchParams(newParams, { replace: true });
    close();
  };

  // Get show deleted preference from user preferences store
  const showDeletedBooks = useUserPreferencesStore((state) =>
    state.getPreference("library.show_deleted_books"),
  );
  const setPreference = useUserPreferencesStore((state) => state.setPreference);

  // Handle Apply - commit draft to URL and close
  const handleApply = () => {
    draftState.applyFilters();
    close();
  };

  // Handle Close - discard draft changes and close
  const handleClose = () => {
    draftState.discardChanges();
    close();
  };

  // Handle Clear All - clear filters, apply, and close
  const handleClearAll = () => {
    draftState.clearAllAndApply();
    close();
  };

  // Fetch available genres (global, not library-specific)
  const { data: genres = [], isLoading: genresLoading } = useQuery({
    queryKey: ["genres"],
    queryFn: () => genresApi.getAll(),
    staleTime: 60000, // Cache for 1 minute
  });

  // Fetch available tags (global, not library-specific)
  const { data: tags = [], isLoading: tagsLoading } = useQuery({
    queryKey: ["tags"],
    queryFn: () => tagsApi.getAll(),
    staleTime: 60000,
  });

  const isLoading = genresLoading || tagsLoading;

  // Transform API data to filter options
  const genreOptions = genres.map((g) => ({
    value: g.name,
    label: g.name,
    count: g.seriesCount ?? undefined,
  }));

  const tagOptions = tags.map((t) => ({
    value: t.name,
    label: t.name,
    count: t.seriesCount ?? undefined,
  }));

  // Check if we have any metadata-based filters available
  const hasMetadataFilters = genreOptions.length > 0 || tagOptions.length > 0;

  // Handle hasError toggle - cycle through neutral -> include (show errors) -> exclude (hide errors)
  const handleHasErrorToggle = () => {
    if (draftState.draftFilters.hasError === "neutral") {
      draftState.setHasErrorState("include");
    } else if (draftState.draftFilters.hasError === "include") {
      draftState.setHasErrorState("exclude");
    } else {
      draftState.setHasErrorState("neutral");
    }
  };

  const titleNode = (
    <Group gap="sm">
      <Title order={4}>Filters</Title>
      {draftState.hasActiveFilters && (
        <Badge size="sm" variant="light">
          {draftState.activeFilterCount} active
        </Badge>
      )}
    </Group>
  );

  const footerNode = (
    <Group justify="space-between" className={classes.footer}>
      <Button
        variant="subtle"
        color="gray"
        size="sm"
        leftSection={<IconX size={16} />}
        onClick={handleClearAll}
        disabled={!draftState.hasActiveFilters}
      >
        Clear all
      </Button>
      <Button size="sm" onClick={handleApply}>
        Apply
      </Button>
    </Group>
  );

  const body = isLoading ? (
    <Group justify="center" py="xl">
      <Loader size="md" />
      <Text size="sm" c="dimmed">
        Loading filter options...
      </Text>
    </Group>
  ) : (
    <Stack gap="sm">
      <ListPresetControls
        target="books"
        libraryId={normalizedLibraryId}
        currentCondition={committedCondition}
        hasActiveFilters={hasCommittedFilters}
        onApply={handleApplyPreset}
      />

      <Divider my={4} />

      <SectionHeader icon={IconBookmark}>Reading Progress</SectionHeader>

      <FilterGroup
        title="Read Status"
        options={READ_STATUS_OPTIONS}
        state={draftState.draftFilters.readStatus}
        onValueChange={draftState.setReadStatusState}
        onModeChange={draftState.setReadStatusMode}
        onClear={() => draftState.clearGroupDraft("readStatus")}
        showModeToggle={false}
        variant="progress"
      />

      <Divider my={4} />

      <SectionHeader icon={IconAlertTriangle}>Book Status</SectionHeader>

      <Group justify="space-between" px="xs">
        <Text size="sm">Show books with errors</Text>
        <Switch
          checked={draftState.draftFilters.hasError === "include"}
          onChange={handleHasErrorToggle}
          color={
            draftState.draftFilters.hasError === "include" ? "red" : "blue"
          }
          label={
            draftState.draftFilters.hasError === "neutral"
              ? "All"
              : draftState.draftFilters.hasError === "include"
                ? "Only errors"
                : "No errors"
          }
        />
      </Group>

      <Group justify="space-between" px="xs">
        <Text size="sm">Show deleted books</Text>
        <Switch
          checked={showDeletedBooks}
          onChange={(e) =>
            setPreference("library.show_deleted_books", e.currentTarget.checked)
          }
          color="red"
        />
      </Group>

      <Divider my={4} />

      <SectionHeader icon={IconCategory}>Book Type</SectionHeader>

      <FilterGroup
        title="Book Type"
        options={BOOK_TYPE_OPTIONS}
        state={draftState.draftFilters.bookType}
        onValueChange={draftState.setBookTypeState}
        onModeChange={draftState.setBookTypeMode}
        onClear={() => draftState.clearGroupDraft("bookType")}
      />

      {hasMetadataFilters && (
        <>
          <Divider my={4} />
          <SectionHeader icon={IconTag}>Metadata</SectionHeader>

          {genreOptions.length > 0 && (
            <FilterGroup
              title="Genres"
              options={genreOptions}
              state={draftState.draftFilters.genres}
              onValueChange={draftState.setGenreState}
              onModeChange={draftState.setGenreMode}
              onClear={() => draftState.clearGroupDraft("genres")}
            />
          )}

          {tagOptions.length > 0 && (
            <FilterGroup
              title="Tags"
              options={tagOptions}
              state={draftState.draftFilters.tags}
              onValueChange={draftState.setTagState}
              onModeChange={draftState.setTagMode}
              onClear={() => draftState.clearGroupDraft("tags")}
            />
          )}
        </>
      )}

      {!hasMetadataFilters && (
        <Text size="sm" c="dimmed" fs="italic" mt="md">
          Genre and tag filters will appear here once your library has metadata.
          Books inherit genres and tags from their series.
        </Text>
      )}
    </Stack>
  );

  return (
    <>
      <Indicator
        label={committedFilterCount}
        size={16}
        disabled={!hasCommittedFilters}
        color="red"
      >
        <ActionIcon
          variant={hasCommittedFilters ? "filled" : "subtle"}
          color={hasCommittedFilters ? "blue" : undefined}
          size="lg"
          title="Filters"
          aria-label="Filter options"
          onClick={open}
        >
          <IconAdjustments size={20} />
        </ActionIcon>
      </Indicator>

      {isMobile ? (
        <FilterBottomSheet
          opened={opened}
          onClose={handleClose}
          title={titleNode}
          footer={footerNode}
        >
          <ScrollArea flex={1} offsetScrollbars>
            <Box pb="md">{body}</Box>
          </ScrollArea>
        </FilterBottomSheet>
      ) : (
        <Drawer
          opened={opened}
          onClose={handleClose}
          title={titleNode}
          position="right"
          size="md"
          padding="md"
          classNames={{
            body: classes.drawerBody,
          }}
        >
          <Stack gap="sm" h="100%">
            <ScrollArea flex={1} offsetScrollbars>
              <Box pb="md">{body}</Box>
            </ScrollArea>
            {footerNode}
          </Stack>
        </Drawer>
      )}
    </>
  );
}
