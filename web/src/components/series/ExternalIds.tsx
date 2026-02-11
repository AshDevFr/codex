import { ActionIcon, Badge, Group, Tooltip } from "@mantine/core";
import { IconEdit, IconExternalLink, IconId } from "@tabler/icons-react";
import type { components } from "@/types/api.generated";

type SeriesExternalId = components["schemas"]["SeriesExternalIdDto"];

interface ExternalIdsProps {
  externalIds: SeriesExternalId[];
  onEdit?: () => void;
}

// Map source keys to display names and colors.
// Keys with a prefix (plugin:, api:) will show the prefix in the badge.
const SOURCE_CONFIG: Record<
  string,
  { name: string; color: string; abbrev?: string }
> = {
  // Plugin sources (provenance from a plugin's match)
  "plugin:myanimelist": {
    name: "plugin: MyAnimeList",
    color: "#2e51a2",
    abbrev: "plugin: MAL",
  },
  "plugin:anilist": { name: "plugin: AniList", color: "#02a9ff" },
  "plugin:mangabaka": { name: "plugin: MangaBaka", color: "#ff6b35" },
  "plugin:mangadex": { name: "plugin: MangaDex", color: "#ff6740" },
  "plugin:kitsu": { name: "plugin: Kitsu", color: "#f75239" },
  "plugin:mangaupdates": {
    name: "plugin: MangaUpdates",
    color: "#2a4a6d",
    abbrev: "plugin: MU",
  },
  "plugin:comicvine": { name: "plugin: Comic Vine", color: "#e41d25" },
  "plugin:goodreads": { name: "plugin: Goodreads", color: "#553b08" },
  "plugin:amazon": { name: "plugin: Amazon", color: "#ff9900" },
  // API sources (cross-reference IDs from external services)
  "api:anilist": { name: "api: AniList", color: "#02a9ff" },
  "api:myanimelist": {
    name: "api: MyAnimeList",
    color: "#2e51a2",
    abbrev: "api: MAL",
  },
  "api:mangabaka": { name: "api: MangaBaka", color: "#ff6b35" },
  "api:mangadex": { name: "api: MangaDex", color: "#ff6740" },
  "api:kitsu": { name: "api: Kitsu", color: "#f75239" },
  "api:mangaupdates": {
    name: "api: MangaUpdates",
    color: "#2a4a6d",
    abbrev: "api: MU",
  },
  "api:comicvine": { name: "api: Comic Vine", color: "#e41d25" },
  "api:goodreads": { name: "api: Goodreads", color: "#553b08" },
  "api:amazon": { name: "api: Amazon", color: "#ff9900" },
  // Unprefixed sources (file-based or user-set)
  comicinfo: { name: "ComicInfo", color: "gray" },
  epub: { name: "EPUB", color: "teal" },
  pdf: { name: "PDF", color: "red" },
  manual: { name: "Manual", color: "violet" },
};

/**
 * Format the source name for display.
 * Handles prefixed sources (plugin:, api:) and unprefixed sources.
 */
function getSourceConfig(source: string): {
  name: string;
  color: string;
  abbrev?: string;
} {
  const lowerSource = source.toLowerCase();

  // Check exact match first
  if (SOURCE_CONFIG[lowerSource]) {
    return SOURCE_CONFIG[lowerSource];
  }

  // For any prefixed source (plugin:xxx, api:xxx, etc.), show prefix: Name
  const colonIdx = lowerSource.indexOf(":");
  if (colonIdx > 0) {
    const prefix = lowerSource.slice(0, colonIdx);
    const name = lowerSource.slice(colonIdx + 1);
    return {
      name: `${prefix}: ${name.charAt(0).toUpperCase() + name.slice(1)}`,
      color: "blue",
    };
  }

  // Fallback: capitalize the source
  return {
    name: source.charAt(0).toUpperCase() + source.slice(1),
    color: "gray",
  };
}

/**
 * Format relative time since last sync.
 */
function formatLastSynced(
  dateString: string | null | undefined,
): string | null {
  if (!dateString) return null;

  const date = new Date(dateString);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return "today";
  if (diffDays === 1) return "yesterday";
  if (diffDays < 7) return `${diffDays} days ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
  if (diffDays < 365) return `${Math.floor(diffDays / 30)} months ago`;
  return `${Math.floor(diffDays / 365)} years ago`;
}

export function ExternalIds({ externalIds, onEdit }: ExternalIdsProps) {
  if (externalIds.length === 0 && !onEdit) {
    return null;
  }

  return (
    <Group gap="xs">
      {externalIds.map((extId) => {
        const config = getSourceConfig(extId.source);
        const displayName = config.abbrev || config.name;
        const lastSynced = formatLastSynced(extId.lastSyncedAt);

        const tooltipContent = [
          `Source: ${extId.source}`,
          `ID: ${extId.externalId}`,
          lastSynced ? `Last synced: ${lastSynced}` : null,
        ]
          .filter(Boolean)
          .join("\n");

        const badge = (
          <Badge
            key={extId.id}
            component={extId.externalUrl ? "a" : "span"}
            href={extId.externalUrl ?? undefined}
            target={extId.externalUrl ? "_blank" : undefined}
            rel={extId.externalUrl ? "noopener noreferrer" : undefined}
            variant="light"
            color={config.color}
            size="sm"
            leftSection={<IconId size={10} />}
            rightSection={
              extId.externalUrl ? <IconExternalLink size={10} /> : undefined
            }
            style={{ cursor: extId.externalUrl ? "pointer" : "default" }}
          >
            {displayName}
          </Badge>
        );

        return (
          <Tooltip key={extId.id} label={tooltipContent} multiline withArrow>
            {badge}
          </Tooltip>
        );
      })}
      {onEdit && (
        <Tooltip label="Edit external IDs" withArrow>
          <ActionIcon variant="subtle" color="gray" size="xs" onClick={onEdit}>
            <IconEdit size={12} />
          </ActionIcon>
        </Tooltip>
      )}
    </Group>
  );
}
