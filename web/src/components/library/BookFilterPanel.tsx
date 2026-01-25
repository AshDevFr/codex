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
	Switch,
	Text,
	Title,
} from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { IconAdjustments, IconX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { genresApi } from "@/api/genres";
import { tagsApi } from "@/api/tags";
import { useBookFilterState } from "@/hooks/useBookFilterState";
import { useDraftBookFilterState } from "@/hooks/useDraftBookFilterState";
import { useUserPreferencesStore } from "@/store/userPreferencesStore";
import { FilterGroup } from "./FilterGroup";
import classes from "./FilterPanel.module.css";

// Read status options (user's reading progress)
const READ_STATUS_OPTIONS = [
	{ value: "unread", label: "Unread" },
	{ value: "in_progress", label: "In Progress" },
	{ value: "read", label: "Read" },
];

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
export function BookFilterPanel() {
	const [opened, { open, close }] = useDisclosure(false);
	// Use committed state for the indicator badge (shows what's actually applied)
	const {
		activeFilterCount: committedFilterCount,
		hasActiveFilters: hasCommittedFilters,
	} = useBookFilterState();
	// Use draft state for editing within the drawer
	const draftState = useDraftBookFilterState();
	const isMobile = useMediaQuery("(max-width: 768px)");

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

								<Divider my="xs" />

								{/* Book Status Section */}
								<Text size="xs" fw={700} tt="uppercase" c="dimmed">
									Book Status
								</Text>

								{/* Has Error Toggle */}
								<Group justify="space-between" px="xs">
									<Text size="sm">Show books with errors</Text>
									<Switch
										checked={draftState.draftFilters.hasError === "include"}
										onChange={handleHasErrorToggle}
										color={
											draftState.draftFilters.hasError === "include"
												? "red"
												: "blue"
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

								{/* Show Deleted Toggle */}
								<Group justify="space-between" px="xs">
									<Text size="sm">Show deleted books</Text>
									<Switch
										checked={showDeletedBooks}
										onChange={(e) =>
											setPreference(
												"library.show_deleted_books",
												e.currentTarget.checked,
											)
										}
										color="red"
									/>
								</Group>

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
										metadata. Books inherit genres and tags from their series.
									</Text>
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
