import { ActionIcon, Group, Menu, Stack, Tabs } from "@mantine/core";
import { useMediaQuery } from "@mantine/hooks";
import {
  IconChevronDown,
  IconChevronUp,
  IconGridDots,
  IconSortAscending,
  IconSortDescending,
} from "@tabler/icons-react";
import { MOBILE_MEDIA_QUERY } from "@/components/ui";
import { BookFilterPanel } from "./BookFilterPanel";
import { SeriesFilterPanel } from "./SeriesFilterPanel";

export interface SortOption {
  field: string;
  label: string;
  defaultDirection: "asc" | "desc";
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
  /** UUID of the library being browsed, or "all" / null for the cross-library view. */
  libraryId?: string | null;
}

// Tabs that show a curated feed without sort/page-size/filter controls.
const TABS_WITHOUT_CONTROLS = new Set([
  "recommended",
  "keep-reading",
  "on-deck",
]);

const PAGE_SIZE_OPTIONS = [
  { value: 25, label: "25" },
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
  pageSize = 50,
  onPageSizeChange,
  libraryId,
}: LibraryToolbarProps) {
  // Tabs whose content is a curated, sort-less, filter-less feed: no toolbar
  // controls apply to them.
  const showControls =
    !TABS_WITHOUT_CONTROLS.has(currentTab) && sortOptions.length > 0;
  // Below the `xs` breakpoint the tabs + controls don't fit in one row (audit
  // finding L1). Stack the controls underneath instead of letting the row wrap.
  const isMobile = useMediaQuery(MOBILE_MEDIA_QUERY) ?? false;

  const tabs = (
    <Tabs value={currentTab} onChange={onTabChange}>
      <Tabs.List>
        {showRecommended && (
          <Tabs.Tab value="recommended">Recommended</Tabs.Tab>
        )}
        <Tabs.Tab value="keep-reading">Keep Reading</Tabs.Tab>
        <Tabs.Tab value="on-deck">On Deck</Tabs.Tab>
        <Tabs.Tab value="series">Series</Tabs.Tab>
        <Tabs.Tab value="books">Books</Tabs.Tab>
      </Tabs.List>
    </Tabs>
  );

  const controls = showControls ? (
    <Group gap="xs" wrap="nowrap" justify={isMobile ? "flex-end" : undefined}>
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
              bg={
                pageSize === option.value
                  ? "var(--mantine-color-blue-light)"
                  : undefined
              }
            >
              {option.label}
            </Menu.Item>
          ))}
        </Menu.Dropdown>
      </Menu>

      {/* Sort Menu */}
      <Menu shadow="md" width={200} position="bottom-end">
        <Menu.Target>
          <ActionIcon
            variant="subtle"
            size="lg"
            title="Sort"
            aria-label="Sort options"
          >
            {sort?.endsWith(",desc") ? (
              <IconSortDescending size={20} />
            ) : (
              <IconSortAscending size={20} />
            )}
          </ActionIcon>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Label>Sort by</Menu.Label>
          {sortOptions.map((option) => {
            const currentField = sort?.split(",")[0];
            const currentDirection = sort?.split(",")[1] as
              | "asc"
              | "desc"
              | undefined;
            const isSelected = currentField === option.field;

            const handleClick = () => {
              if (isSelected) {
                // Toggle direction
                const newDirection =
                  currentDirection === "asc" ? "desc" : "asc";
                onSortChange?.(`${option.field},${newDirection}`);
              } else {
                // Use default direction for new field
                onSortChange?.(`${option.field},${option.defaultDirection}`);
              }
            };

            return (
              <Menu.Item
                key={option.field}
                onClick={handleClick}
                bg={isSelected ? "var(--mantine-color-blue-light)" : undefined}
                rightSection={
                  isSelected ? (
                    currentDirection === "desc" ? (
                      <IconChevronDown size={14} />
                    ) : (
                      <IconChevronUp size={14} />
                    )
                  ) : null
                }
              >
                {option.label}
              </Menu.Item>
            );
          })}
        </Menu.Dropdown>
      </Menu>

      {/* Filter Panel - show appropriate panel based on current tab */}
      {currentTab === "books" ? (
        <BookFilterPanel libraryId={libraryId} />
      ) : (
        <SeriesFilterPanel libraryId={libraryId} />
      )}
    </Group>
  ) : null;

  if (isMobile) {
    return (
      <Stack gap="xs">
        {tabs}
        {controls}
      </Stack>
    );
  }

  return (
    <Group justify="space-between" align="center" wrap="nowrap">
      {tabs}
      {controls}
    </Group>
  );
}
