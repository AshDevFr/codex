import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { EpubReaderSettings } from "./EpubReaderSettings";

// Default settings to reset store before each test
const defaultSettings = {
  fitMode: "screen" as const,
  pageLayout: "single" as const,
  readingDirection: "ltr" as const,
  backgroundColor: "black" as const,
  pdfMode: "streaming" as const,
  pdfSpreadMode: "single" as const,
  pdfContinuousScroll: false,
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
  autoAdvanceToNextBook: false,
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
        <EpubReaderSettings opened={false} onClose={vi.fn()} />,
      );

      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("should render modal when opened", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByRole("dialog")).toBeInTheDocument();
      expect(screen.getByText("Reader Settings")).toBeInTheDocument();
    });

    it("should display theme section with select dropdown", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Theme")).toBeInTheDocument();
      // Theme select shows "Light" as the default selected value
      // We have two textboxes - one for theme, one for font family
      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Light");
    });

    it("should display font size section", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Font Size")).toBeInTheDocument();
      // Multiple 100% elements exist (value display + slider mark)
      expect(screen.getAllByText("100%").length).toBeGreaterThanOrEqual(1);
      // Multiple sliders exist (font size, line height, margin)
      expect(screen.getAllByRole("slider").length).toBeGreaterThanOrEqual(1);
    });

    it("should display auto-hide toolbar toggle", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Auto-hide Toolbar")).toBeInTheDocument();
      // Now there are multiple switches (auto-hide and auto-advance)
      const switches = screen.getAllByRole("switch");
      expect(switches.length).toBeGreaterThanOrEqual(1);
    });

    it("should display auto-advance toggle", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Auto-advance to next book")).toBeInTheDocument();
      expect(
        screen.getByText("Automatically continue to next book in series"),
      ).toBeInTheDocument();
    });

    it("should display keyboard shortcuts inline (desktop only)", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Keyboard shortcuts are now inline and use compact format
      expect(screen.getByText("Navigate")).toBeInTheDocument();
      expect(screen.getByText("Contents")).toBeInTheDocument();
      expect(screen.getByText("Fullscreen")).toBeInTheDocument();
      expect(screen.getByText("Toolbar")).toBeInTheDocument();
      expect(screen.getByText("Close")).toBeInTheDocument();
    });
  });

  describe("theme selection", () => {
    // Note: Mantine Select dropdown interaction tests are complex in jsdom
    // We test that the correct theme is displayed when set programmatically
    // and that the store actions work correctly in readerStore.test.ts

    it("should display Light theme as selected by default", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Theme select is the first textbox
      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Light");
    });

    it("should display Sepia theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("sepia");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Sepia");
    });

    it("should display Dark theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("dark");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Dark");
    });

    it("should display Mint theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("mint");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Mint");
    });

    it("should display Slate theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("slate");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Slate");
    });

    // Test new themes
    it("should display Night theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("night");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Night (OLED)");
    });

    it("should display Paper theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("paper");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Paper (Warm)");
    });

    it("should display Ocean theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("ocean");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Ocean");
    });

    it("should display Forest theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("forest");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Forest");
    });

    it("should display Rose theme when selected in store", () => {
      useReaderStore.getState().setEpubTheme("rose");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[0]).toHaveValue("Rose");
    });
  });

  describe("font size", () => {
    it("should display current font size value", () => {
      useReaderStore.getState().setEpubFontSize(120);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // 120% appears in the value display (not in marks, so only one instance)
      expect(screen.getByText("120%")).toBeInTheDocument();
    });

    it("should show slider with correct min/max marks", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Marks appear on the slider (50%, 100%, 200%)
      expect(screen.getAllByText("50%").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("100%").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("200%").length).toBeGreaterThanOrEqual(1);
    });

    it("should have slider with correct initial value", () => {
      useReaderStore.getState().setEpubFontSize(150);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
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
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Find the auto-hide switch (first switch)
      const switches = screen.getAllByRole("switch");
      const autoHideSwitch = switches[0];
      await user.click(autoHideSwitch);

      expect(useReaderStore.getState().settings.autoHideToolbar).toBe(
        !initialValue,
      );
    });

    it("should show checked state when auto-hide is enabled", () => {
      useReaderStore.getState().setAutoHideToolbar(true);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const switches = screen.getAllByRole("switch");
      expect(switches[0]).toBeChecked();
    });

    it("should show unchecked state when auto-hide is disabled", () => {
      useReaderStore.getState().setAutoHideToolbar(false);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const switches = screen.getAllByRole("switch");
      expect(switches[0]).not.toBeChecked();
    });
  });

  describe("auto-advance to next book", () => {
    it("should toggle auto-advance on click", async () => {
      const user = userEvent.setup();
      const initialValue =
        useReaderStore.getState().settings.autoAdvanceToNextBook;

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Find the auto-advance switch (second switch)
      const switches = screen.getAllByRole("switch");
      const autoAdvanceSwitch = switches[1];
      await user.click(autoAdvanceSwitch);

      expect(useReaderStore.getState().settings.autoAdvanceToNextBook).toBe(
        !initialValue,
      );
    });

    it("should show checked state when auto-advance is enabled", () => {
      useReaderStore.getState().setAutoAdvanceToNextBook(true);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const switches = screen.getAllByRole("switch");
      expect(switches[1]).toBeChecked();
    });

    it("should show unchecked state when auto-advance is disabled", () => {
      useReaderStore.getState().setAutoAdvanceToNextBook(false);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const switches = screen.getAllByRole("switch");
      expect(switches[1]).not.toBeChecked();
    });
  });

  describe("modal interactions", () => {
    it("should call onClose when modal is closed", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={onClose} />,
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
        <EpubReaderSettings opened={true} onClose={onClose} />,
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
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Check that the UI reflects the store state
      expect(screen.getByText("130%")).toBeInTheDocument();
      const switches = screen.getAllByRole("switch");
      expect(switches[0]).not.toBeChecked(); // auto-hide toolbar
    });

    it("should persist changes to the store immediately", async () => {
      const user = userEvent.setup();

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Toggle auto-hide (it's true by default) - first switch
      const switches = screen.getAllByRole("switch");
      const wasChecked = useReaderStore.getState().settings.autoHideToolbar;
      await user.click(switches[0]);
      expect(useReaderStore.getState().settings.autoHideToolbar).toBe(
        !wasChecked,
      );

      // Theme change is tested via store in "theme selection" tests
      // since Mantine Select interactions are complex in jsdom
    });
  });

  describe("font family", () => {
    it("should display font family section", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Font Family")).toBeInTheDocument();
    });

    it("should display current font family in dropdown", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Font family select is the second textbox (after theme)
      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Default");
    });

    // Note: Mantine Select dropdown interaction tests are unreliable in jsdom
    // due to scrollIntoView not being available. We test that the store actions
    // work correctly in readerStore.test.ts instead.

    it("should show serif option as selected", () => {
      useReaderStore.getState().setEpubFontFamily("serif");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Font family select is the second textbox (after theme)
      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Serif (Georgia)");
    });

    it("should show sans-serif option as selected", () => {
      useReaderStore.getState().setEpubFontFamily("sans-serif");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Sans-serif (Helvetica)");
    });

    it("should show monospace option as selected", () => {
      useReaderStore.getState().setEpubFontFamily("monospace");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Monospace (Courier)");
    });

    it("should show dyslexic option as selected", () => {
      useReaderStore.getState().setEpubFontFamily("dyslexic");

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Dyslexic-friendly");
    });
  });

  describe("line spacing", () => {
    it("should display line spacing section", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Line Spacing")).toBeInTheDocument();
    });

    it("should display current line height value", () => {
      useReaderStore.getState().setEpubLineHeight(180);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("180%")).toBeInTheDocument();
    });

    it("should show slider with correct marks", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Tight")).toBeInTheDocument();
      // "Normal" appears in both line spacing and margins slider marks
      expect(screen.getAllByText("Normal").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("Loose")).toBeInTheDocument();
    });

    it("should have slider with correct initial value", () => {
      useReaderStore.getState().setEpubLineHeight(200);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
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
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("Margins")).toBeInTheDocument();
    });

    it("should display current margin value", () => {
      useReaderStore.getState().setEpubMargin(20);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("20%")).toBeInTheDocument();
    });

    it("should show slider with correct marks", () => {
      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      expect(screen.getByText("None")).toBeInTheDocument();
      // "Normal" appears twice (line spacing and margins)
      expect(screen.getAllByText("Normal").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("Max")).toBeInTheDocument();
    });

    it("should have slider with correct initial value", () => {
      useReaderStore.getState().setEpubMargin(25);

      renderWithProviders(
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
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
        <EpubReaderSettings opened={true} onClose={vi.fn()} />,
      );

      // Check that the UI reflects the store state
      // Font family select is the second textbox (after theme)
      const textboxes = screen.getAllByRole("textbox");
      expect(textboxes[1]).toHaveValue("Serif (Georgia)");
      // Use getAllByText since multiple percentage values may exist
      expect(screen.getByText("210%")).toBeInTheDocument();
      expect(screen.getByText("25%")).toBeInTheDocument();
    });
  });
});
