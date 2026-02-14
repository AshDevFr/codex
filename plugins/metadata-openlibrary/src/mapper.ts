/**
 * Mapper functions to convert Open Library data to Codex plugin format
 */

import type {
  BookAuthor,
  BookCover,
  ExternalId,
  ExternalLink,
  ExternalRating,
  PluginBookMetadata,
  SearchResult,
} from "@ashdev/codex-plugin-sdk";

import {
  buildOpenLibraryUrl,
  getAuthor,
  getCoverUrlById,
  getCoverUrlByIsbn,
  getWork,
  getWorkEditions,
  parseDescription,
  parseLanguage,
  parseYear,
} from "./api.js";
import type { OLAuthorReference, OLEdition, OLSearchDoc, OLWork, ParsedAuthor } from "./types.js";

/**
 * Map Open Library search result to Codex SearchResult
 */
export function mapSearchDocToSearchResult(doc: OLSearchDoc): SearchResult {
  const year = doc.first_publish_year;
  const coverUrl = doc.cover_i ? getCoverUrlById(doc.cover_i, "M") : undefined;

  // Calculate a relevance score based on available data
  // More complete entries get higher scores
  let relevanceScore = 0.5;
  if (doc.author_name?.length) relevanceScore += 0.1;
  if (doc.isbn?.length) relevanceScore += 0.15;
  if (doc.cover_i) relevanceScore += 0.1;
  if (doc.first_publish_year) relevanceScore += 0.05;
  if (doc.subject?.length) relevanceScore += 0.05;
  if (doc.ratings_count && doc.ratings_count > 0) relevanceScore += 0.05;

  return {
    externalId: doc.key, // Work key, e.g., "/works/OL45883W"
    title: doc.title,
    alternateTitles: doc.subtitle ? [doc.subtitle] : [],
    year,
    coverUrl,
    relevanceScore: Math.min(1.0, relevanceScore),
    preview: {
      genres: doc.subject?.slice(0, 5) || [],
      rating: doc.ratings_average
        ? Math.round(doc.ratings_average * 2) / 2 // Normalize to 0-10 scale (OL uses 1-5)
        : undefined,
      authors: doc.author_name?.slice(0, 3) || [],
      description: doc.publisher?.length ? `Published by ${doc.publisher[0]}` : undefined,
    },
  };
}

/**
 * Resolve author references to full author data
 */
async function resolveAuthors(
  authorRefs: OLAuthorReference[] | undefined,
): Promise<ParsedAuthor[]> {
  if (!authorRefs?.length) return [];

  const authors: ParsedAuthor[] = [];

  for (const ref of authorRefs) {
    const key = ref.author?.key || ref.key;
    if (!key) continue;

    const authorData = await getAuthor(key);
    if (authorData) {
      authors.push({
        name: authorData.name,
        key,
        sortName: authorData.personal_name || undefined,
      });
    }
  }

  return authors;
}

/**
 * Map parsed authors to BookAuthor format
 */
function mapToBookAuthors(authors: ParsedAuthor[]): BookAuthor[] {
  return authors.map((author) => ({
    name: author.name,
    role: "author" as const,
    sortName: author.sortName,
  }));
}

/**
 * Build cover URLs for a book
 */
function buildCoverUrls(isbn: string | undefined, coverId: number | undefined): BookCover[] {
  const covers: BookCover[] = [];

  // Prefer ISBN-based URLs as they're more reliable
  if (isbn) {
    covers.push({
      url: getCoverUrlByIsbn(isbn, "S"),
      size: "small",
    });
    covers.push({
      url: getCoverUrlByIsbn(isbn, "M"),
      size: "medium",
    });
    covers.push({
      url: getCoverUrlByIsbn(isbn, "L"),
      size: "large",
    });
  } else if (coverId) {
    // Fallback to cover ID
    covers.push({
      url: getCoverUrlById(coverId, "S"),
      size: "small",
    });
    covers.push({
      url: getCoverUrlById(coverId, "M"),
      size: "medium",
    });
    covers.push({
      url: getCoverUrlById(coverId, "L"),
      size: "large",
    });
  }

  return covers;
}

/**
 * Build external IDs (cross-references) for Open Library book
 */
function buildExternalIds(externalId: string): ExternalId[] {
  return [{ source: "api:openlibrary", externalId }];
}

/**
 * Build external links for Open Library book
 */
function buildExternalLinks(editionKey: string, workKey: string | undefined): ExternalLink[] {
  const links: ExternalLink[] = [
    {
      url: buildOpenLibraryUrl(editionKey),
      label: "Open Library (Edition)",
      linkType: "provider",
    },
  ];

  if (workKey) {
    links.push({
      url: buildOpenLibraryUrl(workKey),
      label: "Open Library (Work)",
      linkType: "provider",
    });
  }

  return links;
}

/**
 * Get all ISBNs from edition data
 */
function collectIsbns(edition: OLEdition): string[] {
  const isbns: string[] = [];

  // Prefer ISBN-13
  if (edition.isbn_13?.length) {
    isbns.push(...edition.isbn_13);
  }

  // Add ISBN-10 as well
  if (edition.isbn_10?.length) {
    isbns.push(...edition.isbn_10);
  }

  return [...new Set(isbns)]; // Deduplicate
}

/**
 * Map Open Library edition and optional work to full book metadata
 */
export async function mapEditionToBookMetadata(
  edition: OLEdition,
  workData?: OLWork | null,
): Promise<PluginBookMetadata> {
  // Resolve authors from edition or work
  const authorRefs = edition.authors || workData?.authors;
  const authors = await resolveAuthors(authorRefs);

  // Get ISBNs
  const isbns = collectIsbns(edition);
  const primaryIsbn = isbns[0];

  // Get cover ID from edition or work
  const coverId = edition.covers?.[0] || workData?.covers?.[0];

  // Get description from edition or work
  const description =
    parseDescription(edition.description) || parseDescription(workData?.description);

  // Get subjects from both edition and work
  const subjects = [...(edition.subjects || []), ...(workData?.subjects || [])];
  const uniqueSubjects = [...new Set(subjects)];

  // Parse year
  const year = parseYear(edition.publish_date);
  const originalYear = parseYear(workData?.first_publish_date);

  // Parse language
  const language = parseLanguage(edition.languages?.[0]?.key);

  // Build external rating if ratings exist from search
  const externalRatings: ExternalRating[] = [];

  // Build metadata
  const workKey = edition.works?.[0]?.key || workData?.key;
  const externalId = workKey || edition.key;

  return {
    externalId,
    externalUrl: buildOpenLibraryUrl(externalId),

    // Core fields
    title: edition.title,
    subtitle: edition.subtitle || workData?.subtitle,
    alternateTitles: [],
    summary: description,
    bookType: detectBookType(edition),

    // Book-specific fields
    pageCount: edition.number_of_pages,
    year,

    // ISBN
    isbn: primaryIsbn,
    isbns,

    // Edition info
    edition: edition.edition_name,
    originalTitle: workData?.title !== edition.title ? workData?.title : undefined,
    originalYear,
    language,

    // Taxonomy
    genres: [], // Open Library doesn't have genres, just subjects
    tags: [],
    subjects: uniqueSubjects.slice(0, 20), // Limit to 20 subjects

    // Credits
    authors: mapToBookAuthors(authors),
    artists: [], // Open Library doesn't track artists separately
    publisher: edition.publishers?.[0],

    // Media
    coverUrl: primaryIsbn
      ? getCoverUrlByIsbn(primaryIsbn, "L")
      : coverId
        ? getCoverUrlById(coverId, "L")
        : undefined,
    covers: buildCoverUrls(primaryIsbn, coverId),

    // Rating
    externalRatings,
    awards: [],

    // Links
    externalLinks: buildExternalLinks(edition.key, workKey),

    // Cross-reference IDs
    externalIds: buildExternalIds(externalId),
  };
}

/**
 * Detect book type from edition data
 *
 * Open Library doesn't have explicit book type, but we can infer from:
 * - physical_format field
 * - subjects
 * - other metadata
 */
function detectBookType(edition: OLEdition): string | undefined {
  const format = edition.physical_format?.toLowerCase();

  if (format) {
    if (format.includes("comic") || format.includes("graphic novel")) {
      return "graphic_novel";
    }
    if (format.includes("manga")) {
      return "manga";
    }
    if (format.includes("magazine") || format.includes("periodical")) {
      return "magazine";
    }
  }

  // Check subjects for hints
  const subjects = (edition.subjects || []).join(" ").toLowerCase();

  if (subjects.includes("graphic novel") || subjects.includes("comics")) {
    return "graphic_novel";
  }
  if (subjects.includes("manga")) {
    return "manga";
  }

  // Default to novel for most books
  return "novel";
}

/**
 * Map Open Library search doc to book metadata for quick preview
 *
 * This is a lighter version that doesn't fetch additional data
 */
export function mapSearchDocToBookPreview(doc: OLSearchDoc): PluginBookMetadata {
  const isbns = doc.isbn?.slice(0, 5) || [];
  const primaryIsbn = isbns[0];
  const coverId = doc.cover_i;

  return {
    externalId: doc.key,
    externalUrl: buildOpenLibraryUrl(doc.key),

    // Core fields
    title: doc.title,
    subtitle: doc.subtitle,
    alternateTitles: [],
    summary: undefined, // Not available in search results

    // Book-specific fields
    pageCount: doc.number_of_pages_median,
    year: doc.first_publish_year,

    // ISBN
    isbn: primaryIsbn,
    isbns,

    // Taxonomy
    genres: [],
    tags: [],
    subjects: doc.subject?.slice(0, 10) || [],

    // Credits
    authors:
      doc.author_name?.map((name) => ({
        name,
        role: "author" as const,
      })) || [],
    artists: [],
    publisher: doc.publisher?.[0],

    // Media
    coverUrl: primaryIsbn
      ? getCoverUrlByIsbn(primaryIsbn, "L")
      : coverId
        ? getCoverUrlById(coverId, "L")
        : undefined,
    covers: buildCoverUrls(primaryIsbn, coverId),

    // Rating
    rating: doc.ratings_average
      ? {
          score: Math.round(doc.ratings_average * 20), // Convert 1-5 to 0-100
          voteCount: doc.ratings_count,
          source: "openlibrary",
        }
      : undefined,
    externalRatings:
      doc.ratings_average && doc.ratings_count
        ? [
            {
              score: Math.round(doc.ratings_average * 20),
              voteCount: doc.ratings_count,
              source: "openlibrary",
            },
          ]
        : [],
    awards: [],

    // Links
    externalLinks: [
      {
        url: buildOpenLibraryUrl(doc.key),
        label: "Open Library",
        linkType: "provider",
      },
    ],

    // Cross-reference IDs
    externalIds: buildExternalIds(doc.key),
  };
}

/**
 * Get full book metadata by fetching edition, work, and author data
 *
 * @param editionOrWorkKey Either an edition key or work key
 * @param isbn Optional ISBN for direct lookup
 */
export async function getFullBookMetadata(
  editionOrWorkKey: string,
  isbn?: string,
): Promise<PluginBookMetadata | null> {
  // If we have an ISBN, try to get edition directly
  if (isbn) {
    const { getEditionByIsbn } = await import("./api.js");
    const edition = await getEditionByIsbn(isbn);
    if (edition) {
      const workKey = edition.works?.[0]?.key;
      const workData = workKey ? await getWork(workKey) : null;
      return mapEditionToBookMetadata(edition, workData);
    }
  }

  // Check if it's a work key
  if (editionOrWorkKey.includes("/works/")) {
    const workData = await getWork(editionOrWorkKey);
    if (!workData) return null;

    // Fetch editions directly from the work using the editions API.
    // This is much more reliable than searching by title, which can
    // return completely unrelated books with similar titles.
    const editions = await getWorkEditions(editionOrWorkKey, 5);

    if (editions.length > 0) {
      // Prefer an edition that has ISBNs for richer metadata
      const editionWithIsbn = editions.find((e) => e.isbn_13?.length || e.isbn_10?.length);
      const edition = editionWithIsbn || editions[0];
      return mapEditionToBookMetadata(edition, workData);
    }

    // Fallback: create metadata from work data only
    const authors = await resolveAuthors(workData.authors);
    const coverId = workData.covers?.[0];

    return {
      externalId: workData.key,
      externalUrl: buildOpenLibraryUrl(workData.key),
      title: workData.title,
      subtitle: workData.subtitle,
      alternateTitles: [],
      summary: parseDescription(workData.description),
      isbns: [],
      genres: [],
      tags: [],
      subjects: workData.subjects?.slice(0, 20) || [],
      authors: mapToBookAuthors(authors),
      artists: [],
      coverUrl: coverId ? getCoverUrlById(coverId, "L") : undefined,
      covers: coverId
        ? [
            { url: getCoverUrlById(coverId, "S"), size: "small" },
            { url: getCoverUrlById(coverId, "M"), size: "medium" },
            { url: getCoverUrlById(coverId, "L"), size: "large" },
          ]
        : [],
      externalRatings: [],
      awards: [],
      externalLinks: [
        {
          url: buildOpenLibraryUrl(workData.key),
          label: "Open Library",
          linkType: "provider",
        },
      ],
      externalIds: buildExternalIds(workData.key),
    };
  }

  // It's an edition key - fetch directly
  // For edition keys, we need to use a different approach
  // since there's no direct edition endpoint by key
  // Try to use the key directly
  const url = `https://openlibrary.org${editionOrWorkKey}.json`;
  try {
    const response = await fetch(url, {
      headers: {
        "User-Agent": "Codex/1.0 (https://github.com/AshDevFr/codex; codex-plugin)",
        Accept: "application/json",
      },
    });

    if (response.ok) {
      const edition = (await response.json()) as OLEdition;
      const workKey = edition.works?.[0]?.key;
      const workData = workKey ? await getWork(workKey) : null;
      return mapEditionToBookMetadata(edition, workData);
    }
  } catch {
    // Ignore fetch errors
  }

  return null;
}
