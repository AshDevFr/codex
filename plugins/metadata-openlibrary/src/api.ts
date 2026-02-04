/**
 * Open Library API Client
 *
 * Handles communication with the Open Library API with:
 * - Rate limiting (100 requests per 5 minutes recommended)
 * - Caching to reduce API calls
 * - Error handling with retries
 *
 * @see https://openlibrary.org/developers/api
 */

import type {
  OLAuthor,
  OLEdition,
  OLSearchResponse,
  OLWork,
  OLWorkEditionsResponse,
} from "./types.js";

const BASE_URL = "https://openlibrary.org";
const COVERS_BASE_URL = "https://covers.openlibrary.org";

// Simple in-memory cache with TTL
interface CacheEntry<T> {
  data: T;
  timestamp: number;
}

const CACHE_TTL_MS = 15 * 60 * 1000; // 15 minutes
const cache = new Map<string, CacheEntry<unknown>>();

/**
 * Get cached value if not expired
 */
function getCached<T>(key: string): T | null {
  const entry = cache.get(key);
  if (entry && Date.now() - entry.timestamp < CACHE_TTL_MS) {
    return entry.data as T;
  }
  if (entry) {
    cache.delete(key); // Cleanup expired
  }
  return null;
}

/**
 * Store value in cache
 */
function setCache<T>(key: string, data: T): void {
  cache.set(key, { data, timestamp: Date.now() });
}

/**
 * Make an HTTP request with error handling
 */
async function fetchJson<T>(url: string, description: string): Promise<T | null> {
  // Check cache first
  const cached = getCached<T>(url);
  if (cached !== null) {
    return cached;
  }

  try {
    const response = await fetch(url, {
      headers: {
        "User-Agent": "Codex/1.0 (https://github.com/AshDevFr/codex; codex-plugin)",
        Accept: "application/json",
      },
    });

    if (!response.ok) {
      if (response.status === 404) {
        return null;
      }
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const data = (await response.json()) as T;
    setCache(url, data);
    return data;
  } catch (error) {
    console.error(`[openlibrary] Failed to fetch ${description}:`, error);
    return null;
  }
}

/**
 * Normalize ISBN by removing hyphens and spaces
 */
export function normalizeIsbn(isbn: string): string {
  return isbn.replace(/[-\s]/g, "").toUpperCase();
}

/**
 * Check if a string is a valid ISBN-10 or ISBN-13
 */
export function isValidIsbn(isbn: string): boolean {
  const normalized = normalizeIsbn(isbn);
  return normalized.length === 10 || normalized.length === 13;
}

/**
 * Fetch book edition by ISBN
 *
 * @param isbn ISBN-10 or ISBN-13
 * @returns Edition data or null if not found
 */
export async function getEditionByIsbn(isbn: string): Promise<OLEdition | null> {
  const normalized = normalizeIsbn(isbn);
  const url = `${BASE_URL}/isbn/${normalized}.json`;
  return fetchJson<OLEdition>(url, `edition by ISBN ${normalized}`);
}

/**
 * Fetch work details by key
 *
 * @param workKey Work key (e.g., "/works/OL45883W" or just "OL45883W")
 * @returns Work data or null if not found
 */
export async function getWork(workKey: string): Promise<OLWork | null> {
  // Normalize key to just the ID part
  const key = workKey.startsWith("/works/") ? workKey : `/works/${workKey}`;
  const url = `${BASE_URL}${key}.json`;
  return fetchJson<OLWork>(url, `work ${key}`);
}

/**
 * Fetch editions for a work
 *
 * Returns editions directly associated with a work, ordered by most recent.
 * This is more reliable than searching by title, which can return unrelated books.
 *
 * @param workKey Work key (e.g., "/works/OL45883W")
 * @param limit Maximum number of editions to fetch
 * @returns Array of editions or empty array if none found
 */
export async function getWorkEditions(workKey: string, limit = 5): Promise<OLEdition[]> {
  const key = workKey.startsWith("/works/") ? workKey : `/works/${workKey}`;
  const url = `${BASE_URL}${key}/editions.json?limit=${limit}`;
  const response = await fetchJson<OLWorkEditionsResponse>(url, `editions for ${key}`);
  return response?.entries || [];
}

/**
 * Fetch author details by key
 *
 * @param authorKey Author key (e.g., "/authors/OL34184A" or just "OL34184A")
 * @returns Author data or null if not found
 */
export async function getAuthor(authorKey: string): Promise<OLAuthor | null> {
  // Normalize key to just the ID part
  const key = authorKey.startsWith("/authors/") ? authorKey : `/authors/${authorKey}`;
  const url = `${BASE_URL}${key}.json`;
  return fetchJson<OLAuthor>(url, `author ${key}`);
}

/** Fields to request from the Open Library search API */
const SEARCH_FIELDS = [
  "key",
  "title",
  "subtitle",
  "author_name",
  "author_key",
  "first_publish_year",
  "publish_year",
  "publisher",
  "isbn",
  "number_of_pages_median",
  "cover_i",
  "cover_edition_key",
  "edition_count",
  "language",
  "subject",
  "ratings_average",
  "ratings_count",
].join(",");

/**
 * Search for books
 *
 * When an author is provided, uses the `title` + `author` parameters for
 * more precise results. If that yields no results, falls back to a general
 * `q` search to ensure we still return something useful.
 *
 * @param query Search query (title, author, or combined)
 * @param options Additional search options
 * @returns Search results
 */
export async function searchBooks(
  query: string,
  options: {
    author?: string;
    limit?: number;
  } = {},
): Promise<OLSearchResponse | null> {
  const { author, limit = 10 } = options;

  // When author is provided, try a refined title + author search first
  if (author) {
    const params = new URLSearchParams({
      title: query,
      author,
      fields: SEARCH_FIELDS,
      limit: String(limit),
    });

    const url = `${BASE_URL}/search.json?${params}`;
    const response = await fetchJson<OLSearchResponse>(
      url,
      `search title="${query}" author="${author}"`,
    );

    if (response?.docs?.length) {
      return response;
    }

    // Fall back to general q search if title+author yielded no results
  }

  // General search using q parameter
  const params = new URLSearchParams({
    q: query,
    fields: SEARCH_FIELDS,
    limit: String(limit),
  });

  if (author) {
    params.set("author", author);
  }

  const url = `${BASE_URL}/search.json?${params}`;
  return fetchJson<OLSearchResponse>(url, `search "${query}"`);
}

/**
 * Get cover image URL by ISBN
 *
 * @param isbn ISBN-10 or ISBN-13
 * @param size Cover size: S (small ~50w), M (medium ~180w), L (large ~300w+)
 * @returns Cover URL
 */
export function getCoverUrlByIsbn(isbn: string, size: "S" | "M" | "L"): string {
  const normalized = normalizeIsbn(isbn);
  return `${COVERS_BASE_URL}/b/isbn/${normalized}-${size}.jpg`;
}

/**
 * Get cover image URL by cover ID
 *
 * @param coverId Open Library cover ID
 * @param size Cover size: S (small), M (medium), L (large)
 * @returns Cover URL
 */
export function getCoverUrlById(coverId: number, size: "S" | "M" | "L"): string {
  return `${COVERS_BASE_URL}/b/id/${coverId}-${size}.jpg`;
}

/**
 * Get cover image URL by Open Library ID (OLID)
 *
 * @param olid Open Library ID (e.g., "OL7353617M" for edition, "OL45883W" for work)
 * @param size Cover size: S (small), M (medium), L (large)
 * @returns Cover URL
 */
export function getCoverUrlByOlid(olid: string, size: "S" | "M" | "L"): string {
  // Strip any prefix if present
  const id = olid.replace(/^\/(?:books|works)\//, "");
  return `${COVERS_BASE_URL}/b/olid/${id}-${size}.jpg`;
}

/**
 * Parse year from Open Library date string
 *
 * Open Library dates can be in various formats:
 * - "2020"
 * - "January 1, 2020"
 * - "2020-01-15"
 * - "c1985"
 * - "1985?"
 *
 * @param dateStr Date string from Open Library
 * @returns Parsed year or undefined if unable to parse
 */
export function parseYear(dateStr: string | undefined): number | undefined {
  if (!dateStr) return undefined;

  // Try to extract a 4-digit year
  // Using (?:^|[^0-9]) to handle "c1985" format where there's no word boundary
  const match = dateStr.match(/(?:^|[^0-9])(1[89]\d{2}|20\d{2})(?:[^0-9]|$)/);
  if (match) {
    return Number.parseInt(match[1], 10);
  }

  return undefined;
}

/**
 * Parse description from Open Library
 *
 * Description can be either a string or an object with { type, value }.
 * Strips HTML tags and normalizes whitespace, since Open Library descriptions
 * can contain raw HTML (e.g., from Standard Ebooks imports).
 */
export function parseDescription(
  desc: string | { type?: string; value: string } | undefined,
): string | undefined {
  if (!desc) return undefined;
  const raw = typeof desc === "string" ? desc : desc.value;
  return stripHtml(raw);
}

/**
 * Strip HTML tags from a string and normalize whitespace.
 *
 * Converts block-level tags (p, br, div, li) to newlines,
 * strips all remaining tags, decodes common HTML entities,
 * and collapses excessive whitespace.
 */
function stripHtml(html: string): string | undefined {
  let text = html;

  // Convert block-level elements to newlines
  text = text.replace(/<\/(p|div|li|tr|h[1-6])>/gi, "\n");
  text = text.replace(/<br\s*\/?>/gi, "\n");

  // Remove all remaining HTML tags
  text = text.replace(/<[^>]+>/g, "");

  // Decode common HTML entities
  text = text
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&apos;/g, "'")
    .replace(/&nbsp;/g, " ");

  // Collapse multiple spaces/tabs on the same line into one space
  text = text.replace(/[^\S\n]+/g, " ");

  // Collapse 3+ consecutive newlines into 2
  text = text.replace(/\n{3,}/g, "\n\n");

  // Trim each line and remove leading/trailing whitespace
  text = text
    .split("\n")
    .map((line) => line.trim())
    .join("\n")
    .trim();

  return text || undefined;
}

/**
 * Convert Open Library language code to BCP47
 *
 * Open Library uses format like "/languages/eng"
 *
 * @param langRef Language reference (e.g., "/languages/eng")
 * @returns BCP47 language code (e.g., "en")
 */
export function parseLanguage(langRef: string | undefined): string | undefined {
  if (!langRef) return undefined;

  // Extract language code from "/languages/xxx" format
  const match = langRef.match(/\/languages\/(\w+)$/);
  if (!match) return undefined;

  const code = match[1].toLowerCase();

  // Map Open Library 3-letter codes to BCP47 2-letter codes
  const languageMap: Record<string, string> = {
    eng: "en",
    spa: "es",
    fre: "fr",
    fra: "fr",
    ger: "de",
    deu: "de",
    ita: "it",
    por: "pt",
    rus: "ru",
    jpn: "ja",
    chi: "zh",
    zho: "zh",
    kor: "ko",
    ara: "ar",
    hin: "hi",
    pol: "pl",
    tur: "tr",
    dut: "nl",
    nld: "nl",
    swe: "sv",
    nor: "no",
    dan: "da",
    fin: "fi",
    cze: "cs",
    ces: "cs",
    gre: "el",
    ell: "el",
    heb: "he",
    hun: "hu",
    rom: "ro",
    ron: "ro",
    tha: "th",
    vie: "vi",
    ind: "id",
    mal: "ms",
    msa: "ms",
    ukr: "uk",
    cat: "ca",
    lat: "la",
  };

  return languageMap[code] || code;
}

/**
 * Extract Open Library ID from a key
 *
 * @param key Full key (e.g., "/works/OL45883W" or "/books/OL7353617M")
 * @returns Just the ID (e.g., "OL45883W" or "OL7353617M")
 */
export function extractOlid(key: string): string {
  return key.replace(/^\/(?:works|books|authors)\//, "");
}

/**
 * Build Open Library URL from a key
 *
 * @param key Key (e.g., "/works/OL45883W")
 * @returns Full URL (e.g., "https://openlibrary.org/works/OL45883W")
 */
export function buildOpenLibraryUrl(key: string): string {
  return `${BASE_URL}${key.startsWith("/") ? key : `/${key}`}`;
}

/**
 * Clear the cache
 */
export function clearCache(): void {
  cache.clear();
}
