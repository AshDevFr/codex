import { Center, Text } from "@mantine/core";
import { type ReadingDirection, useReaderStore } from "@/store/readerStore";
import { ComicReader } from "./ComicReader";
import { EpubReader } from "./EpubReader";
import { PdfReader } from "./PdfReader";

/** Size threshold for defaulting to streaming mode (100MB) */
const PDF_STREAMING_THRESHOLD = 100 * 1024 * 1024;

interface ReaderRouterProps {
  /** Book ID */
  bookId: string;
  /** Series ID (for updating reading direction) */
  seriesId: string | null;
  /** Book title */
  title: string;
  /** Total number of pages */
  totalPages: number;
  /** Book format (CBZ, CBR, PDF, EPUB) */
  format: string;
  /** File size in bytes (for PDF mode selection) */
  fileSize?: number;
  /** Reading direction from series/library metadata */
  readingDirection?: string | null;
  /** Whether the book has been analyzed (page dimensions available) */
  analyzed?: boolean;
  /** Starting page from URL parameter (overrides saved progress) - for comics/PDFs */
  startPage?: number;
  /** Starting percentage from URL parameter (0.0-1.0) - for EPUBs */
  startPercent?: number;
  /** Incognito mode - when true, progress tracking is disabled */
  incognito?: boolean;
  /** Callback when reader should close */
  onClose: () => void;
}

/**
 * Router component that selects the appropriate reader based on book format.
 *
 * Routing logic:
 * - CBZ/CBR: ComicReader (image-based)
 * - PDF: ComicReader (streaming) or PdfReader (native) based on user preference
 * - EPUB: EpubReader
 */
export function ReaderRouter({
  bookId,
  seriesId,
  title,
  totalPages,
  format,
  fileSize,
  readingDirection,
  analyzed,
  startPage,
  startPercent,
  incognito,
  onClose,
}: ReaderRouterProps) {
  const normalizedFormat = format.toUpperCase();
  const pdfMode = useReaderStore((state) => state.settings.pdfMode);

  // Convert reading direction string to typed value
  const readingDirectionOverride: ReadingDirection | null =
    readingDirection === "rtl"
      ? "rtl"
      : readingDirection === "ltr"
        ? "ltr"
        : readingDirection === "ttb"
          ? "ttb"
          : readingDirection === "webtoon"
            ? "webtoon"
            : null;

  // Route to appropriate reader
  switch (normalizedFormat) {
    case "CBZ":
    case "CBR":
      // Use ComicReader for image-based formats
      return (
        <ComicReader
          bookId={bookId}
          seriesId={seriesId}
          title={title}
          totalPages={totalPages}
          format={normalizedFormat}
          readingDirectionOverride={readingDirectionOverride}
          analyzed={analyzed}
          startPage={startPage}
          incognito={incognito}
          onClose={onClose}
        />
      );

    case "PDF": {
      // Determine effective PDF mode:
      // - "streaming": Always use server-rendered images (ComicReader)
      // - "native": Always use pdf.js (PdfReader)
      // - "auto": Choose based on file size (>100MB → streaming, otherwise native)
      const effectivePdfMode =
        pdfMode === "streaming"
          ? "streaming"
          : pdfMode === "native"
            ? "native"
            : // Auto mode: large files use streaming for better performance
              fileSize && fileSize > PDF_STREAMING_THRESHOLD
              ? "streaming"
              : "native";

      if (effectivePdfMode === "native") {
        return (
          <PdfReader
            bookId={bookId}
            title={title}
            totalPages={totalPages}
            startPage={startPage}
            incognito={incognito}
            onClose={onClose}
          />
        );
      }

      // Streaming mode - use ComicReader
      return (
        <ComicReader
          bookId={bookId}
          seriesId={seriesId}
          title={title}
          totalPages={totalPages}
          format={normalizedFormat}
          readingDirectionOverride={readingDirectionOverride}
          analyzed={analyzed}
          startPage={startPage}
          incognito={incognito}
          onClose={onClose}
        />
      );
    }

    case "EPUB":
      return (
        <EpubReader
          bookId={bookId}
          seriesId={seriesId}
          title={title}
          totalPages={totalPages}
          startPercent={startPercent}
          incognito={incognito}
          onClose={onClose}
        />
      );

    default:
      return (
        <Center
          style={{ width: "100vw", height: "100vh", backgroundColor: "#000" }}
        >
          <Text c="dimmed">Unsupported format: {format}</Text>
        </Center>
      );
  }
}
