import { ActionIcon, Group, Menu, Tabs } from "@mantine/core";
import {
	IconAdjustments,
	IconGridDots,
	IconSortAscending,
} from "@tabler/icons-react";

interface SortOption {
	value: string;
	label: string;
}

interface LibraryToolbarProps {
	currentTab: string;
	onTabChange: (value: string | null) => void;
	showRecommended?: boolean;
	sort?: string;
	onSortChange?: (value: string) => void;
	sortOptions?: SortOption[];
	pageSize?: number;
	onPageSizeChange?: (value: number) => void;
}

const PAGE_SIZE_OPTIONS = [
	{ value: 20, label: "20" },
	{ value: 50, label: "50" },
	{ value: 100, label: "100" },
	{ value: 200, label: "200" },
	{ value: 500, label: "500" },
];

export function LibraryToolbar({
	currentTab,
	onTabChange,
	showRecommended = true,
	sort,
	onSortChange,
	sortOptions = [],
	pageSize = 20,
	onPageSizeChange,
}: LibraryToolbarProps) {
	const showControls = currentTab !== "recommended" && sortOptions.length > 0;

	return (
		<Group justify="space-between" align="center" wrap="nowrap">
			<Tabs value={currentTab} onChange={onTabChange}>
				<Tabs.List>
					{showRecommended && <Tabs.Tab value="recommended">Recommended</Tabs.Tab>}
					<Tabs.Tab value="series">Series</Tabs.Tab>
					<Tabs.Tab value="books">Books</Tabs.Tab>
				</Tabs.List>
			</Tabs>

			{showControls && (
				<Group gap="xs" wrap="nowrap">
					{/* Sort Menu */}
					<Menu shadow="md" width={200} position="bottom-end">
						<Menu.Target>
							<ActionIcon
								variant="subtle"
								size="lg"
								title="Sort"
								aria-label="Sort options"
							>
								<IconSortAscending size={20} />
							</ActionIcon>
						</Menu.Target>
						<Menu.Dropdown>
							<Menu.Label>Sort by</Menu.Label>
							{sortOptions.map((option) => (
								<Menu.Item
									key={option.value}
									onClick={() => onSortChange?.(option.value)}
									bg={sort === option.value ? "var(--mantine-color-blue-light)" : undefined}
								>
									{option.label}
								</Menu.Item>
							))}
						</Menu.Dropdown>
					</Menu>

					{/* Page Size Menu */}
					<Menu shadow="md" width={120} position="bottom-end">
						<Menu.Target>
							<ActionIcon
								variant="subtle"
								size="lg"
								title="Page size"
								aria-label="Page size options"
							>
								<IconGridDots size={20} />
							</ActionIcon>
						</Menu.Target>
						<Menu.Dropdown>
							<Menu.Label>Page size</Menu.Label>
							{PAGE_SIZE_OPTIONS.map((option) => (
								<Menu.Item
									key={option.value}
									onClick={() => onPageSizeChange?.(option.value)}
									bg={pageSize === option.value ? "var(--mantine-color-blue-light)" : undefined}
								>
									{option.label}
								</Menu.Item>
							))}
						</Menu.Dropdown>
					</Menu>

					{/* Filter Menu - Placeholder for future implementation */}
					<ActionIcon
						variant="subtle"
						size="lg"
						title="Filters"
						aria-label="Filter options"
						disabled
					>
						<IconAdjustments size={20} />
					</ActionIcon>
				</Group>
			)}
		</Group>
	);
}
