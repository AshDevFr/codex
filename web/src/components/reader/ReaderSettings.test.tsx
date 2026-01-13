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
			preloadedImages: new Set<string>(),
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

	describe("Reading Mode", () => {
		it("should display reading mode selector", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Reading mode")).toBeInTheDocument();
		});

		it("should display reading mode select with current value", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByDisplayValue("Left to Right")).toBeInTheDocument();
		});

		it("should show session message when no seriesId is provided", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Session only")).toBeInTheDocument();
		});

		it("should show sync message when seriesId is provided", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} seriesId="series-123" />);

			expect(screen.getByText("Saved to series")).toBeInTheDocument();
		});

		it("should show RTL as selected when readingDirectionOverride is rtl", () => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "rtl",
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByDisplayValue("Right to Left (Manga)")).toBeInTheDocument();
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

			expect(screen.getByDisplayValue("Webtoon (Continuous Scroll)")).toBeInTheDocument();
		});
	});

	describe("Display section", () => {
		it("should display Display section header", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Display")).toBeInTheDocument();
		});

		it("should display scale selector", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scale")).toBeInTheDocument();
		});

		it("should display background color options", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Background")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Black" })).toBeChecked();
		});

		it("should update background color when changed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Gray" }));

			expect(useReaderStore.getState().settings.backgroundColor).toBe("gray");
		});
	});

	describe("Paginated Mode (LTR/RTL/TTB)", () => {
		it("should show page layout selector in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page layout")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Single" })).toBeChecked();
		});

		it("should update page layout when changed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Double" }));

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

			expect(screen.getByText("Wide pages alone")).toBeInTheDocument();
			expect(screen.getByText("Start on odd page")).toBeInTheDocument();
		});

		it("should show Transitions section in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Transitions")).toBeInTheDocument();
		});

		it("should show page transitions selector in paginated mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page transitions")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "None" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Fade" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Slide" })).toBeInTheDocument();
		});

		it("should show slide as default transition", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByRole("radio", { name: "Slide" })).toBeChecked();
		});

		it("should show transition speed when transitions are enabled", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Speed")).toBeInTheDocument();
		});

		it("should hide transition speed when transitions are set to none", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pageTransition: "none",
				},
			});

			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page transitions")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "None" })).toBeChecked();
			expect(screen.queryByText("Speed")).not.toBeInTheDocument();
		});
	});

	describe("TTB Reading Direction", () => {
		beforeEach(() => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "ttb",
			});
		});

		it("should show page layout selector in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page layout")).toBeInTheDocument();
		});

		it("should show page transitions selector in TTB mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page transitions")).toBeInTheDocument();
		});
	});

	describe("Webtoon/Continuous Scroll Mode", () => {
		beforeEach(() => {
			useReaderStore.setState({
				...useReaderStore.getState(),
				readingDirectionOverride: "webtoon",
			});
		});

		it("should not show page layout selector in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Page layout")).not.toBeInTheDocument();
		});

		it("should not show page transitions selector in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Page transitions")).not.toBeInTheDocument();
		});

		it("should not show Transitions section in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.queryByText("Transitions")).not.toBeInTheDocument();
		});

		it("should show Scroll Options section in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scroll Options")).toBeInTheDocument();
		});

		it("should show side padding option in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Side padding")).toBeInTheDocument();
		});

		it("should show page gap option in webtoon mode", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page gap")).toBeInTheDocument();
		});

		it("should show scale type with only Fit width and Original options", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Scale")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Fit width" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Original" })).toBeInTheDocument();
		});
	});

	describe("Common options", () => {
		it("should show preload pages option", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Preload pages")).toBeInTheDocument();
		});

		it("should show auto-hide toolbar option", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			expect(screen.getByText("Auto-hide toolbar")).toBeInTheDocument();
		});

		it("should toggle auto-hide toolbar", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			const switches = screen.getAllByRole("switch");
			const autoHideSwitch = switches.find(s => {
				const parent = s.closest('.mantine-Group-root');
				return parent?.textContent?.includes("Auto-hide toolbar");
			}) || switches[switches.length - 1];
			fireEvent.click(autoHideSwitch);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});
	});

	describe("Modal behavior", () => {
		it("should call onClose when modal is closed", () => {
			renderWithProviders(<ReaderSettings {...defaultProps} />);

			const buttons = screen.getAllByRole("button");
			const closeButton = buttons[0];
			fireEvent.click(closeButton);

			expect(defaultProps.onClose).toHaveBeenCalledTimes(1);
		});
	});
});
