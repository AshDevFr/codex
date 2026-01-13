import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { EpubReaderSettings } from "./EpubReaderSettings";
import { useReaderStore } from "@/store/readerStore";

// Default settings to reset store before each test
const defaultSettings = {
	fitMode: "screen" as const,
	pageLayout: "single" as const,
	readingDirection: "ltr" as const,
	backgroundColor: "black" as const,
	pdfMode: "streaming" as const,
	autoHideToolbar: true,
	toolbarHideDelay: 3000,
	epubTheme: "light" as const,
	epubFontSize: 100,
	epubFontFamily: "default" as const,
	epubLineHeight: 140,
	epubMargin: 10,
	preloadPages: 1,
	doublePageShowWideAlone: true,
	doublePageStartOnOdd: true,
	pageTransition: "slide" as const,
	transitionDuration: 200,
	webtoonSidePadding: 0,
	webtoonPageGap: 0,
};

beforeEach(() => {
	useReaderStore.setState({
		settings: { ...defaultSettings },
	});
});

describe("EpubReaderSettings", () => {
	describe("rendering", () => {
		it("should not render when closed", () => {
			renderWithProviders(
				<EpubReaderSettings opened={false} onClose={vi.fn()} />
			);

			expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		});

		it("should render modal when opened", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("dialog")).toBeInTheDocument();
			expect(screen.getByText("Reader Settings")).toBeInTheDocument();
		});

		it("should display theme section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Theme")).toBeInTheDocument();
			expect(screen.getByText("Light")).toBeInTheDocument();
			expect(screen.getByText("Sepia")).toBeInTheDocument();
			expect(screen.getByText("Dark")).toBeInTheDocument();
			expect(screen.getByText("Mint")).toBeInTheDocument();
			expect(screen.getByText("Slate")).toBeInTheDocument();
		});

		it("should display font size section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Font Size")).toBeInTheDocument();
			// Multiple 100% elements exist (value display + slider mark)
			expect(screen.getAllByText("100%").length).toBeGreaterThanOrEqual(1);
			// Multiple sliders exist (font size, line height, margin)
			expect(screen.getAllByRole("slider").length).toBeGreaterThanOrEqual(1);
		});

		it("should display auto-hide toolbar toggle", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Auto-hide Toolbar")).toBeInTheDocument();
			expect(screen.getByRole("switch")).toBeInTheDocument();
		});

		it("should display keyboard shortcuts section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Keyboard Shortcuts")).toBeInTheDocument();
			expect(screen.getByText("Previous/Next page")).toBeInTheDocument();
			expect(screen.getByText("Arrow keys")).toBeInTheDocument();
			expect(screen.getByText("Table of contents")).toBeInTheDocument();
			expect(screen.getByText("Toggle fullscreen")).toBeInTheDocument();
			expect(screen.getByText("Toggle toolbar")).toBeInTheDocument();
			expect(screen.getByText("Close reader")).toBeInTheDocument();
		});
	});

	describe("theme selection", () => {
		it("should update theme when Light is selected", async () => {
			const user = userEvent.setup();
			useReaderStore.getState().setEpubTheme("dark"); // Start with different theme

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			await user.click(screen.getByText("Light"));

			expect(useReaderStore.getState().settings.epubTheme).toBe("light");
		});

		it("should update theme when Sepia is selected", async () => {
			const user = userEvent.setup();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			await user.click(screen.getByText("Sepia"));

			expect(useReaderStore.getState().settings.epubTheme).toBe("sepia");
		});

		it("should update theme when Dark is selected", async () => {
			const user = userEvent.setup();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			await user.click(screen.getByText("Dark"));

			expect(useReaderStore.getState().settings.epubTheme).toBe("dark");
		});

		it("should update theme when Mint is selected", async () => {
			const user = userEvent.setup();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			await user.click(screen.getByText("Mint"));

			expect(useReaderStore.getState().settings.epubTheme).toBe("mint");
		});

		it("should update theme when Slate is selected", async () => {
			const user = userEvent.setup();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			await user.click(screen.getByText("Slate"));

			expect(useReaderStore.getState().settings.epubTheme).toBe("slate");
		});
	});

	describe("font size", () => {
		it("should display current font size value", () => {
			useReaderStore.getState().setEpubFontSize(120);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// 120% appears in the value display (not in marks, so only one instance)
			expect(screen.getByText("120%")).toBeInTheDocument();
		});

		it("should show slider with correct min/max marks", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Marks appear on the slider
			expect(screen.getAllByText("50%").length).toBeGreaterThanOrEqual(1);
			expect(screen.getAllByText("150%").length).toBeGreaterThanOrEqual(1);
			expect(screen.getAllByText("200%").length).toBeGreaterThanOrEqual(1);
		});

		it("should have slider with correct initial value", () => {
			useReaderStore.getState().setEpubFontSize(150);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Font size slider is the first one in the modal
			const sliders = screen.getAllByRole("slider");
			const fontSizeSlider = sliders[0];
			expect(fontSizeSlider).toHaveAttribute("aria-valuenow", "150");
		});
	});

	describe("auto-hide toolbar", () => {
		it("should toggle auto-hide toolbar on click", async () => {
			const user = userEvent.setup();
			const initialValue = useReaderStore.getState().settings.autoHideToolbar;

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Find the switch by its associated label text
			const autoHideSwitch = screen.getByRole("switch");
			await user.click(autoHideSwitch);

			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(!initialValue);
		});

		it("should show checked state when auto-hide is enabled", () => {
			useReaderStore.getState().setAutoHideToolbar(true);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("switch")).toBeChecked();
		});

		it("should show unchecked state when auto-hide is disabled", () => {
			useReaderStore.getState().setAutoHideToolbar(false);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("switch")).not.toBeChecked();
		});
	});

	describe("modal interactions", () => {
		it("should call onClose when modal is closed", async () => {
			const user = userEvent.setup();
			const onClose = vi.fn();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={onClose} />
			);

			// Close via the X button (mantine CloseButton doesn't have accessible name)
			const closeButton = document.querySelector(".mantine-Modal-close");
			expect(closeButton).toBeInTheDocument();
			if (closeButton) {
				await user.click(closeButton);
			}

			expect(onClose).toHaveBeenCalledTimes(1);
		});

		it("should call onClose when clicking outside the modal", async () => {
			const user = userEvent.setup();
			const onClose = vi.fn();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={onClose} />
			);

			// Click the overlay (outside the modal)
			const overlay = document.querySelector(".mantine-Modal-overlay");
			expect(overlay).toBeInTheDocument();
			if (overlay) {
				await user.click(overlay);
			}

			await waitFor(() => {
				expect(onClose).toHaveBeenCalled();
			});
		});
	});

	describe("state persistence", () => {
		it("should reflect store values when modal opens", () => {
			// Set up specific store state - use a unique font size that won't conflict with line height
			useReaderStore.getState().setEpubTheme("dark");
			useReaderStore.getState().setEpubFontSize(130);
			useReaderStore.getState().setAutoHideToolbar(false);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Check that the UI reflects the store state
			expect(screen.getByText("130%")).toBeInTheDocument();
			expect(screen.getByRole("switch")).not.toBeChecked();
		});

		it("should persist changes to the store immediately", async () => {
			const user = userEvent.setup();

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Change theme
			await user.click(screen.getByText("Dark"));
			expect(useReaderStore.getState().settings.epubTheme).toBe("dark");

			// Toggle auto-hide (it's true by default)
			const switchControl = screen.getByRole("switch");
			const wasChecked = useReaderStore.getState().settings.autoHideToolbar;
			await user.click(switchControl);
			expect(useReaderStore.getState().settings.autoHideToolbar).toBe(!wasChecked);
		});
	});

	describe("font family", () => {
		it("should display font family section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Font Family")).toBeInTheDocument();
			expect(screen.getByText("Choose a typeface for reading")).toBeInTheDocument();
		});

		it("should display current font family in dropdown", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Default is selected
			expect(screen.getByRole("textbox")).toHaveValue("Default");
		});

		// Note: Mantine Select dropdown interaction tests are unreliable in jsdom
		// due to scrollIntoView not being available. We test that the store actions
		// work correctly in readerStore.test.ts instead.

		it("should show serif option as selected", () => {
			useReaderStore.getState().setEpubFontFamily("serif");

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("textbox")).toHaveValue("Serif (Georgia)");
		});

		it("should show sans-serif option as selected", () => {
			useReaderStore.getState().setEpubFontFamily("sans-serif");

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("textbox")).toHaveValue("Sans-serif (Helvetica)");
		});

		it("should show monospace option as selected", () => {
			useReaderStore.getState().setEpubFontFamily("monospace");

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("textbox")).toHaveValue("Monospace (Courier)");
		});

		it("should show dyslexic option as selected", () => {
			useReaderStore.getState().setEpubFontFamily("dyslexic");

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByRole("textbox")).toHaveValue("Dyslexic-friendly");
		});
	});

	describe("line spacing", () => {
		it("should display line spacing section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Line Spacing")).toBeInTheDocument();
			expect(screen.getByText("Space between lines of text")).toBeInTheDocument();
		});

		it("should display current line height value", () => {
			useReaderStore.getState().setEpubLineHeight(180);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("180%")).toBeInTheDocument();
		});

		it("should show slider with correct marks", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Tight")).toBeInTheDocument();
			// "Normal" appears in both line spacing and margins slider marks
			expect(screen.getAllByText("Normal").length).toBeGreaterThanOrEqual(1);
			expect(screen.getByText("Relaxed")).toBeInTheDocument();
			expect(screen.getByText("Loose")).toBeInTheDocument();
		});

		it("should have slider with correct initial value", () => {
			useReaderStore.getState().setEpubLineHeight(200);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Find the line height slider (second slider in the modal)
			const sliders = screen.getAllByRole("slider");
			// Line height slider is the second one (after font size)
			const lineHeightSlider = sliders[1];
			expect(lineHeightSlider).toHaveAttribute("aria-valuenow", "200");
		});
	});

	describe("margins", () => {
		it("should display margins section", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("Margins")).toBeInTheDocument();
			expect(screen.getByText("Horizontal padding around text")).toBeInTheDocument();
		});

		it("should display current margin value", () => {
			useReaderStore.getState().setEpubMargin(20);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("20%")).toBeInTheDocument();
		});

		it("should show slider with correct marks", () => {
			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			expect(screen.getByText("None")).toBeInTheDocument();
			// "Normal" appears twice (line spacing and margins)
			expect(screen.getAllByText("Normal").length).toBeGreaterThanOrEqual(1);
			expect(screen.getByText("Wide")).toBeInTheDocument();
			expect(screen.getByText("Max")).toBeInTheDocument();
		});

		it("should have slider with correct initial value", () => {
			useReaderStore.getState().setEpubMargin(25);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Find the margin slider (third slider in the modal)
			const sliders = screen.getAllByRole("slider");
			// Margin slider is the third one (after font size and line height)
			const marginSlider = sliders[2];
			expect(marginSlider).toHaveAttribute("aria-valuenow", "25");
		});
	});

	describe("typography settings state persistence", () => {
		it("should reflect all typography store values when modal opens", () => {
			// Set up specific store state - use unique values to avoid ambiguity
			useReaderStore.getState().setEpubFontFamily("serif");
			useReaderStore.getState().setEpubLineHeight(210);
			useReaderStore.getState().setEpubMargin(25);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Check that the UI reflects the store state
			expect(screen.getByRole("textbox")).toHaveValue("Serif (Georgia)");
			// Use getAllByText since multiple percentage values may exist
			expect(screen.getByText("210%")).toBeInTheDocument();
			expect(screen.getByText("25%")).toBeInTheDocument();
		});
	});
});
