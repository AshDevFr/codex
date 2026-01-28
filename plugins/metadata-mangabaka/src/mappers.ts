/**
 * Mappers to convert MangaBaka API responses to Codex plugin protocol types
 */

import type {
  AlternateTitle,
  ExternalLink,
  ExternalRating,
  PluginSeriesMetadata,
  ReadingDirection,
  SearchResult,
  SeriesStatus,
} from "@codex/plugin-sdk";
import type { MbContentRating, MbSeries, MbSeriesType, MbStatus } from "./types.js";

/**
 * Map MangaBaka status to protocol SeriesStatus
 * MangaBaka uses: cancelled, completed, hiatus, releasing, unknown, upcoming
 * Codex uses: ongoing, ended, hiatus, abandoned, unknown
 */
function mapStatus(mbStatus: MbStatus): SeriesStatus {
  switch (mbStatus) {
    case "completed":
      return "ended";
    case "releasing":
    case "upcoming":
      return "ongoing";
    case "hiatus":
      return "hiatus";
    case "cancelled":
      return "abandoned";
    default:
      return "unknown";
  }
}

/**
 * Format genre from snake_case to Title Case
 */
function formatGenre(genre: string): string {
  return genre
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

/**
 * Detect language code from country of origin
 */
function detectLanguageFromCountry(country: string | null | undefined): string | undefined {
  if (!country) return undefined;

  const countryLower = country.toLowerCase();
  if (countryLower === "jp" || countryLower === "japan") return "ja";
  if (countryLower === "kr" || countryLower === "korea" || countryLower === "south korea")
    return "ko";
  if (countryLower === "cn" || countryLower === "china") return "zh";
  if (countryLower === "tw" || countryLower === "taiwan") return "zh-TW";

  return undefined;
}

/**
 * Map MangaBaka content rating to numeric age rating
 */
function mapContentRating(rating: MbContentRating | null | undefined): number | undefined {
  if (!rating) return undefined;

  switch (rating) {
    case "safe":
      return 0; // All ages
    case "suggestive":
      return 13; // Teen
    case "erotica":
      return 16; // Mature
    case "pornographic":
      return 18; // Adults only
    default:
      return undefined;
  }
}

/**
 * Extract rating value from either a number or an object with bayesian/average
 */
function extractRating(
  rating: number | { bayesian?: number | null; average?: number | null } | null | undefined,
): number | undefined {
  if (rating == null) return undefined;
  if (typeof rating === "number") return rating;
  return rating.bayesian ?? rating.average ?? undefined;
}

/**
 * Infer reading direction from series type and country
 */
function inferReadingDirection(
  seriesType: MbSeriesType,
  country: string | null | undefined,
): ReadingDirection | undefined {
  // Manhwa (Korean) and Manhua (Chinese) are typically left-to-right
  if (seriesType === "manhwa" || seriesType === "manhua") {
    return "ltr";
  }

  // Manga (Japanese) is typically right-to-left
  if (seriesType === "manga") {
    return "rtl";
  }

  // OEL (Original English Language) is left-to-right
  if (seriesType === "oel") {
    return "ltr";
  }

  // Fall back to country-based detection
  if (country) {
    const countryLower = country.toLowerCase();
    if (countryLower === "jp" || countryLower === "japan") return "rtl";
    if (countryLower === "kr" || countryLower === "korea" || countryLower === "south korea")
      return "ltr";
    if (countryLower === "cn" || countryLower === "china") return "ltr";
    if (countryLower === "tw" || countryLower === "taiwan") return "ltr";
  }

  return undefined;
}

/**
 * Map a MangaBaka series to a protocol SearchResult
 */
export function mapSearchResult(series: MbSeries): SearchResult {
  // Get cover URL - prefer x250 for search results
  const coverUrl = series.cover?.x250?.x1 ?? series.cover?.raw?.url ?? undefined;

  // Build alternate titles array
  const alternateTitles: string[] = [];
  if (series.native_title && series.native_title !== series.title) {
    alternateTitles.push(series.native_title);
  }
  if (series.romanized_title && series.romanized_title !== series.title) {
    alternateTitles.push(series.romanized_title);
  }

  // Note: relevanceScore is omitted - the API already returns results in relevance order
  return {
    externalId: String(series.id),
    title: series.title,
    alternateTitles,
    year: series.year ?? undefined,
    coverUrl: coverUrl ?? undefined,
    preview: {
      status: mapStatus(series.status),
      genres: (series.genres ?? []).slice(0, 3).map(formatGenre),
      rating: extractRating(series.rating),
      description: series.description?.slice(0, 200) ?? undefined,
    },
  };
}

/**
 * Map full series response to protocol PluginSeriesMetadata
 */
export function mapSeriesMetadata(series: MbSeries): PluginSeriesMetadata {
  // Build alternate titles array with language info
  const alternateTitles: AlternateTitle[] = [];

  // Add native title
  if (series.native_title && series.native_title !== series.title) {
    alternateTitles.push({
      title: series.native_title,
      language: detectLanguageFromCountry(series.country_of_origin),
      titleType: "native",
    });
  }

  // Add romanized title
  if (series.romanized_title && series.romanized_title !== series.title) {
    alternateTitles.push({
      title: series.romanized_title,
      language: "en",
      titleType: "romaji",
    });
  }

  // Add secondary titles from all languages
  if (series.secondary_titles) {
    for (const [langCode, titleList] of Object.entries(series.secondary_titles)) {
      if (titleList) {
        for (const titleEntry of titleList) {
          if (titleEntry.title !== series.title) {
            alternateTitles.push({
              title: titleEntry.title,
              language: langCode,
            });
          }
        }
      }
    }
  }

  // Extract authors and artists as string arrays
  const authors = series.authors ?? [];
  const artists = series.artists ?? [];

  // Format genres
  const genres = (series.genres ?? []).map(formatGenre);

  // Get cover URL - prefer raw for full metadata
  const coverUrl = series.cover?.raw?.url ?? series.cover?.x350?.x1 ?? undefined;

  // Build external links from sources
  // Always include MangaBaka link first
  const externalLinks: ExternalLink[] = [
    {
      url: `https://mangabaka.org/${series.id}`,
      label: "MangaBaka",
      linkType: "provider",
    },
  ];

  if (series.source?.anilist?.id) {
    externalLinks.push({
      url: `https://anilist.co/manga/${series.source.anilist.id}`,
      label: "AniList",
      linkType: "provider",
    });
  }
  if (series.source?.mal?.id) {
    externalLinks.push({
      url: `https://myanimelist.net/manga/${series.source.mal.id}`,
      label: "MyAnimeList",
      linkType: "provider",
    });
  }
  if (series.source?.mangadex?.id) {
    externalLinks.push({
      url: `https://mangadex.org/title/${series.source.mangadex.id}`,
      label: "MangaDex",
      linkType: "provider",
    });
  }

  // Build external ratings from sources (all normalized to 0-100 scale)
  const externalRatings: ExternalRating[] = [];

  if (series.source?.anilist?.rating_normalized != null) {
    externalRatings.push({ score: series.source.anilist.rating_normalized, source: "anilist" });
  }
  if (series.source?.my_anime_list?.rating_normalized != null) {
    externalRatings.push({
      score: series.source.my_anime_list.rating_normalized,
      source: "myanimelist",
    });
  }
  if (series.source?.mangadex?.rating_normalized != null) {
    externalRatings.push({ score: series.source.mangadex.rating_normalized, source: "mangadex" });
  }
  if (series.source?.manga_updates?.rating_normalized != null) {
    externalRatings.push({
      score: series.source.manga_updates.rating_normalized,
      source: "mangaupdates",
    });
  }
  if (series.source?.kitsu?.rating_normalized != null) {
    externalRatings.push({ score: series.source.kitsu.rating_normalized, source: "kitsu" });
  }
  if (series.source?.anime_planet?.rating_normalized != null) {
    externalRatings.push({
      score: series.source.anime_planet.rating_normalized,
      source: "animeplanet",
    });
  }

  // Get publisher name (pick first one if available)
  const publisher = series.publishers?.[0]?.name ?? undefined;

  return {
    externalId: String(series.id),
    externalUrl: `https://mangabaka.org/${series.id}`,
    title: series.title,
    alternateTitles,
    summary: series.description ?? undefined,
    status: mapStatus(series.status),
    year: series.year ?? undefined,
    // Extended metadata
    publisher,
    totalBookCount: series.final_volume ? Number.parseInt(series.final_volume, 10) : undefined,
    ageRating: mapContentRating(series.content_rating),
    readingDirection: inferReadingDirection(series.type, series.country_of_origin),
    // Taxonomy
    genres,
    tags: series.tags ?? [],
    authors,
    artists,
    coverUrl: coverUrl ?? undefined,
    rating: (() => {
      const r = extractRating(series.rating);
      return r != null ? { score: r, source: "mangabaka" } : undefined;
    })(),
    externalRatings: externalRatings.length > 0 ? externalRatings : undefined,
    externalLinks,
  };
}
