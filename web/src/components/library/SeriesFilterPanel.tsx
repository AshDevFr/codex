import {
  ActionIcon,
  Badge,
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
import { IconAdjustments, IconX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { genresApi } from "@/api/genres";
import { sharingTagsApi } from "@/api/sharingTags";
import { tagsApi } from "@/api/tags";
import { useDraftSeriesFilterState } from "@/hooks/useDraftSeriesFilterState";
import { useSeriesFilterState } from "@/hooks/useSeriesFilterState";
import { useAuthStore } from "@/store/authStore";
import { FilterGroup } from "./FilterGroup";
import classes from "./FilterPanel.module.css";

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
 * Filter panel component that displays filter groups in a drawer.
 *
 * Features:
 * - Fetches available genres and tags from the API
 * - Displays filter groups with tri-state chips
 * - Shows active filter count on the trigger button
 * - URL-synchronized filter state
 */
export function SeriesFilterPanel() {
  const [opened, { open, close }] = useDisclosure(false);
  // Use committed state for the indicator badge (shows what's actually applied)
  const {
    activeFilterCount: committedFilterCount,
    hasActiveFilters: hasCommittedFilters,
  } = useSeriesFilterState();
  // Use draft state for editing within the drawer
  const draftState = useDraftSeriesFilterState();
  const isMobile = useMediaQuery("(max-width: 768px)");
  const user = useAuthStore((state) => state.user);
  const isAdmin = user?.role === "admin";

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

      {/* Filter Drawer - uses draft state */}
      <Drawer
        opened={opened}
        onClose={handleClose}
        title={
          <Group gap="sm">
            <Title order={4}>Filters</Title>
            {draftState.hasActiveFilters && (
              <Badge size="sm" variant="light">
                {draftState.activeFilterCount} active
              </Badge>
            )}
          </Group>
        }
        position="right"
        size={isMobile ? "100%" : "md"}
        padding="md"
        classNames={{
          body: classes.drawerBody,
        }}
      >
        {isLoading ? (
          <Group justify="center" py="xl">
            <Loader size="md" />
            <Text size="sm" c="dimmed">
              Loading filter options...
            </Text>
          </Group>
        ) : (
          <Stack gap="md" h="100%">
            <ScrollArea flex={1} offsetScrollbars>
              <Stack gap="md">
                {/* Reading Progress Section */}
                <Text size="xs" fw={700} tt="uppercase" c="dimmed">
                  Reading Progress
                </Text>

                {/* Read Status Filters */}
                <FilterGroup
                  title="Read Status"
                  options={READ_STATUS_OPTIONS}
                  state={draftState.draftFilters.readStatus}
                  onValueChange={draftState.setReadStatusState}
                  onModeChange={draftState.setReadStatusMode}
                  onClear={() => draftState.clearGroupDraft("readStatus")}
                  showModeToggle={false}
                />

                {/* User Rating Filter */}
                <FilterGroup
                  title="My Rating"
                  options={[{ value: "rated", label: "Has Rating" }]}
                  state={{
                    mode: "allOf",
                    values:
                      draftState.draftFilters.hasUserRating !== "neutral"
                        ? new Map([
                            ["rated", draftState.draftFilters.hasUserRating],
                          ])
                        : new Map(),
                  }}
                  onValueChange={(_value, state) =>
                    draftState.setHasUserRatingState(state)
                  }
                  onModeChange={() => {}}
                  showModeToggle={false}
                />

                <Divider my="xs" />

                {/* Publication Status Section */}
                <Text size="xs" fw={700} tt="uppercase" c="dimmed">
                  Publication
                </Text>

                {/* Status Filters */}
                <FilterGroup
                  title="Status"
                  options={SERIES_STATUS_OPTIONS}
                  state={draftState.draftFilters.status}
                  onValueChange={draftState.setStatusState}
                  onModeChange={draftState.setStatusMode}
                  onClear={() => draftState.clearGroupDraft("status")}
                  showModeToggle={false}
                />

                {/* Collection Completion Filter */}
                <FilterGroup
                  title="Collection"
                  options={[{ value: "complete", label: "Complete" }]}
                  state={{
                    mode: "allOf",
                    values:
                      draftState.draftFilters.completion !== "neutral"
                        ? new Map([
                            ["complete", draftState.draftFilters.completion],
                          ])
                        : new Map(),
                  }}
                  onValueChange={(_value, state) =>
                    draftState.setCompletionState(state)
                  }
                  onModeChange={() => {}}
                  showModeToggle={false}
                />

                {/* External Source ID Filter */}
                <FilterGroup
                  title="Metadata Source"
                  options={[{ value: "linked", label: "Has External ID" }]}
                  state={{
                    mode: "allOf",
                    values:
                      draftState.draftFilters.hasExternalSourceId !== "neutral"
                        ? new Map([
                            [
                              "linked",
                              draftState.draftFilters.hasExternalSourceId,
                            ],
                          ])
                        : new Map(),
                  }}
                  onValueChange={(_value, state) =>
                    draftState.setHasExternalSourceIdState(state)
                  }
                  onModeChange={() => {}}
                  showModeToggle={false}
                />

                {/* Metadata Section - Only show if there's data */}
                {hasMetadataFilters && (
                  <>
                    <Divider my="xs" />
                    <Text size="xs" fw={700} tt="uppercase" c="dimmed">
                      Metadata
                    </Text>

                    {/* Genre Filters */}
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

                    {/* Tag Filters */}
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

                {/* Empty state hint when no metadata */}
                {!hasMetadataFilters && (
                  <Text size="sm" c="dimmed" fs="italic" mt="md">
                    Genre and tag filters will appear here once your library has
                    metadata. You can add genres and tags to series from the
                    series detail page.
                  </Text>
                )}

                {/* Access Control Section - Admin only */}
                {hasSharingTagFilters && (
                  <>
                    <Divider my="xs" />
                    <Text size="xs" fw={700} tt="uppercase" c="dimmed">
                      Access Control
                    </Text>

                    <FilterGroup
                      title="Sharing Tags"
                      options={sharingTagOptions}
                      state={draftState.draftFilters.sharingTags}
                      onValueChange={draftState.setSharingTagState}
                      onModeChange={draftState.setSharingTagMode}
                      onClear={() => draftState.clearGroupDraft("sharingTags")}
                    />
                  </>
                )}
              </Stack>
            </ScrollArea>

            {/* Footer Actions */}
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
          </Stack>
        )}
      </Drawer>
    </>
  );
}
