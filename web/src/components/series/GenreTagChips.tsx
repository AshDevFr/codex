import { Badge, Group, Text } from "@mantine/core";
import { useState } from "react";
import { Link } from "react-router-dom";
import type { Genre } from "@/api/genres";
import type { Tag } from "@/api/tags";

interface BadgeItem {
  id: string;
  name: string;
}

interface BadgeGroup {
  items: BadgeItem[];
  color: string;
  getUrl?: (item: BadgeItem) => string;
}

interface GenreTagChipsProps {
  /** Pre-configured genre badges (blue, clickable by default) */
  genres?: Genre[];
  /** Pre-configured tag badges (gray, clickable by default) */
  tags?: Tag[];
  /** Generic badge groups with custom colors and optional links */
  groups?: BadgeGroup[];
  libraryId?: string;
  clickable?: boolean;
  maxDisplay?: number;
}

export function GenreTagChips({
  genres = [],
  tags = [],
  groups = [],
  libraryId,
  clickable = true,
  maxDisplay,
}: GenreTagChipsProps) {
  const [expanded, setExpanded] = useState(false);
  const basePath = libraryId ? `/libraries/${libraryId}` : "/libraries/all";

  const getGenreUrl = (genre: BadgeItem) =>
    `${basePath}/series?gf=any:${encodeURIComponent(genre.name)}`;

  const getTagUrl = (tag: BadgeItem) =>
    `${basePath}/series?tf=any:${encodeURIComponent(tag.name)}`;

  // Build unified badge groups: genres first, then tags, then custom groups
  const allGroups: BadgeGroup[] = [
    ...(genres.length > 0
      ? [
          {
            items: genres,
            color: "blue",
            getUrl: clickable ? getGenreUrl : undefined,
          },
        ]
      : []),
    ...(tags.length > 0
      ? [
          {
            items: tags,
            color: "gray",
            getUrl: clickable ? getTagUrl : undefined,
          },
        ]
      : []),
    ...groups,
  ];

  // Flatten all items for counting
  const totalCount = allGroups.reduce((sum, g) => sum + g.items.length, 0);

  if (totalCount === 0) {
    return null;
  }

  const shouldCollapse = maxDisplay != null && totalCount > maxDisplay;
  const isCollapsed = shouldCollapse && !expanded;

  // Determine which items to display from each group
  let remaining = isCollapsed ? maxDisplay : totalCount;
  const displayGroups = allGroups.map((group) => {
    const take = Math.min(group.items.length, remaining);
    remaining -= take;
    return { ...group, displayItems: group.items.slice(0, take) };
  });

  const displayedCount = displayGroups.reduce(
    (sum, g) => sum + g.displayItems.length,
    0,
  );
  const hiddenCount = isCollapsed ? totalCount - displayedCount : 0;

  return (
    <Group gap="xs">
      {displayGroups.map((group) =>
        group.displayItems.map((item) =>
          group.getUrl ? (
            <Badge
              key={`${group.color}-${item.id}`}
              component={Link}
              to={group.getUrl(item)}
              variant="light"
              color={group.color}
              size="sm"
              style={{ cursor: "pointer", textDecoration: "none" }}
            >
              {item.name}
            </Badge>
          ) : (
            <Badge
              key={`${group.color}-${item.id}`}
              variant="light"
              color={group.color}
              size="sm"
            >
              {item.name}
            </Badge>
          ),
        ),
      )}
      {hiddenCount > 0 && (
        <Text
          size="xs"
          c="dimmed"
          style={{ cursor: "pointer" }}
          onClick={() => setExpanded(true)}
        >
          +{hiddenCount} more
        </Text>
      )}
      {expanded && shouldCollapse && (
        <Text
          size="xs"
          c="dimmed"
          style={{ cursor: "pointer" }}
          onClick={() => setExpanded(false)}
        >
          Show less
        </Text>
      )}
    </Group>
  );
}
