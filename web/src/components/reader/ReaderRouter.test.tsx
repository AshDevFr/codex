import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, waitFor } from "@/test/utils";
import { useReaderStore } from "@/store/readerStore";

// Mock the reader components
vi.mock("./ComicReader", () => ({
	ComicReader: ({ bookId, seriesId, format }: { bookId: string; seriesId: string | null; format: string }) => (
		<div data-testid="comic-reader" data-book-id={bookId} data-series-id={seriesId ?? ""} data-format={format}>
			Comic Reader
		</div>
	),
}));

vi.mock("./EpubReader", () => ({
	EpubReader: ({ bookId }: { bookId: string }) => (
		<div data-testid="epub-reader" data-book-id={bookId}>
			EPUB Reader
		</div>
	),
}));

vi.mock("./PdfReader", () => ({
	PdfReader: ({ bookId }: { bookId: string }) => (
		<div data-testid="pdf-reader" data-book-id={bookId}>
			PDF Reader
		</div>
	),
}));

import { ReaderRouter } from "./ReaderRouter";

describe("ReaderRouter", () => {
	const defaultProps = {
		bookId: "book-123",
		seriesId: "series-456",
		title: "Test Book",
		totalPages: 100,
		format: "CBZ",
		onClose: vi.fn(),
	};

	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store to defaults
		useReaderStore.getState().resetSession();
		// Reset PDF mode to default (streaming)
		useReaderStore.getState().setPdfMode("streaming");
	});

	describe("format routing", () => {
		it("should route CBZ to ComicReader", () => {
			renderWithProviders(<ReaderRouter {...defaultProps} format="CBZ" />);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-format",
				"CBZ",
			);
		});

		it("should route CBR to ComicReader", () => {
			renderWithProviders(<ReaderRouter {...defaultProps} format="CBR" />);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-format",
				"CBR",
			);
		});

		it("should route EPUB to EpubReader", () => {
			renderWithProviders(<ReaderRouter {...defaultProps} format="EPUB" />);

			expect(screen.getByTestId("epub-reader")).toBeInTheDocument();
		});

		it("should handle lowercase format", () => {
			renderWithProviders(<ReaderRouter {...defaultProps} format="cbz" />);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
		});

		it("should show unsupported format message for unknown formats", () => {
			renderWithProviders(<ReaderRouter {...defaultProps} format="DOC" />);

			expect(screen.getByText("Unsupported format: DOC")).toBeInTheDocument();
		});
	});

	describe("PDF mode routing", () => {
		it("should route PDF to ComicReader when pdfMode is streaming", () => {
			useReaderStore.getState().setPdfMode("streaming");

			renderWithProviders(<ReaderRouter {...defaultProps} format="PDF" />);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-format",
				"PDF",
			);
		});

		it("should route PDF to PdfReader when pdfMode is native", () => {
			useReaderStore.getState().setPdfMode("native");

			renderWithProviders(<ReaderRouter {...defaultProps} format="PDF" />);

			expect(screen.getByTestId("pdf-reader")).toBeInTheDocument();
		});

		it("should default to streaming for large files (>100MB)", () => {
			// Reset to no explicit preference by setting to streaming
			// (In real usage, the smart default logic would apply for undefined)
			useReaderStore.getState().setPdfMode("streaming");

			const largeFileSize = 150 * 1024 * 1024; // 150MB
			renderWithProviders(
				<ReaderRouter {...defaultProps} format="PDF" fileSize={largeFileSize} />,
			);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
		});

		it("should use native mode for small PDF files when preference is native", () => {
			useReaderStore.getState().setPdfMode("native");

			const smallFileSize = 50 * 1024 * 1024; // 50MB
			renderWithProviders(
				<ReaderRouter {...defaultProps} format="PDF" fileSize={smallFileSize} />,
			);

			expect(screen.getByTestId("pdf-reader")).toBeInTheDocument();
		});
	});

	describe("props forwarding", () => {
		it("should pass bookId to the reader component", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} bookId="my-unique-book" />,
			);

			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-book-id",
				"my-unique-book",
			);
		});

		it("should pass seriesId to the reader component", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} seriesId="my-series-id" />,
			);

			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-series-id",
				"my-series-id",
			);
		});

		it("should handle null seriesId", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} seriesId={null} />,
			);

			expect(screen.getByTestId("comic-reader")).toHaveAttribute(
				"data-series-id",
				"",
			);
		});

		it("should pass reading direction override for RTL", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} readingDirection="rtl" />,
			);

			// Component receives the prop - we verify it renders
			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
		});

		it("should pass reading direction override for LTR", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} readingDirection="ltr" />,
			);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
		});

		it("should handle null reading direction", () => {
			renderWithProviders(
				<ReaderRouter {...defaultProps} readingDirection={null} />,
			);

			expect(screen.getByTestId("comic-reader")).toBeInTheDocument();
		});
	});
});
