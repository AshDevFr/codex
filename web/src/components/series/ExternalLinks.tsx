import { Badge, Group } from "@mantine/core";
import { IconExternalLink } from "@tabler/icons-react";
import { capitalize } from "es-toolkit/string";

/** Minimal interface for external link data. Works with both series and book external links. */
export interface ExternalLinkItem {
  id: string;
  sourceName: string;
  url: string;
}

interface ExternalLinksProps {
  links: ExternalLinkItem[];
}

// Map source names to display names and colors
// Shared between series and books
const SOURCE_CONFIG: Record<
  string,
  { name: string; color: string; abbrev?: string }
> = {
  // Series/manga sources
  myanimelist: { name: "MyAnimeList", color: "#2e51a2", abbrev: "MAL" },
  anilist: { name: "AniList", color: "#02a9ff" },
  mangabaka: { name: "MangaBaka", color: "#ff6b35" },
  mangadex: { name: "MangaDex", color: "#ff6740" },
  kitsu: { name: "Kitsu", color: "#f75239" },
  mangaupdates: { name: "MangaUpdates", color: "#2a4a6d", abbrev: "MU" },
  comicvine: { name: "Comic Vine", color: "#e41d25" },
  // Book sources
  openlibrary: { name: "Open Library", color: "#00a388", abbrev: "OL" },
  goodreads: { name: "Goodreads", color: "#553b08" },
  amazon: { name: "Amazon", color: "#ff9900" },
  googlebooks: { name: "Google Books", color: "#4285F4", abbrev: "GB" },
};

/**
 * Extract a human-readable site name from a URL.
 * Strips www and TLDs, then joins remaining parts capitalized.
 *
 * e.g. "https://www.goodreads.com/book/show/123" → "Goodreads"
 *      "https://books.google.com/..." → "Google Books"
 *      "https://comicvine.gamespot.com/..." → "Gamespot Comicvine"
 *      "https://openlibrary.org/works/OL123" → "Openlibrary"
 *      "https://amazon.co.uk/dp/..." → "Amazon"
 * Returns undefined if the URL is invalid.
 */
export function extractSourceFromUrl(url: string): string | undefined {
  try {
    const hostname = new URL(url).hostname;
    const parts = hostname.split(".");

    // Strip TLD(s): remove last part, and also second-to-last if it's short (co.uk, co.jp, etc.)
    let tldCount = 1;
    if (parts.length >= 3 && parts[parts.length - 2].length <= 2) {
      tldCount = 2;
    }
    const meaningful = parts
      .slice(0, parts.length - tldCount)
      .filter((p) => p !== "www");

    if (meaningful.length === 0) return capitalize(parts[0]);

    // Domain first, then subdomains: ["books", "google"] → "Google Books"
    return meaningful.reverse().map(capitalize).join(" ");
  } catch {
    return undefined;
  }
}

export function ExternalLinks({ links }: ExternalLinksProps) {
  if (links.length === 0) {
    return null;
  }

  return (
    <Group gap="xs">
      {links.map((link) => {
        const config = SOURCE_CONFIG[link.sourceName.toLowerCase()] || {
          name: link.sourceName,
          color: "gray",
        };
        const displayName = config.abbrev || config.name;

        return (
          <Badge
            key={link.id}
            component="a"
            href={link.url}
            target="_blank"
            rel="noopener noreferrer"
            variant="light"
            color={config.color}
            size="sm"
            rightSection={<IconExternalLink size={10} />}
            style={{ cursor: "pointer" }}
          >
            {displayName}
          </Badge>
        );
      })}
    </Group>
  );
}
