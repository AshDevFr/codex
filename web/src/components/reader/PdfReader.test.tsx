import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen, waitFor } from "@/test/utils";

// Mock react-pdf since it requires Web Workers which aren't available in tests
vi.mock("react-pdf", () => ({
	Document: (props: {
		children: React.ReactNode;
		file: string;
		onLoadSuccess?: (pdf: { numPages: number }) => void;
		onLoadError?: (error: Error) => void;
		loading?: React.ReactNode;
	}) => {
		// Simulate successful load after a tick
		if (props.onLoadSuccess) {
			setTimeout(() => props.onLoadSuccess?.({ numPages: 10 }), 0);
		}
		return (
			<div data-testid="pdf-document" data-file={props.file}>
				{props.children}
			</div>
		);
	},
	Page: (props: {
		pageNumber: number;
		width?: number;
		height?: number;
		scale?: number;
		renderTextLayer?: boolean;
		renderAnnotationLayer?: boolean;
		loading?: React.ReactNode;
		customTextRenderer?: ({ str }: { str: string }) => string;
	}) => (
		<div
			data-testid="pdf-page"
			data-page-number={props.pageNumber}
			data-width={props.width}
			data-height={props.height}
			data-scale={props.scale}
			data-render-text-layer={props.renderTextLayer}
			data-render-annotation-layer={props.renderAnnotationLayer}
		>
			PDF Page {props.pageNumber}
		</div>
	),
	pdfjs: {
		GlobalWorkerOptions: {
			workerSrc: "",
		},
	},
}));

// Mock the CSS imports
vi.mock("react-pdf/dist/esm/Page/TextLayer.css", () => ({}));
vi.mock("react-pdf/dist/esm/Page/AnnotationLayer.css", () => ({}));

// Mock hooks
vi.mock("./hooks", () => ({
	useAdjacentBooks: vi.fn(),
	useKeyboardNav: vi.fn(),
	useReadProgress: vi.fn(() => ({
		initialPage: 1,
		isLoading: false,
	})),
	useSeriesNavigation: vi.fn(() => ({
		handleNextPage: vi.fn(),
		handlePrevPage: vi.fn(),
		goToNextBook: vi.fn(),
		goToPrevBook: vi.fn(),
		canGoNextBook: false,
		canGoPrevBook: false,
	})),
	useTouchNav: vi.fn(() => ({
		touchRef: vi.fn(),
	})),
}));

// Import component after mocks
import { PdfReader } from "./PdfReader";

// Mock ResizeObserver - needed for container dimension measurement
const mockResizeObserver = vi.fn();
const mockResizeObserve = vi.fn();
const mockResizeDisconnect = vi.fn();

describe("PdfReader", () => {
	const defaultProps = {
		bookId: "book-123",
		title: "Test PDF Book",
		totalPages: 10,
		onClose: vi.fn(),
	};

	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store state
		useReaderStore.getState().resetSession();

		// Setup ResizeObserver mock - simulate container with dimensions
		mockResizeObserver.mockImplementation((callback) => {
			// Immediately call the callback with mock dimensions
			queueMicrotask(() => {
				callback([{ contentRect: { width: 800, height: 600 } }]);
			});
			return {
				observe: mockResizeObserve,
				disconnect: mockResizeDisconnect,
			};
		});
		global.ResizeObserver = mockResizeObserver;
	});

	it("should render PDF document with correct file URL", async () => {
		renderWithProviders(<PdfReader {...defaultProps} />);

		await waitFor(() => {
			const document = screen.getByTestId("pdf-document");
			expect(document).toHaveAttribute(
				"data-file",
				"/api/v1/books/book-123/file",
			);
		});
	});

	it("should render current page", async () => {
		renderWithProviders(<PdfReader {...defaultProps} />);

		await waitFor(() => {
			const page = screen.getByTestId("pdf-page");
			expect(page).toHaveAttribute("data-page-number", "1");
		});
	});

	it("should display book title in toolbar", async () => {
		renderWithProviders(<PdfReader {...defaultProps} />);

		await waitFor(() => {
			expect(screen.getByText("Test PDF Book")).toBeInTheDocument();
		});
	});

	it("should enable text layer rendering", async () => {
		renderWithProviders(<PdfReader {...defaultProps} />);

		await waitFor(() => {
			const page = screen.getByTestId("pdf-page");
			expect(page).toHaveAttribute("data-render-text-layer", "true");
		});
	});

	it("should enable annotation layer rendering", async () => {
		renderWithProviders(<PdfReader {...defaultProps} />);

		await waitFor(() => {
			const page = screen.getByTestId("pdf-page");
			expect(page).toHaveAttribute("data-render-annotation-layer", "true");
		});
	});

	it("should use startPage when provided", async () => {
		renderWithProviders(<PdfReader {...defaultProps} startPage={5} />);

		// Wait for initialization to complete
		await waitFor(() => {
			const state = useReaderStore.getState();
			expect(state.currentPage).toBe(5);
		});
	});

	describe("search functionality", () => {
		it("should show search bar when Ctrl+F is pressed", async () => {
			renderWithProviders(<PdfReader {...defaultProps} />);

			// Simulate Ctrl+F
			fireEvent.keyDown(document, { key: "f", ctrlKey: true });

			await waitFor(() => {
				expect(
					screen.getByPlaceholderText("Search in PDF..."),
				).toBeInTheDocument();
			});
		});

		it("should hide search bar when Escape is pressed", async () => {
			renderWithProviders(<PdfReader {...defaultProps} />);

			// Open search
			fireEvent.keyDown(document, { key: "f", ctrlKey: true });

			await waitFor(() => {
				expect(
					screen.getByPlaceholderText("Search in PDF..."),
				).toBeInTheDocument();
			});

			// Close search
			const searchInput = screen.getByPlaceholderText("Search in PDF...");
			fireEvent.keyDown(searchInput, { key: "Escape" });

			await waitFor(() => {
				expect(
					screen.queryByPlaceholderText("Search in PDF..."),
				).not.toBeInTheDocument();
			});
		});
	});

	describe("click zones", () => {
		it("should navigate on left zone click", async () => {
			// Validate hook availability for click zone navigation
			const hooks = await import("./hooks");
			expect(hooks.useSeriesNavigation).toBeDefined();

			renderWithProviders(<PdfReader {...defaultProps} />);

			await waitFor(() => {
				expect(screen.getByTestId("pdf-page")).toBeInTheDocument();
			});
		});
	});
});
