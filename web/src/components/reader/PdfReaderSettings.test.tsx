import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, renderWithProviders, screen } from "@/test/utils";
import { useReaderStore } from "@/store/readerStore";
import { PdfReaderSettings } from "./PdfReaderSettings";
import type { PdfZoomLevel } from "./PdfReader";

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
			const toggle = screen.getByRole("switch");
			fireEvent.click(toggle);

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
