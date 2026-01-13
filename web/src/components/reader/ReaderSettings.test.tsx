import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen } from "@/test/utils";
import { ReaderSettings } from "./ReaderSettings";

// Mock the API client
vi.mock("@/api/client", () => ({
	api: {
		get: vi.fn(),
		put: vi.fn(),
		post: vi.fn(),
		patch: vi.fn(),
		delete: vi.fn(),
	},
}));

describe("ReaderSettings", () => {
	const defaultProps = {
		opened: true,
		onClose: vi.fn(),
	};

	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store to default state (LTR = paginated mode)
		useReaderStore.setState({
			settings: {
				fitMode: "screen",
				pageLayout: "single",
				readingDirection: "ltr",
				backgroundColor: "black",
				pdfMode: "streaming",
				autoHideToolbar: true,
				toolbarHideDelay: 3000,
				epubTheme: "light",
				epubFontSize: 100,
				preloadPages: 1,
				doublePageShowWideAlone: true,
				doublePageStartOnOdd: true,
				pageTransition: "slide",
				transitionDuration: 200,
				webtoonSidePadding: 0,
				webtoonPageGap: 0,
			},
			currentPage: 1,
			totalPages: 10,
			isLoading: false,
			toolbarVisible: true,
			isFullscreen: false,
			currentBookId: "book-123",
			readingDirectionOverride: null,
			adjacentBooks: null,
			boundaryState: "none",
			pageOrientations: {},
			lastNavigationDirection: null,
		});
	});

	it("should render the modal when opened", () => {
		renderWithProviders(<ReaderSettings {...defaultProps} />);

		expect(screen.getByText("Reader Settings")).toBeInTheDocument();
	});

	it("should not render when closed", () => {
		renderWithProviders(<ReaderSettings {...defaultProps} opened={false} />);

		expect(screen.queryByText("Reader Settings")).not.toBeInTheDocument();
	});

	describe("General section", () => {
		it("should display General section header", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("General")).toBeInTheDocument();
		});

		it("should display reading mode selector", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Reading mode")).toBeInTheDocument();
		});

		it("should display auto-hide toolbar toggle", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Auto-hide toolbar")).toBeInTheDocument();
		});

		it("should toggle auto-hide toolbar", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			// Find the auto-hide switch
			const switches = screen.getAllByRole("switch");
			const autoHideSwitch = switches.find(s => {
				const parent = s.closest('.mantine-Group-root');
				return parent?.textContent?.includes("Auto-hide toolbar");
			}) || switches[0];
			fireEvent.click(autoHideSwitch);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});
	});

	describe("Reading Mode", () => {
		it("should display reading mode select with current value", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			// The select should show "Left to Right" as the default
			expect(screen.getByDisplayValue("Left to Right")).toBeInTheDocument();
		});

		it("should show session message when no seriesId is provided", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Navigation direction for this session")).toBeInTheDocument();
		});

		it("should show sync message when seriesId is provided", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} seriesId="series-123" />);

			expect(screen.getByText("Saved to series metadata")).toBeInTheDocument();
		});

		it("should show RTL as selected when readingDirectionOverride is rtl", () => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "rtl",
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByDisplayValue("Right to Left")).toBeInTheDocument();
		});

		it("should show Vertical as selected when readingDirectionOverride is ttb", () => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "ttb",
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByDisplayValue("Vertical")).toBeInTheDocument();
		});

		it("should show Webtoon as selected when readingDirectionOverride is webtoon", () => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "webtoon",
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByDisplayValue("Webtoon")).toBeInTheDocument();
		});
	});

	describe("Display section", () => {
		it("should display Display section header", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Display")).toBeInTheDocument();
		});

		it("should display background color options", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Background color")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Black" })).toBeChecked();
		});

		it("should update background color when changed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Gray" }));

			expect(useReaderStore.getState().settings.backgroundColor).toBe("gray");
		});
	});

	describe("Paginated Mode (LTR/RTL)", () => {
		it("should show Paginated Reader Options header in LTR mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Paginated Reader Options")).toBeInTheDocument();
		});

		it("should show animate page transitions toggle in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Animate page transitions")).toBeInTheDocument();
		});

		it("should show page layout selector in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page layout")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Single page" })).toBeChecked();
		});

		it("should update page layout when changed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Double pages" }));

			expect(useReaderStore.getState().settings.pageLayout).toBe("double");
		});

		it("should show double page options when double layout is selected", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageLayout: "double",
				},
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Show wide pages alone")).toBeInTheDocument();
			expect(screen.getByText("Start on odd page")).toBeInTheDocument();
		});

		it("should show transition style when transitions are enabled", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Transition style")).toBeInTheDocument();
			// Slide is default
			expect(screen.getByRole("radio", { name: "Slide" })).toBeChecked();
		});

		it("should show transition speed when transitions are enabled", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Transition speed")).toBeInTheDocument();
		});

		it("should hide transition options when transitions are disabled", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageTransition: "none",
				},
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Transition style")).not.toBeInTheDocument();
			expect(screen.queryByText("Transition speed")).not.toBeInTheDocument();
		});
	});

	describe("TTB Reading Direction", () => {
		beforeEach(() => {
			// Set to TTB reading direction
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "ttb",
			});
		});

		it("should show Paginated Reader Options header in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			// TTB is just a reading direction, not a special mode
			expect(screen.getByText("Paginated Reader Options")).toBeInTheDocument();
		});

		it("should show page layout selector in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page layout")).toBeInTheDocument();
		});

		it("should show animate page transitions in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Animate page transitions")).toBeInTheDocument();
		});

		it("should show Previous/Next page keyboard shortcut in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Previous/Next page")).toBeInTheDocument();
		});
	});

	describe("Webtoon Reading Direction", () => {
		beforeEach(() => {
			// Set to webtoon reading direction
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "webtoon",
			});
		});

		it("should show Continuous Scroll Options header in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Continuous Scroll Options")).toBeInTheDocument();
		});

		it("should not show page layout selector in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Page layout")).not.toBeInTheDocument();
		});

		it("should not show animate page transitions in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Animate page transitions")).not.toBeInTheDocument();
		});

		it("should show Scroll up/down keyboard shortcut in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scroll up/down")).toBeInTheDocument();
		});
	});

	describe("Continuous Scroll Mode", () => {
		beforeEach(() => {
			// Set to continuous scroll mode
			useReaderStore.setState({
				...useReaderStore.getState(),
				settings: {
					...useReaderStore.getState().settings,
					pageLayout: "continuous",
				},
			});
		});

		it("should show Continuous Scroll Options header", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Continuous Scroll Options")).toBeInTheDocument();
		});

		it("should not show page layout selector in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Page layout")).not.toBeInTheDocument();
		});

		it("should not show animate page transitions in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Animate page transitions")).not.toBeInTheDocument();
		});

		it("should show preload buffer option in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Preload buffer")).toBeInTheDocument();
		});

		it("should show Scroll up/down keyboard shortcut in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scroll up/down")).toBeInTheDocument();
		});

		it("should show side padding option in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Side padding")).toBeInTheDocument();
		});

		it("should show page gap option in continuous mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page gap")).toBeInTheDocument();
		});

		it("should show scale type with only Fit width and Original options", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scale type")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Fit width" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Original size" })).toBeInTheDocument();
		});
	});

	describe("Keyboard Shortcuts", () => {
		it("should display keyboard shortcuts section", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Keyboard Shortcuts")).toBeInTheDocument();
			expect(screen.getByText("Arrow keys, Space")).toBeInTheDocument();
		});

		it("should show Previous/Next page in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Previous/Next page")).toBeInTheDocument();
		});
	});

	describe("Modal behavior", () => {
		it("should call onClose when modal is closed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			// Mantine Modal close button
			const buttons = screen.getAllByRole("button");
			const closeButton = buttons[0];
			fireEvent.click(closeButton);

			expect(defaultProps.onClose).toHaveBeenCalledTimes(1);
		});
	});
});
