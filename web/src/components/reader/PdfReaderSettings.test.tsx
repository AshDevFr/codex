import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen, userEvent } from "@/test/utils";
import type { PdfZoomLevel } from "./PdfReader";
import { PdfReaderSettings } from "./PdfReaderSettings";

describe("PdfReaderSettings", () => {
	const defaultProps = {
		opened: true,
		onClose: vi.fn(),
		zoomLevel: "fit-page" as PdfZoomLevel,
		onZoomChange: vi.fn(),
	};

	beforeEach(() => {
		vi.clearAllMocks();
		// Reset store to defaults
		useReaderStore.getState().resetSession();
		useReaderStore.setState({
			settings: {
				...useReaderStore.getState().settings,
				pdfMode: "native",
				pdfContinuousScroll: false,
				pdfSpreadMode: "single",
			},
		});
	});

	it("should render when opened", () => {
		renderWithProviders(<PdfReaderSettings {...defaultProps} />);

		expect(screen.getByText("Reader Settings")).toBeInTheDocument();
	});

	it("should not render when closed", () => {
		renderWithProviders(<PdfReaderSettings {...defaultProps} opened={false} />);

		expect(screen.queryByText("Reader Settings")).not.toBeInTheDocument();
	});

	describe("PDF rendering mode toggle", () => {
		it("should display PDF mode toggle options including Auto", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("PDF Rendering Mode")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Auto" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Streaming" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Native" })).toBeInTheDocument();
		});

		it("should show auto mode description when auto is selected", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "auto",
				},
			});

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Automatically selects based on file size (>100MB uses streaming)"),
			).toBeInTheDocument();
		});

		it("should show native mode description when native is selected", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Downloads full PDF for text selection and search"),
			).toBeInTheDocument();
		});

		it("should change PDF mode when Auto is selected", async () => {
			// Start with native mode
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "native",
				},
			});

			const user = userEvent.setup();
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			await user.click(screen.getByRole("radio", { name: "Auto" }));

			expect(useReaderStore.getState().settings.pdfMode).toBe("auto");
		});

		it("should change PDF mode when Streaming is selected", async () => {
			const user = userEvent.setup();
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			await user.click(screen.getByRole("radio", { name: "Streaming" }));

			expect(useReaderStore.getState().settings.pdfMode).toBe("streaming");
		});

		it("should show streaming mode description when streaming is selected", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfMode: "streaming",
				},
			});

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Server renders pages as images (lower bandwidth)"),
			).toBeInTheDocument();
		});

		it("should show re-open warning message", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Re-open the book after changing to apply"),
			).toBeInTheDocument();
		});
	});

	describe("zoom level controls", () => {
		it("should display zoom level options", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Zoom")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Fit Page" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Fit Width" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "100%" })).toBeInTheDocument();
		});

		it("should call onZoomChange when zoom level is selected", () => {
			const onZoomChange = vi.fn();
			renderWithProviders(
				<PdfReaderSettings {...defaultProps} onZoomChange={onZoomChange} />,
			);

			fireEvent.click(screen.getByRole("radio", { name: "Fit Width" }));
			expect(onZoomChange).toHaveBeenCalledWith("fit-width");
		});

		it("should display additional zoom options", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("More Zoom")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "50%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "75%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "125%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "150%" })).toBeInTheDocument();
		});

		it("should call onZoomChange with percentage values", () => {
			const onZoomChange = vi.fn();
			renderWithProviders(
				<PdfReaderSettings {...defaultProps} onZoomChange={onZoomChange} />,
			);

			fireEvent.click(screen.getByRole("radio", { name: "150%" }));
			expect(onZoomChange).toHaveBeenCalledWith("150%");
		});
	});

	describe("background color", () => {
		it("should display background color options", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Background")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Black" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Gray" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "White" })).toBeInTheDocument();
		});

		it("should update store when background color is changed", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Gray" }));

			expect(useReaderStore.getState().settings.backgroundColor).toBe("gray");
		});
	});

	describe("page layout (spread mode)", () => {
		it("should display page layout options when not in continuous scroll mode", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page layout")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Single" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Double" })).toBeInTheDocument();
			expect(
				screen.getByRole("radio", { name: "Double (Odd)" }),
			).toBeInTheDocument();
		});

		it("should hide page layout when continuous scroll is enabled", async () => {
			const user = userEvent.setup();
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Enable continuous scroll
			const switches = screen.getAllByRole("switch");
			const continuousScrollSwitch = switches.find(
				(s) => s.closest('[class*="Group"]')?.textContent?.includes("Continuous Scroll")
			) || switches[1]; // Fallback to second switch
			await user.click(continuousScrollSwitch);

			// Page layout should be hidden
			expect(screen.queryByText("Page layout")).not.toBeInTheDocument();
		});

		it("should update store when spread mode is changed to double", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Double" }));

			expect(useReaderStore.getState().settings.pdfSpreadMode).toBe("double");
		});

		it("should show description for spread modes", () => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					pdfSpreadMode: "single",
				},
			});

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Display one page at a time"),
			).toBeInTheDocument();
		});
	});

	describe("continuous scroll", () => {
		it("should display continuous scroll toggle", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Continuous Scroll")).toBeInTheDocument();
			expect(
				screen.getByText("Scroll through all pages vertically"),
			).toBeInTheDocument();
		});

		it("should toggle continuous scroll setting", async () => {
			const user = userEvent.setup();
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Find the continuous scroll toggle (second switch after auto-hide)
			const switches = screen.getAllByRole("switch");
			const continuousScrollSwitch = switches[1];
			await user.click(continuousScrollSwitch);

			expect(useReaderStore.getState().settings.pdfContinuousScroll).toBe(true);
		});
	});

	describe("auto-hide toolbar", () => {
		it("should display auto-hide toggle", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Auto-hide toolbar")).toBeInTheDocument();
		});

		it("should toggle auto-hide setting", async () => {
			// Set initial state
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					autoHideToolbar: true,
				},
			});

			const user = userEvent.setup();
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Auto-hide is the first switch
			const switches = screen.getAllByRole("switch");
			const autoHideSwitch = switches[0];
			await user.click(autoHideSwitch);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});
	});

	describe("keyboard shortcuts", () => {
		it("should display keyboard shortcuts section", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Keyboard Shortcuts")).toBeInTheDocument();
			expect(screen.getByText("Previous/Next page")).toBeInTheDocument();
			// Arrow keys appear in both the detailed box and the footer
			expect(screen.getAllByText("← → ↑ ↓").length).toBeGreaterThanOrEqual(1);
			// "Search" appears in both the shortcuts box and footer
			expect(screen.getAllByText("Search").length).toBeGreaterThanOrEqual(1);
			expect(screen.getByText("Ctrl+F / Cmd+F")).toBeInTheDocument();
		});
	});

	describe("PDF-specific features info", () => {
		it("should display native PDF features section", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Native PDF Features")).toBeInTheDocument();
			expect(screen.getByText("Text selection and copy")).toBeInTheDocument();
			expect(
				screen.getByText("Search within document (Ctrl+F)"),
			).toBeInTheDocument();
			expect(
				screen.getByText("Clickable links and bookmarks"),
			).toBeInTheDocument();
			expect(
				screen.getByText("Vector rendering (sharp at any zoom)"),
			).toBeInTheDocument();
		});
	});

	describe("modal behavior", () => {
		it("should render modal with correct title", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Modal is rendered with consistent "Reader Settings" title
			expect(screen.getByText("Reader Settings")).toBeInTheDocument();
		});
	});
});
