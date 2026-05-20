/**
 * Catalog of every leaf field the FilterBuilder can produce, per target.
 *
 * Each entry describes the operator family the field belongs to (Field,
 * Uuid, Bool, Number, Date) and, when applicable, the closed enum set the
 * user picks from. The builder uses this to render the right input control
 * and to validate that the emitted condition fits the backend grammar.
 */

export type OperatorType = "field" | "uuid" | "bool" | "number" | "date";

export type FieldTarget = "series" | "books";

export interface EnumOption {
  value: string;
  label: string;
}

export interface FieldDef {
  /** Condition key as serialized to the API (`name`, `format`, `dateAdded`, …). */
  key: string;
  label: string;
  group: string;
  operatorType: OperatorType;
  /** Targets that expose this field. */
  targets: FieldTarget[];
  /** Closed enum: only `is` / `isNot` / `isNull` / `isNotNull` make sense. */
  enumValues?: EnumOption[];
  /** Hint for the LeafEditor (e.g. textarea for free text, year for number). */
  hint?: "text" | "year" | "page-count" | "path";
}

const READ_STATUS: EnumOption[] = [
  { value: "unread", label: "Unread" },
  { value: "in_progress", label: "In Progress" },
  { value: "read", label: "Read" },
];

const SERIES_STATUS: EnumOption[] = [
  { value: "ongoing", label: "Ongoing" },
  { value: "ended", label: "Ended" },
  { value: "hiatus", label: "Hiatus" },
  { value: "abandoned", label: "Abandoned" },
  { value: "unknown", label: "Unknown" },
];

const BOOK_TYPES: EnumOption[] = [
  { value: "comic", label: "Comic" },
  { value: "manga", label: "Manga" },
  { value: "novel", label: "Novel" },
  { value: "magazine", label: "Magazine" },
  { value: "guide", label: "Guide" },
];

const BOOK_FORMATS: EnumOption[] = [
  { value: "cbz", label: "CBZ" },
  { value: "cbr", label: "CBR" },
  { value: "epub", label: "EPUB" },
  { value: "pdf", label: "PDF" },
];

export const FIELD_CATALOG: FieldDef[] = [
  // ----- Shared (series + books) -----
  {
    key: "libraryId",
    label: "Library",
    group: "Scope",
    operatorType: "uuid",
    targets: ["series", "books"],
  },
  {
    key: "genre",
    label: "Genre",
    group: "Metadata",
    operatorType: "field",
    targets: ["series", "books"],
  },
  {
    key: "tag",
    label: "Tag",
    group: "Metadata",
    operatorType: "field",
    targets: ["series", "books"],
  },
  {
    key: "readStatus",
    label: "Read status",
    group: "User",
    operatorType: "field",
    targets: ["series", "books"],
    enumValues: READ_STATUS,
  },
  {
    key: "dateAdded",
    label: "Date added",
    group: "User",
    operatorType: "date",
    targets: ["series", "books"],
  },

  // ----- Series-only -----
  {
    key: "name",
    label: "Series name",
    group: "Text",
    operatorType: "field",
    targets: ["series"],
    hint: "text",
  },
  {
    key: "titleSort",
    label: "Sort title",
    group: "Text",
    operatorType: "field",
    targets: ["series"],
    hint: "text",
  },
  {
    key: "author",
    label: "Author",
    group: "Text",
    operatorType: "field",
    targets: ["series"],
    hint: "text",
  },
  {
    key: "publisher",
    label: "Publisher",
    group: "Metadata",
    operatorType: "field",
    targets: ["series"],
  },
  {
    key: "language",
    label: "Language",
    group: "Metadata",
    operatorType: "field",
    targets: ["series"],
  },
  {
    key: "status",
    label: "Publication status",
    group: "Metadata",
    operatorType: "field",
    targets: ["series"],
    enumValues: SERIES_STATUS,
  },
  {
    key: "sharingTag",
    label: "Sharing tag",
    group: "Sharing",
    operatorType: "field",
    targets: ["series"],
  },
  {
    key: "year",
    label: "Year",
    group: "Metadata",
    operatorType: "number",
    targets: ["series"],
    hint: "year",
  },
  {
    key: "completion",
    label: "Marked complete",
    group: "User",
    operatorType: "bool",
    targets: ["series"],
  },
  {
    key: "hasExternalSourceId",
    label: "Has external source",
    group: "Metadata",
    operatorType: "bool",
    targets: ["series"],
  },
  {
    key: "hasUserRating",
    label: "Has user rating",
    group: "User",
    operatorType: "bool",
    targets: ["series"],
  },
  {
    key: "isTracked",
    label: "Tracked for releases",
    group: "User",
    operatorType: "bool",
    targets: ["series"],
  },

  // ----- Books-only -----
  {
    key: "seriesId",
    label: "Series",
    group: "Scope",
    operatorType: "uuid",
    targets: ["books"],
  },
  {
    key: "title",
    label: "Book title",
    group: "Text",
    operatorType: "field",
    targets: ["books"],
    hint: "text",
  },
  {
    key: "path",
    label: "File path",
    group: "Files",
    operatorType: "field",
    targets: ["books"],
    hint: "path",
  },
  {
    key: "format",
    label: "Format",
    group: "Files",
    operatorType: "field",
    targets: ["books"],
    enumValues: BOOK_FORMATS,
  },
  {
    key: "bookType",
    label: "Book type",
    group: "Metadata",
    operatorType: "field",
    targets: ["books"],
    enumValues: BOOK_TYPES,
  },
  {
    key: "pageCount",
    label: "Page count",
    group: "Files",
    operatorType: "number",
    targets: ["books"],
    hint: "page-count",
  },
  {
    key: "hasError",
    label: "Has error",
    group: "Files",
    operatorType: "bool",
    targets: ["books"],
  },
];

export function fieldsForTarget(target: FieldTarget): FieldDef[] {
  return FIELD_CATALOG.filter((f) => f.targets.includes(target));
}

export function findField(
  target: FieldTarget,
  key: string,
): FieldDef | undefined {
  return FIELD_CATALOG.find((f) => f.key === key && f.targets.includes(target));
}
