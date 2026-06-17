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
  Text,
  Title,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { notifications } from "@mantine/notifications";
import {
  IconAdjustments,
  IconBookmark,
  IconLock,
  IconNotebook,
  IconTag,
  IconX,
  type TablerIcon,
} from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import type { FilterPresetDto } from "@/api/filterPresets";
import { sharingTagsApi } from "@/api/sharingTags";
import { useDraftSeriesFilterState } from "@/hooks/useDraftSeriesFilterState";
import { useAllGenres, useAllTags } from "@/hooks/useReferenceData";
import { useSeriesFilterState } from "@/hooks/useSeriesFilterState";
import { useAuthStore } from "@/store/authStore";
import {
  conditionToSeriesFilterState,
  type SeriesCondition,
  serializeSeriesFilters,
  seriesFilterStateToCondition,
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

// Series status options (publication status from metadata)
const SERIES_STATUS_OPTIONS = [
  { value: "ongoing", label: "Ongoing" },
  { value: "ended", label: "Ended" },
  { value: "hiatus", label: "Hiatus" },
  { value: "abandoned", label: "Abandoned" },
  { value: "unknown", label: "Unknown" },
];

/**
 * Sticky section header for the filter panel. The label sits flush
 * against the top of the scrolling region; a hairline below appears
 * only when content scrolls under it (the rule lives in
 * FilterPanel.module.css; the React side only needs to render the
 * structure with the leading icon).
 */
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
 * Filter panel component that displays filter groups in a drawer.
 *
 * Features:
 * - Fetches available genres and tags from the API
 * - Displays filter groups with tri-state chips
 * - Shows active filter count on the trigger button
 * - URL-synchronized filter state
 */
export interface SeriesFilterPanelProps {
  /** UUID of the library being filtered, or null when scope is "all libraries". */
  libraryId?: string | null;
}

export function SeriesFilterPanel({ libraryId }: SeriesFilterPanelProps = {}) {
  const [opened, { open, close }] = useDisclosure(false);
  // Use committed state for the indicator badge (shows what's actually applied)
  const {
    activeFilterCount: committedFilterCount,
    hasActiveFilters: hasCommittedFilters,
  } = useSeriesFilterState();
  // Use draft state for editing within the drawer
  const draftState = useDraftSeriesFilterState();
  // Build the API condition from the draft so presets can be saved directly
  // from in-drawer selections without first hitting Apply.
  const draftCondition = useMemo(
    () => seriesFilterStateToCondition(draftState.draftFilters),
    [draftState.draftFilters],
  );
  const isMobile = useMediaQuery("(max-width: 768px)");
  const user = useAuthStore((state) => state.user);
  const isAdmin = user?.role === "admin";
  const [searchParams, setSearchParams] = useSearchParams();

  const normalizedLibraryId =
    !libraryId || libraryId === "all" ? null : libraryId;

  const handleApplyPreset = (preset: FilterPresetDto) => {
    const condition = preset.condition as unknown as
      | SeriesCondition
      | undefined;
    const next = conditionToSeriesFilterState(condition);
    if (!next) {
      notifications.show({
        title: "Preset uses advanced filters",
        message:
          "This preset can't be applied here. Open it in the advanced search page.",
        color: "yellow",
      });
      return;
    }

    const filterParams = serializeSeriesFilters(next);
    const newParams = new URLSearchParams(searchParams);
    for (const key of [
      "gf",
      "tf",
      "sf",
      "rf",
      "pf",
      "lf",
      "stf",
      "cf",
      "esf",
      "urf",
      "trf",
      "icf",
    ]) {
      newParams.delete(key);
    }
    for (const [key, value] of filterParams) {
      newParams.set(key, value);
    }
    newParams.set("page", "1");
    setSearchParams(newParams, { replace: true });
    close();
  };

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

  // Fetch available genres + tags (global reference data, shared + long-cached
  // so the multi-page sweep doesn't re-run on every remount / reconnect).
  const { data: genres = [], isLoading: genresLoading } = useAllGenres();
  const { data: tags = [], isLoading: tagsLoading } = useAllTags();

  // Fetch sharing tags (admin only)
  const { data: sharingTags = [], isLoading: sharingTagsLoading } = useQuery({
    queryKey: ["sharing-tags"],
    queryFn: () => sharingTagsApi.list(),
    staleTime: 60000,
    enabled: isAdmin,
  });

  const isLoading =
    genresLoading || tagsLoading || (isAdmin && sharingTagsLoading);

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

  const sharingTagOptions = sharingTags.map((st) => ({
    value: st.name,
    label: st.name,
    count: st.seriesCount ?? undefined,
  }));

  // Check if we have any metadata-based filters available
  const hasMetadataFilters = genreOptions.length > 0 || tagOptions.length > 0;
  const hasSharingTagFilters = isAdmin && sharingTagOptions.length > 0;

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
        target="series"
        libraryId={normalizedLibraryId}
        currentCondition={draftCondition}
        hasActiveFilters={draftState.hasActiveFilters}
        onApply={handleApplyPreset}
      />

      <Divider my={4} />

      {/* Reading Progress */}
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

      <FilterGroup
        title="My Rating"
        options={[{ value: "rated", label: "Has Rating" }]}
        state={{
          mode: "allOf",
          values:
            draftState.draftFilters.hasUserRating !== "neutral"
              ? new Map([["rated", draftState.draftFilters.hasUserRating]])
              : new Map(),
        }}
        onValueChange={(_value, state) =>
          draftState.setHasUserRatingState(state)
        }
        onModeChange={() => {}}
        showModeToggle={false}
        variant="neutral"
      />

      <Divider my={4} />

      {/* Publication */}
      <SectionHeader icon={IconNotebook}>Publication</SectionHeader>

      <FilterGroup
        title="Status"
        options={SERIES_STATUS_OPTIONS}
        state={draftState.draftFilters.status}
        onValueChange={draftState.setStatusState}
        onModeChange={draftState.setStatusMode}
        onClear={() => draftState.clearGroupDraft("status")}
        showModeToggle={false}
        variant="status"
      />

      <FilterGroup
        title="Completeness"
        options={[{ value: "complete", label: "Complete" }]}
        state={{
          mode: "allOf",
          values:
            draftState.draftFilters.completion !== "neutral"
              ? new Map([["complete", draftState.draftFilters.completion]])
              : new Map(),
        }}
        onValueChange={(_value, state) => draftState.setCompletionState(state)}
        onModeChange={() => {}}
        showModeToggle={false}
        variant="neutral"
      />

      <FilterGroup
        title="Metadata Source"
        options={[{ value: "linked", label: "Has External ID" }]}
        state={{
          mode: "allOf",
          values:
            draftState.draftFilters.hasExternalSourceId !== "neutral"
              ? new Map([
                  ["linked", draftState.draftFilters.hasExternalSourceId],
                ])
              : new Map(),
        }}
        onValueChange={(_value, state) =>
          draftState.setHasExternalSourceIdState(state)
        }
        onModeChange={() => {}}
        showModeToggle={false}
        variant="neutral"
      />

      <FilterGroup
        title="Release Tracking"
        options={[{ value: "tracked", label: "Tracked" }]}
        state={{
          mode: "allOf",
          values:
            draftState.draftFilters.isTracked !== "neutral"
              ? new Map([["tracked", draftState.draftFilters.isTracked]])
              : new Map(),
        }}
        onValueChange={(_value, state) => draftState.setIsTrackedState(state)}
        onModeChange={() => {}}
        showModeToggle={false}
        variant="neutral"
      />

      <FilterGroup
        title="Collections"
        options={[{ value: "inCollection", label: "In Collection" }]}
        state={{
          mode: "allOf",
          values:
            draftState.draftFilters.inCollection !== "neutral"
              ? new Map([
                  ["inCollection", draftState.draftFilters.inCollection],
                ])
              : new Map(),
        }}
        onValueChange={(_value, state) =>
          draftState.setInCollectionState(state)
        }
        onModeChange={() => {}}
        showModeToggle={false}
        variant="neutral"
      />

      {/* Metadata Section - Only show if there's data */}
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
              searchable
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
              searchable
            />
          )}
        </>
      )}

      {/* Empty state hint when no metadata */}
      {!hasMetadataFilters && (
        <Text size="sm" c="dimmed" fs="italic" mt="md">
          Genre and tag filters will appear here once your library has metadata.
          You can add genres and tags to series from the series detail page.
        </Text>
      )}

      {/* Access Control Section - Admin only */}
      {hasSharingTagFilters && (
        <>
          <Divider my={4} />
          <SectionHeader icon={IconLock}>Access Control</SectionHeader>

          <FilterGroup
            title="Sharing Tags"
            options={sharingTagOptions}
            state={draftState.draftFilters.sharingTags}
            onValueChange={draftState.setSharingTagState}
            onModeChange={draftState.setSharingTagMode}
            onClear={() => draftState.clearGroupDraft("sharingTags")}
            searchable
          />
        </>
      )}
    </Stack>
  );

  return (
    <>
      {/* Trigger Button - shows committed filter count */}
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

      {/* Mobile: bottom sheet with snap points. Desktop: right-side drawer. */}
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
