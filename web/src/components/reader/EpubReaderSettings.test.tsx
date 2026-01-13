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
			expect(screen.getByRole("slider")).toBeInTheDocument();
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

			const slider = screen.getByRole("slider");
			expect(slider).toHaveAttribute("aria-valuenow", "150");
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
			// Set up specific store state
			useReaderStore.getState().setEpubTheme("dark");
			useReaderStore.getState().setEpubFontSize(140);
			useReaderStore.getState().setAutoHideToolbar(false);

			renderWithProviders(
				<EpubReaderSettings opened={true} onClose={vi.fn()} />
			);

			// Check that the UI reflects the store state
			expect(screen.getByText("140%")).toBeInTheDocument();
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
});
