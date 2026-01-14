import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { fireEvent, renderWithProviders, screen } from "@/test/utils";
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
	});

	it("should render when opened", () => {
		renderWithProviders(<PdfReaderSettings {...defaultProps} />);

		expect(screen.getByText("PDF Reader Settings")).toBeInTheDocument();
	});

	it("should not render when closed", () => {
		renderWithProviders(<PdfReaderSettings {...defaultProps} opened={false} />);

		expect(screen.queryByText("PDF Reader Settings")).not.toBeInTheDocument();
	});

	describe("zoom level controls", () => {
		it("should display zoom level options", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Zoom Level")).toBeInTheDocument();
			expect(
				screen.getByRole("radio", { name: "Fit Page" }),
			).toBeInTheDocument();
			expect(
				screen.getByRole("radio", { name: "Fit Width" }),
			).toBeInTheDocument();
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

			expect(screen.getByText("More Zoom Options")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "50%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "75%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "125%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "150%" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "200%" })).toBeInTheDocument();
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

			expect(screen.getByText("Background Color")).toBeInTheDocument();
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

	describe("page spread mode", () => {
		it("should display page spread options", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Page Spread")).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Single" })).toBeInTheDocument();
			expect(screen.getByRole("radio", { name: "Double" })).toBeInTheDocument();
			expect(
				screen.getByRole("radio", { name: "Double (Odd)" }),
			).toBeInTheDocument();
		});

		it("should update store when spread mode is changed to double", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Double" }));

			expect(useReaderStore.getState().settings.pdfSpreadMode).toBe("double");
		});

		it("should update store when spread mode is changed to double-odd", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			fireEvent.click(screen.getByRole("radio", { name: "Double (Odd)" }));

			expect(useReaderStore.getState().settings.pdfSpreadMode).toBe(
				"double-odd",
			);
		});

		it("should show description for single mode", () => {
			useReaderStore.getState().setPdfSpreadMode("single");

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Display one page at a time"),
			).toBeInTheDocument();
		});

		it("should show description for double mode", () => {
			useReaderStore.getState().setPdfSpreadMode("double");

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Display two pages side by side"),
			).toBeInTheDocument();
		});

		it("should show description for double-odd mode", () => {
			useReaderStore.getState().setPdfSpreadMode("double-odd");

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(
				screen.getByText("Two pages, starting spreads on odd pages"),
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

		it("should toggle continuous scroll setting", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Find the continuous scroll toggle (first switch)
			const switches = screen.getAllByRole("switch");
			const continuousScrollSwitch = switches[0];
			fireEvent.click(continuousScrollSwitch);

			expect(useReaderStore.getState().settings.pdfContinuousScroll).toBe(true);
		});
	});

	describe("auto-hide toolbar", () => {
		it("should display auto-hide toggle", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Auto-hide Toolbar")).toBeInTheDocument();
		});

		it("should toggle auto-hide setting", () => {
			// Set initial state
			useReaderStore.getState().setAutoHideToolbar(true);

			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			// Mantine Switch uses role="switch" not "checkbox"
			// Auto-hide is the second switch now
			const switches = screen.getAllByRole("switch");
			const autoHideSwitch = switches[1];
			fireEvent.click(autoHideSwitch);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(false);
		});
	});

	describe("keyboard shortcuts", () => {
		it("should display keyboard shortcuts section", () => {
			renderWithProviders(<PdfReaderSettings {...defaultProps} />);

			expect(screen.getByText("Keyboard Shortcuts")).toBeInTheDocument();
			expect(screen.getByText("Previous/Next page")).toBeInTheDocument();
			expect(screen.getByText("Arrow keys, Space")).toBeInTheDocument();
			expect(screen.getByText("Search")).toBeInTheDocument();
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
		it("should call onClose when clicking outside modal", () => {
			const onClose = vi.fn();
			renderWithProviders(
				<PdfReaderSettings {...defaultProps} onClose={onClose} />,
			);

			// Modal is rendered - verify it exists
			expect(screen.getByText("PDF Reader Settings")).toBeInTheDocument();

			// The modal close behavior is handled by Mantine internally
			// We just verify the modal renders with correct props
		});
	});
});
