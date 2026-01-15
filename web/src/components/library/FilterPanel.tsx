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
import { tagsApi } from "@/api/tags";
import { useFilterState } from "@/hooks/useFilterState";
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
export function FilterPanel() {
	const [opened, { open, close }] = useDisclosure(false);
	const filterState = useFilterState();
	const isMobile = useMediaQuery("(max-width: 768px)");

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

	return (
		<>
			{/* Trigger Button */}
			<Indicator
				label={filterState.activeFilterCount}
				size={16}
				disabled={!filterState.hasActiveFilters}
				color="red"
			>
				<ActionIcon
					variant={filterState.hasActiveFilters ? "filled" : "subtle"}
					color={filterState.hasActiveFilters ? "blue" : undefined}
					size="lg"
					title="Filters"
					aria-label="Filter options"
					onClick={open}
				>
					<IconAdjustments size={20} />
				</ActionIcon>
			</Indicator>

			{/* Filter Drawer */}
			<Drawer
				opened={opened}
				onClose={close}
				title={
					<Group gap="sm">
						<Title order={4}>Filters</Title>
						{filterState.hasActiveFilters && (
							<Badge size="sm" variant="light">
								{filterState.activeFilterCount} active
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
									state={filterState.filters.readStatus}
									onValueChange={filterState.setReadStatusState}
									onModeChange={filterState.setReadStatusMode}
									onClear={() => filterState.clearGroup("readStatus")}
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
									state={filterState.filters.status}
									onValueChange={filterState.setStatusState}
									onModeChange={filterState.setStatusMode}
									onClear={() => filterState.clearGroup("status")}
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
												state={filterState.filters.genres}
												onValueChange={filterState.setGenreState}
												onModeChange={filterState.setGenreMode}
												onClear={() => filterState.clearGroup("genres")}
											/>
										)}

										{/* Tag Filters */}
										{tagOptions.length > 0 && (
											<FilterGroup
												title="Tags"
												options={tagOptions}
												state={filterState.filters.tags}
												onValueChange={filterState.setTagState}
												onModeChange={filterState.setTagMode}
												onClear={() => filterState.clearGroup("tags")}
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
							</Stack>
						</ScrollArea>

						{/* Footer Actions */}
						<Group justify="space-between" className={classes.footer}>
							<Button
								variant="subtle"
								color="gray"
								size="sm"
								leftSection={<IconX size={16} />}
								onClick={filterState.clearAll}
								disabled={!filterState.hasActiveFilters}
							>
								Clear all
							</Button>
							<Button size="sm" onClick={close}>
								Apply
							</Button>
						</Group>
					</Stack>
				)}
			</Drawer>
		</>
	);
}
