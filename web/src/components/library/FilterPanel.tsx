import { ActionIcon, Badge, Button, Drawer, Group, Loader, ScrollArea, Stack, Text, Title } from "@mantine/core";
import { useDisclosure, useMediaQuery } from "@mantine/hooks";
import { IconAdjustments, IconX } from "@tabler/icons-react";
import { useQuery } from "@tanstack/react-query";
import { FilterGroup } from "./FilterGroup";
import { useFilterState } from "@/hooks/useFilterState";
import { genresApi } from "@/api/genres";
import { tagsApi } from "@/api/tags";
import classes from "./FilterPanel.module.css";

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
		count: g.series_count,
	}));

	const tagOptions = tags.map((t) => ({
		value: t.name,
		label: t.name,
		count: t.series_count,
	}));

	// Static status options (these match the backend enum)
	const statusOptions = [
		{ value: "ongoing", label: "Ongoing" },
		{ value: "ended", label: "Ended" },
		{ value: "hiatus", label: "Hiatus" },
		{ value: "abandoned", label: "Abandoned" },
		{ value: "unknown", label: "Unknown" },
	];

	return (
		<>
			{/* Trigger Button */}
			<ActionIcon
				variant={filterState.hasActiveFilters ? "filled" : "subtle"}
				color={filterState.hasActiveFilters ? "blue" : undefined}
				size="lg"
				title="Filters"
				aria-label="Filter options"
				onClick={open}
				className={classes.triggerButton}
			>
				<IconAdjustments size={20} />
				{filterState.hasActiveFilters && (
					<Badge size="xs" variant="filled" color="red" className={classes.filterBadge}>
						{filterState.activeFilterCount}
					</Badge>
				)}
			</ActionIcon>

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

								{/* Status Filters */}
								<FilterGroup
									title="Status"
									options={statusOptions}
									state={filterState.filters.status}
									onValueChange={filterState.setStatusState}
									onModeChange={filterState.setStatusMode}
									onClear={() => filterState.clearGroup("status")}
									showModeToggle={false}
								/>
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
