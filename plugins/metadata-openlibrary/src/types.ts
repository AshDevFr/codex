/**
 * Types for Open Library API responses
 *
 * @see https://openlibrary.org/developers/api
 */

/**
 * Open Library Edition (book) response from /isbn/{isbn}.json
 */
export interface OLEdition {
  key: string; // e.g., "/books/OL7353617M"
  title: string;
  subtitle?: string;
  authors?: OLAuthorReference[];
  publishers?: string[];
  publish_date?: string;
  publish_places?: string[];
  number_of_pages?: number;
  pagination?: string;
  isbn_10?: string[];
  isbn_13?: string[];
  identifiers?: Record<string, string[]>;
  covers?: number[]; // Cover IDs
  works?: OLWorkReference[];
  description?: string | OLTextValue;
  subjects?: string[];
  subject_places?: string[];
  subject_people?: string[];
  subject_times?: string[];
  languages?: OLLanguageReference[];
  edition_name?: string;
  series?: string[];
  physical_format?: string;
  physical_dimensions?: string;
  weight?: string;
  notes?: string | OLTextValue;
  contributions?: string[];
  by_statement?: string;
  first_sentence?: OLTextValue;
  lc_classifications?: string[];
  dewey_decimal_class?: string[];
  ocaid?: string; // Internet Archive ID
  oclc_numbers?: string[];
  lccn?: string[];
}

/**
 * Open Library Work response from /works/{id}.json
 */
export interface OLWork {
  key: string; // e.g., "/works/OL45883W"
  title: string;
  subtitle?: string;
  authors?: OLAuthorReference[];
  description?: string | OLTextValue;
  subjects?: string[];
  subject_places?: string[];
  subject_people?: string[];
  subject_times?: string[];
  covers?: number[];
  first_publish_date?: string;
  links?: OLLink[];
  excerpts?: OLExcerpt[];
}

/**
 * Open Library Author response from /authors/{id}.json
 */
export interface OLAuthor {
  key: string; // e.g., "/authors/OL34184A"
  name: string;
  personal_name?: string;
  alternate_names?: string[];
  bio?: string | OLTextValue;
  birth_date?: string;
  death_date?: string;
  photos?: number[];
  links?: OLLink[];
  wikipedia?: string;
}

/**
 * Reference to an author in edition/work
 */
export interface OLAuthorReference {
  author?: { key: string };
  key?: string; // Direct key reference
}

/**
 * Reference to a work
 */
export interface OLWorkReference {
  key: string;
}

/**
 * Reference to a language
 */
export interface OLLanguageReference {
  key: string; // e.g., "/languages/eng"
}

/**
 * Text value with type (description, bio, etc.)
 */
export interface OLTextValue {
  type: string;
  value: string;
}

/**
 * Link to external resource
 */
export interface OLLink {
  url: string;
  title: string;
  type?: { key: string };
}

/**
 * Book excerpt
 */
export interface OLExcerpt {
  excerpt: string;
  comment?: string;
  author?: { key: string };
}

/**
 * Open Library Work Editions response from /works/{id}/editions.json
 */
export interface OLWorkEditionsResponse {
  links: { self: string; work: string; next?: string };
  size: number;
  entries: OLEdition[];
}

/**
 * Open Library Search response from /search.json
 */
export interface OLSearchResponse {
  numFound: number;
  start: number;
  numFoundExact: boolean;
  docs: OLSearchDoc[];
}

/**
 * Individual search result document
 */
export interface OLSearchDoc {
  key: string; // Work key, e.g., "/works/OL45883W"
  title: string;
  subtitle?: string;
  author_name?: string[];
  author_key?: string[];
  first_publish_year?: number;
  publish_year?: number[];
  publisher?: string[];
  isbn?: string[];
  number_of_pages_median?: number;
  cover_i?: number; // Cover ID
  cover_edition_key?: string;
  edition_key?: string[];
  edition_count?: number;
  language?: string[];
  subject?: string[];
  subject_key?: string[];
  subject_facet?: string[];
  place?: string[];
  person?: string[];
  time?: string[];
  ratings_average?: number;
  ratings_count?: number;
  want_to_read_count?: number;
  currently_reading_count?: number;
  already_read_count?: number;
}

/**
 * Parsed and normalized author information
 */
export interface ParsedAuthor {
  name: string;
  key?: string;
  sortName?: string;
}
