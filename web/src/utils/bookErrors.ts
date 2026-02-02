import type { BookErrorTypeDto } from "@/api/books";

/**
 * Human-readable labels for each error type
 */
export const ERROR_TYPE_LABELS: Record<BookErrorTypeDto, string> = {
  format_detection: "Format Detection",
  parser: "Parser Error",
  metadata: "Metadata Error",
  thumbnail: "Thumbnail Error",
  page_extraction: "Page Extraction",
  pdf_rendering: "PDF Rendering",
  zero_pages: "Zero Pages",
  other: "Other Error",
};

/**
 * Get human-readable label for an error type
 */
export function getErrorTypeLabel(errorType: BookErrorTypeDto): string {
  return ERROR_TYPE_LABELS[errorType] ?? errorType;
}

/**
 * Icons for each error type (Tabler icon names)
 */
export const ERROR_TYPE_ICONS: Record<BookErrorTypeDto, string> = {
  format_detection: "IconFileQuestion",
  parser: "IconFileAlert",
  metadata: "IconDatabase",
  thumbnail: "IconPhoto",
  page_extraction: "IconFileBroken",
  pdf_rendering: "IconPdf",
  zero_pages: "IconFileOff",
  other: "IconAlertCircle",
};

/**
 * Badge colors for each error type
 */
export const ERROR_TYPE_COLORS: Record<BookErrorTypeDto, string> = {
  format_detection: "grape",
  parser: "red",
  metadata: "orange",
  thumbnail: "blue",
  page_extraction: "pink",
  pdf_rendering: "violet",
  zero_pages: "yellow",
  other: "gray",
};

/**
 * Get badge color for an error type
 */
export function getErrorTypeColor(errorType: BookErrorTypeDto): string {
  return ERROR_TYPE_COLORS[errorType] ?? "gray";
}

/**
 * Descriptions for each error type (for tooltips/info)
 */
export const ERROR_TYPE_DESCRIPTIONS: Record<BookErrorTypeDto, string> = {
  format_detection:
    "The file format could not be identified. The file may be corrupted or in an unsupported format.",
  parser:
    "The file could not be parsed. The archive may be corrupted or contain invalid data.",
  metadata:
    "Metadata extraction failed. Some book information may be missing or incorrect.",
  thumbnail:
    "Thumbnail generation failed. The cover image could not be extracted or created.",
  page_extraction:
    "Pages could not be extracted from the archive. The file may be corrupted.",
  pdf_rendering:
    "PDF pages could not be rendered. This typically occurs with text-only PDFs when PDFium is not available.",
  zero_pages:
    "The book contains no readable pages. The file may be empty or contain only non-image content.",
  other: "An unexpected error occurred during book processing.",
};

/**
 * Get description for an error type
 */
export function getErrorTypeDescription(errorType: BookErrorTypeDto): string {
  return ERROR_TYPE_DESCRIPTIONS[errorType] ?? "Unknown error type.";
}

/**
 * All error types in display order
 */
export const ERROR_TYPES_ORDER: BookErrorTypeDto[] = [
  "parser",
  "format_detection",
  "page_extraction",
  "zero_pages",
  "thumbnail",
  "pdf_rendering",
  "metadata",
  "other",
];

/**
 * Check if an error type requires analysis retry (vs thumbnail retry)
 */
export function isAnalysisError(errorType: BookErrorTypeDto): boolean {
  return errorType !== "thumbnail";
}
