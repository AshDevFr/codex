import { Badge, Group, Tooltip } from "@mantine/core";
import { IconExternalLink, IconId } from "@tabler/icons-react";
import type { components } from "@/types/api.generated";

type SeriesExternalId = components["schemas"]["SeriesExternalIdDto"];

interface ExternalIdsProps {
  externalIds: SeriesExternalId[];
}

// Map source prefixes to display names and colors
const SOURCE_CONFIG: Record<
  string,
  { name: string; color: string; abbrev?: string }
> = {
  "plugin:myanimelist": {
    name: "MyAnimeList",
    color: "#2e51a2",
    abbrev: "MAL",
  },
  "plugin:anilist": { name: "AniList", color: "#02a9ff" },
  "plugin:mangabaka": { name: "MangaBaka", color: "#ff6b35" },
  "plugin:mangadex": { name: "MangaDex", color: "#ff6740" },
  "plugin:kitsu": { name: "Kitsu", color: "#f75239" },
  "plugin:mangaupdates": {
    name: "MangaUpdates",
    color: "#2a4a6d",
    abbrev: "MU",
  },
  "plugin:comicvine": { name: "Comic Vine", color: "#e41d25" },
  "plugin:goodreads": { name: "Goodreads", color: "#553b08" },
  "plugin:amazon": { name: "Amazon", color: "#ff9900" },
  comicinfo: { name: "ComicInfo", color: "gray" },
  epub: { name: "EPUB", color: "teal" },
  pdf: { name: "PDF", color: "red" },
  manual: { name: "Manual", color: "violet" },
};

/**
 * Format the source name for display.
 * Handles plugin: prefix and known sources.
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

  // Check if it's a plugin source with a known plugin name
  if (lowerSource.startsWith("plugin:")) {
    const pluginName = lowerSource.replace("plugin:", "");
    // Check if there's a config for this plugin name without the prefix
    for (const [key, config] of Object.entries(SOURCE_CONFIG)) {
      if (key === `plugin:${pluginName}` || key === pluginName) {
        return config;
      }
    }
    // Fallback: capitalize the plugin name
    return {
      name: pluginName.charAt(0).toUpperCase() + pluginName.slice(1),
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

export function ExternalIds({ externalIds }: ExternalIdsProps) {
  if (externalIds.length === 0) {
    return null;
  }

  return (
    <Group gap="xs">
      {externalIds.map((extId) => {
        const config = getSourceConfig(extId.source);
        const displayName = config.abbrev || config.name;
        const lastSynced = formatLastSynced(extId.lastSyncedAt);

        const tooltipContent = [
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
    </Group>
  );
}
