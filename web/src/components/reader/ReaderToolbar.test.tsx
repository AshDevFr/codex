import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen } from "@/test/utils";
import { ReaderToolbar } from "./ReaderToolbar";

describe("ReaderToolbar", () => {
	const defaultProps = {
		title: "Test Book",
		visible: true,
		onClose: vi.fn(),
		onOpenSettings: vi.fn(),
	};

	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store to default state
		useReaderStore.setState({
			settings: {
				fitMode: "screen",
				pageLayout: "single",
				readingDirection: "ltr",
				backgroundColor: "black",
				pdfMode: "streaming",
				pdfSpreadMode: "single",
				pdfContinuousScroll: false,
				autoHideToolbar: true,
				toolbarHideDelay: 3000,
			},
			currentPage: 5,
			totalPages: 10,
			isLoading: false,
			toolbarVisible: true,
			isFullscreen: false,
			currentBookId: "book-123",
			readingDirectionOverride: null,
		});
	});

	it("should render book title", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		expect(screen.getByText("Test Book")).toBeInTheDocument();
	});

	it("should display current page and total pages", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		expect(screen.getByText("5 / 10")).toBeInTheDocument();
	});

	it("should call onClose when close button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		// Close button is the first button (X icon)
		const buttons = screen.getAllByRole("button");
		fireEvent.click(buttons[0]); // First button is close

		expect(defaultProps.onClose).toHaveBeenCalledTimes(1);
	});

	it("should call onOpenSettings when settings button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		// Settings button is the last button
		const buttons = screen.getAllByRole("button");
		fireEvent.click(buttons[buttons.length - 1]); // Last button is settings

		expect(defaultProps.onOpenSettings).toHaveBeenCalledTimes(1);
	});

	it("should navigate to next page when forward button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		// Navigation buttons are in the center group
		const buttons = screen.getAllByRole("button");
		// In LTR: [close, prev, next, fit, fullscreen, settings]
		// Index 2 is the forward button
		fireEvent.click(buttons[2]);

		expect(useReaderStore.getState().currentPage).toBe(6);
	});

	it("should navigate to previous page when backward button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		const buttons = screen.getAllByRole("button");
		// Index 1 is the backward button
		fireEvent.click(buttons[1]);

		expect(useReaderStore.getState().currentPage).toBe(4);
	});

	it("should disable backward button on first page in LTR", () => {
		useReaderStore.setState({ currentPage: 1 });
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		const buttons = screen.getAllByRole("button");
		// In LTR mode, first page means backward (prev) is disabled
		expect(buttons[1]).toBeDisabled();
	});

	it("should disable forward button on last page in LTR", () => {
		useReaderStore.setState({ currentPage: 10 });
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		const buttons = screen.getAllByRole("button");
		// In LTR mode, last page means forward (next) is disabled
		expect(buttons[2]).toBeDisabled();
	});

	it("should toggle fullscreen when fullscreen button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		const buttons = screen.getAllByRole("button");
		// Fullscreen button is index 4
		fireEvent.click(buttons[4]);

		expect(useReaderStore.getState().isFullscreen).toBe(true);
	});

	it("should cycle fit mode when fit mode button is clicked", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		const buttons = screen.getAllByRole("button");
		// Fit mode button is index 3
		fireEvent.click(buttons[3]);

		expect(useReaderStore.getState().settings.fitMode).toBe("width");
	});

	it("should not render when visible is false", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} visible={false} />);

		// The toolbar should be hidden via Transition
		expect(screen.queryByText("Test Book")).not.toBeInTheDocument();
	});

	it("should display progress percentage", () => {
		renderWithProviders(<ReaderToolbar {...defaultProps} />);

		// 5/10 = 50%
		expect(screen.getByText("50%")).toBeInTheDocument();
	});

	describe("RTL reading direction", () => {
		beforeEach(() => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					readingDirection: "rtl",
				},
			});
		});

		it("should render with RTL direction buttons", () => {
			renderWithProviders(<ReaderToolbar {...defaultProps} />);

			// In RTL mode, the buttons should still be rendered
			const buttons = screen.getAllByRole("button");
			expect(buttons.length).toBeGreaterThan(0);
		});

		it("should navigate correctly in RTL mode", () => {
			renderWithProviders(<ReaderToolbar {...defaultProps} />);

			const buttons = screen.getAllByRole("button");

			// In RTL mode, the backward button (index 1) should call nextPage
			fireEvent.click(buttons[1]);
			expect(useReaderStore.getState().currentPage).toBe(6);

			// Reset page
			useReaderStore.setState({ currentPage: 5 });

			// In RTL mode, the forward button (index 2) should call prevPage
			fireEvent.click(buttons[2]);
			expect(useReaderStore.getState().currentPage).toBe(4);
		});
	});
});
