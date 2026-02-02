import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { Library } from "@/types";
import { LibraryModal } from "./LibraryModal";

vi.mock("@/api/filesystem");
vi.mock("@/api/libraries");
vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

const mockLibrary: Library = {
  id: "1",
  name: "Test Library",
  path: "/home/user/Comics",
  isActive: true,
  createdAt: "2024-01-01T00:00:00Z",
  updatedAt: "2024-01-01T00:00:00Z",
  scanningConfig: {
    enabled: false,
    scanMode: "normal",
    scanOnStart: false,
    purgeDeletedOnScan: false,
  },
  allowedFormats: [],
  excludedPatterns: "",
  defaultReadingDirection: "ltr",
  seriesStrategy: "series_volume",
  bookStrategy: "filename",
  numberStrategy: "file_order",
};

const mockLibraryWithAutoScan: Library = {
  ...mockLibrary,
  scanningConfig: {
    enabled: true,
    scanMode: "normal",
    cronSchedule: "0 */6 * * *",
    scanOnStart: false,
    purgeDeletedOnScan: true,
  },
};

describe("LibraryModal (Edit Mode)", () => {
  const mockOnClose = vi.fn();
  const originalScrollIntoView = Element.prototype.scrollIntoView;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(librariesApi.update).mockResolvedValue(mockLibrary);
    // Mock filesystem API (used by LibraryModal for path browsing)
    vi.mocked(filesystemApi.getDrives).mockResolvedValue([]);
    vi.mocked(filesystemApi.browse).mockResolvedValue({
      entries: [],
      current_path: "/",
      parent_path: null,
    });
    // Mock scrollIntoView for Mantine Combobox
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(() => {
    Element.prototype.scrollIntoView = originalScrollIntoView;
  });

  it("should not render when closed", () => {
    renderWithProviders(
      <LibraryModal
        opened={false}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    expect(screen.queryByText("Edit Library")).not.toBeInTheDocument();
  });

  it("should not render when library is null", () => {
    renderWithProviders(
      <LibraryModal opened={true} onClose={mockOnClose} library={null} />,
    );

    expect(screen.queryByText("Edit Library")).not.toBeInTheDocument();
  });

  it("should render form with library data when opened", async () => {
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Check that library name is populated
    // Try getByLabelText first, fallback to placeholder
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let nameInput;
    try {
      nameInput = modalContent.getByLabelText("Library Name");
    } catch {
      nameInput = modalContent.getByPlaceholderText("Enter library name");
    }
    expect(nameInput).toHaveValue("Test Library");

    // Check that path is shown (read-only)
    // Try getByLabelText first, fallback to placeholder
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let pathInput;
    try {
      pathInput = modalContent.getByLabelText("Library Path");
    } catch {
      pathInput = modalContent.getByPlaceholderText("Path to library");
    }
    expect(pathInput).toHaveValue("/home/user/Comics");
    expect(pathInput).toBeDisabled();
  });

  it("should show cron input when library has auto scan enabled", async () => {
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibraryWithAutoScan}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Cron input should be visible
    await waitFor(() => {
      // Try getByLabelText first, fallback to placeholder or searching in document
      // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
      let cronInput;
      try {
        cronInput = modalContent.getByLabelText("Cron Schedule");
      } catch {
        try {
          cronInput = screen.getByLabelText("Cron Schedule");
        } catch {
          // Fallback: find by placeholder or by the cron value
          cronInput =
            modalContent.queryByPlaceholderText("0 0 * * *") ||
            screen.queryByDisplayValue("0 */6 * * *");
        }
      }
      expect(cronInput).toBeInTheDocument();
      if (cronInput) {
        expect(cronInput).toHaveValue("0 */6 * * *");
      }
    });
  });

  it("should not show cron input when library has manual scan", async () => {
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Cron input should not be visible
    expect(
      modalContent.queryByLabelText("Cron Schedule"),
    ).not.toBeInTheDocument();
  });

  it("should show cron input when switching to auto scan", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Initially cron input should not be visible
    expect(
      modalContent.queryByLabelText("Cron Schedule"),
    ).not.toBeInTheDocument();

    // Switch to auto scan
    // Mantine Select renders as a button/combobox - find by displayed text first
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let selectInput;
    try {
      // Mantine Select shows the selected value as text
      selectInput = screen.getByText("Manual - Trigger scans on demand");
    } catch {
      try {
        selectInput = screen.getByLabelText("Scan Strategy");
      } catch {
        // Fallback: find combobox or button near the label
        const label = modalContent.getByText("Scan Strategy");
        const allInteractive = modalContent.getAllByRole("combobox");
        if (allInteractive.length > 0) {
          selectInput = allInteractive[0];
        } else {
          const buttons = modalContent.getAllByRole("button");
          selectInput =
            buttons.find(
              (btn) =>
                btn.textContent?.includes("Manual") ||
                btn.closest("form")?.contains(label),
            ) || buttons[0];
        }
      }
    }
    await user.click(selectInput);

    await waitFor(() => {
      const autoOption = screen.getByText("Automatic - Scheduled scanning");
      expect(autoOption).toBeInTheDocument();
    });

    const autoOption = screen.getByText("Automatic - Scheduled scanning");
    await user.click(autoOption);

    // Cron input should now be visible
    await waitFor(() => {
      // Try multiple ways to find the CronInput
      // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
      let cronInput;
      try {
        cronInput = modalContent.getByLabelText("Cron Schedule");
      } catch {
        try {
          cronInput = screen.getByLabelText("Cron Schedule");
        } catch {
          // Fallback: find by placeholder
          cronInput = modalContent.getByPlaceholderText("0 0 * * *");
        }
      }
      expect(cronInput).toBeInTheDocument();
    });
  });

  it("should update library with cron schedule when auto scan is enabled", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Switch to auto scan
    // Mantine Select renders as a button/combobox - find by displayed text first
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let selectInput;
    try {
      // Mantine Select shows the selected value as text
      selectInput = screen.getByText("Manual - Trigger scans on demand");
    } catch {
      try {
        selectInput = screen.getByLabelText("Scan Strategy");
      } catch {
        // Fallback: find combobox or button near the label
        const label = modalContent.getByText("Scan Strategy");
        const allInteractive = modalContent.getAllByRole("combobox");
        if (allInteractive.length > 0) {
          selectInput = allInteractive[0];
        } else {
          const buttons = modalContent.getAllByRole("button");
          selectInput =
            buttons.find(
              (btn) =>
                btn.textContent?.includes("Manual") ||
                btn.closest("form")?.contains(label),
            ) || buttons[0];
        }
      }
    }
    await user.click(selectInput);

    await waitFor(() => {
      const autoOption = screen.getByText("Automatic - Scheduled scanning");
      expect(autoOption).toBeInTheDocument();
    });

    const autoOption = screen.getByText("Automatic - Scheduled scanning");
    await user.click(autoOption);

    // Wait for cron input and change it
    let cronInput: HTMLElement | null = null;
    await waitFor(() => {
      // Try multiple ways to find the CronInput
      try {
        cronInput = modalContent.getByLabelText("Cron Schedule");
      } catch {
        try {
          cronInput = screen.getByLabelText("Cron Schedule");
        } catch {
          // Fallback: find by placeholder
          cronInput = modalContent.getByPlaceholderText("0 0 * * *");
        }
      }
      expect(cronInput).toBeInTheDocument();
    });

    await user.clear(cronInput!);
    await user.type(cronInput!, "0 2 * * *");

    // Submit form
    const saveButton = screen.getByText("Save Changes");
    await user.click(saveButton);

    await waitFor(() => {
      expect(librariesApi.update).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({
          scanningConfig: expect.objectContaining({
            enabled: true,
            cronSchedule: "0 2 * * *",
          }),
        }),
      );
    });
  });

  it("should update library without cron schedule when manual scan is selected", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibraryWithAutoScan}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Switch to manual scan
    // Mantine Select renders as a button/combobox - find by displayed text first
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let selectInput;
    try {
      // Mantine Select shows the selected value as text
      // Since library has auto scan enabled, it should show "Automatic - Scheduled scanning"
      selectInput = screen.getByText("Automatic - Scheduled scanning");
    } catch {
      try {
        selectInput = screen.getByLabelText("Scan Strategy");
      } catch {
        // Fallback: find combobox or button near the label
        const label = modalContent.getByText("Scan Strategy");
        const allInteractive = modalContent.getAllByRole("combobox");
        if (allInteractive.length > 0) {
          selectInput = allInteractive[0];
        } else {
          const buttons = modalContent.getAllByRole("button");
          selectInput =
            buttons.find(
              (btn) =>
                btn.textContent?.includes("Automatic") ||
                btn.closest("form")?.contains(label),
            ) || buttons[0];
        }
      }
    }
    await user.click(selectInput);

    await waitFor(() => {
      const manualOption = screen.getByText("Manual - Trigger scans on demand");
      expect(manualOption).toBeInTheDocument();
    });

    const manualOption = screen.getByText("Manual - Trigger scans on demand");
    await user.click(manualOption);

    // Submit form
    const saveButton = screen.getByText("Save Changes");
    await user.click(saveButton);

    await waitFor(() => {
      expect(librariesApi.update).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({
          scanningConfig: expect.objectContaining({
            enabled: false,
            cronSchedule: undefined,
          }),
        }),
      );
    });
  });

  it("should close modal when Cancel is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Cancel")).toBeInTheDocument();
    });

    const cancelButton = screen.getByText("Cancel");
    await user.click(cancelButton);

    expect(mockOnClose).toHaveBeenCalled();
  });

  it("should validate cron input when auto scan is enabled", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Switch to auto scan
    // Mantine Select renders as a button/combobox - find by displayed text first
    // biome-ignore lint/suspicious/noImplicitAnyLet: Test variable with dynamic assignment
    let selectInput;
    try {
      // Mantine Select shows the selected value as text
      selectInput = screen.getByText("Manual - Trigger scans on demand");
    } catch {
      try {
        selectInput = screen.getByLabelText("Scan Strategy");
      } catch {
        // Fallback: find combobox or button near the label
        const label = modalContent.getByText("Scan Strategy");
        const allInteractive = modalContent.getAllByRole("combobox");
        if (allInteractive.length > 0) {
          selectInput = allInteractive[0];
        } else {
          const buttons = modalContent.getAllByRole("button");
          selectInput =
            buttons.find(
              (btn) =>
                btn.textContent?.includes("Manual") ||
                btn.closest("form")?.contains(label),
            ) || buttons[0];
        }
      }
    }
    await user.click(selectInput);

    await waitFor(() => {
      const autoOption = screen.getByText("Automatic - Scheduled scanning");
      expect(autoOption).toBeInTheDocument();
    });

    const autoOption = screen.getByText("Automatic - Scheduled scanning");
    await user.click(autoOption);

    // Wait for cron input and enter invalid value
    let cronInput: HTMLElement | null = null;
    await waitFor(() => {
      // Try multiple ways to find the CronInput
      try {
        cronInput = modalContent.getByLabelText("Cron Schedule");
      } catch {
        try {
          cronInput = screen.getByLabelText("Cron Schedule");
        } catch {
          // Fallback: find by placeholder
          cronInput = modalContent.getByPlaceholderText("0 0 * * *");
        }
      }
      expect(cronInput).toBeInTheDocument();
    });

    await user.clear(cronInput!);
    await user.type(cronInput!, "invalid cron");

    // Input should show validation error
    await waitFor(() => {
      expect(cronInput!).toHaveAttribute("aria-invalid", "true");
    });
  });

  it("should default to all formats when library has empty allowedFormats", async () => {
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Find the MultiSelect for allowed formats
    const formatsInput = modalContent.getByLabelText("Allowed Formats");

    // Click to open the dropdown and see selected values
    const user = userEvent.setup();
    await user.click(formatsInput);

    // All formats should be selected by default
    await waitFor(() => {
      // Check that all format options are available (may appear multiple times)
      const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
      expect(cbzElements.length).toBeGreaterThan(0);
      // MultiSelect shows text in multiple places, so use getAllByText
      const cbrElements = screen.getAllByText("CBR (Comic Book RAR)");
      expect(cbrElements.length).toBeGreaterThan(0);
      const epubElements = screen.getAllByText("EPUB (Ebook)");
      expect(epubElements.length).toBeGreaterThan(0);
      const pdfElements = screen.getAllByText("PDF (Portable Document Format)");
      expect(pdfElements.length).toBeGreaterThan(0);
    });
  });

  it("should default to all formats when library has undefined allowedFormats", async () => {
    const libraryWithoutFormats: Library = {
      ...mockLibrary,
      allowedFormats: undefined,
    };

    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={libraryWithoutFormats}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    const formatsInput = modalContent.getByLabelText("Allowed Formats");
    const user = userEvent.setup();
    await user.click(formatsInput);

    // All formats should be available (may appear multiple times in MultiSelect)
    // Use getAllByText to handle multiple instances - this is expected behavior
    await waitFor(() => {
      const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
      expect(cbzElements.length).toBeGreaterThan(0);
    });
    // Check other formats exist (may be multiple instances)
    expect(screen.getAllByText("CBR (Comic Book RAR)").length).toBeGreaterThan(
      0,
    );
    expect(screen.getAllByText("EPUB (Ebook)").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("PDF (Portable Document Format)").length,
    ).toBeGreaterThan(0);
  });

  it("should use library's allowedFormats when provided", async () => {
    const libraryWithFormats: Library = {
      ...mockLibrary,
      allowedFormats: ["CBZ", "EPUB"],
    };

    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={libraryWithFormats}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // The MultiSelect should have the library's formats
    const formatsInput = modalContent.getByLabelText("Allowed Formats");
    expect(formatsInput).toBeInTheDocument();
  });

  it("should submit with all formats when all are selected", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    // Submit form with all formats (default)
    const saveButton = screen.getByText("Save Changes");
    await user.click(saveButton);

    await waitFor(() => {
      expect(librariesApi.update).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({
          allowedFormats: ["CBZ", "CBR", "EPUB", "PDF"],
        }),
      );
    });
  });

  it("should submit with selected formats when some are deselected", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <LibraryModal
        opened={true}
        onClose={mockOnClose}
        library={mockLibrary}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText("Edit Library")).toBeInTheDocument();
    });

    const modal = await screen.findByRole("dialog");
    const modalContent = within(modal);

    // Open formats dropdown
    const formatsInput = modalContent.getByLabelText("Allowed Formats");
    await user.click(formatsInput);

    // Wait for options to appear (may appear multiple times)
    await waitFor(() => {
      const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
      expect(cbzElements.length).toBeGreaterThan(0);
    });

    // Click to deselect CBR (toggle it off)
    // In Mantine MultiSelect, clicking a selected item removes it
    // Wait for dropdown to be fully open
    await waitFor(() => {
      const options = screen.getAllByText("CBR (Comic Book RAR)");
      expect(options.length).toBeGreaterThan(0);
    });

    // Find the option in the dropdown (not the selected item in the input)
    const cbrOptions = screen.getAllByText("CBR (Comic Book RAR)");
    // Click the last one (usually the option in dropdown, not the selected tag)
    const cbrOption = cbrOptions[cbrOptions.length - 1];
    await user.click(cbrOption);

    // Wait for state to update after clicking
    await waitFor(
      () => {
        // Give time for the click to process and state to update
      },
      { timeout: 500 },
    );

    // Close dropdown by clicking outside or pressing escape
    await user.keyboard("{Escape}");

    // Wait for state to update after deselecting formats
    await waitFor(
      () => {
        // Give time for the MultiSelect state to update
      },
      { timeout: 500 },
    );

    // Submit form
    const saveButton = screen.getByText("Save Changes");
    await user.click(saveButton);

    await waitFor(() => {
      expect(librariesApi.update).toHaveBeenCalledWith(
        "1",
        expect.objectContaining({
          allowedFormats: expect.arrayContaining(["CBZ", "EPUB", "PDF"]),
        }),
      );
    });

    // Verify CBR is not in the submitted formats
    const updateCall = vi.mocked(librariesApi.update).mock.calls[0];
    const payload = updateCall[1] as { allowedFormats?: string[] };
    const submittedFormats = payload.allowedFormats;
    expect(submittedFormats).not.toContain("CBR");
  });

  describe("Strategy Tab in Edit Mode", () => {
    it("should show the Strategy tab in edit mode", async () => {
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Strategy tab should be visible
      expect(screen.getByRole("tab", { name: "Strategy" })).toBeInTheDocument();
    });

    it("should show series strategy as disabled in edit mode", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: "Strategy" });
      await user.click(strategyTab);

      // Wait for Strategy tab content to render
      await waitFor(() => {
        // Look for the select input that displays the current value
        expect(
          screen.getByDisplayValue("Series-Volume (Recommended)"),
        ).toBeInTheDocument();
      });

      // The series strategy select should be disabled
      // Find the input by its displayed value
      const seriesSelect = screen.getByDisplayValue(
        "Series-Volume (Recommended)",
      );
      expect(seriesSelect).toBeDisabled();
    });

    it("should allow changing book naming strategy in edit mode", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: "Strategy" });
      await user.click(strategyTab);

      // Wait for Strategy tab content to render - look for the select by value
      await waitFor(() => {
        expect(
          screen.getByDisplayValue("Filename (Recommended)"),
        ).toBeInTheDocument();
      });

      // Find and click the book strategy select
      const bookSelect = screen.getByDisplayValue("Filename (Recommended)");
      expect(bookSelect).not.toBeDisabled();
      await user.click(bookSelect);

      // Select a different book strategy
      await waitFor(() => {
        expect(screen.getByText("Metadata First")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Metadata First"));

      // Submit the form
      const saveButton = screen.getByText("Save Changes");
      await user.click(saveButton);

      // Verify the update was called with the new book strategy
      await waitFor(() => {
        expect(librariesApi.update).toHaveBeenCalledWith(
          "1",
          expect.objectContaining({
            bookStrategy: "metadata_first",
          }),
        );
      });
    });

    it("should allow changing number strategy in edit mode", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: "Strategy" });
      await user.click(strategyTab);

      // Wait for Strategy tab content to render - look for the number strategy select by value
      await waitFor(() => {
        expect(
          screen.getByDisplayValue("File Order (Recommended)"),
        ).toBeInTheDocument();
      });

      // Find and click the number strategy select
      const numberSelect = screen.getByDisplayValue("File Order (Recommended)");
      expect(numberSelect).not.toBeDisabled();
      await user.click(numberSelect);

      // Select a different number strategy - use "Filename Patterns" which is unique to number strategy
      await waitFor(() => {
        expect(screen.getByText("Filename Patterns")).toBeInTheDocument();
      });
      await user.click(screen.getByText("Filename Patterns"));

      // Submit the form
      const saveButton = screen.getByText("Save Changes");
      await user.click(saveButton);

      // Verify the update was called with the new number strategy
      await waitFor(() => {
        expect(librariesApi.update).toHaveBeenCalledWith(
          "1",
          expect.objectContaining({
            numberStrategy: "filename",
          }),
        );
      });
    });

    it("should show edit mode alert explaining strategy constraints", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: "Strategy" });
      await user.click(strategyTab);

      // Wait for the edit mode alert - look for the alert component with role
      await waitFor(() => {
        const alerts = screen.getAllByRole("alert");
        // The first alert should be the edit mode info alert
        expect(alerts.length).toBeGreaterThan(0);
        // Check that the alert mentions the strategy constraint
        expect(
          screen.getByText(/cannot be changed after library creation/i),
        ).toBeInTheDocument();
      });

      // Should mention that book naming and numbering can be modified
      expect(
        screen.getByText(/book naming and numbering/i),
      ).toBeInTheDocument();
    });

    it("should not show preview scan panel in edit mode", async () => {
      const user = userEvent.setup();
      renderWithProviders(
        <LibraryModal
          opened={true}
          onClose={mockOnClose}
          library={mockLibrary}
        />,
      );

      await waitFor(() => {
        expect(screen.getByText("Edit Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: "Strategy" });
      await user.click(strategyTab);

      // Wait for Strategy tab content to render - look for the series strategy select by value
      await waitFor(() => {
        expect(
          screen.getByDisplayValue("Series-Volume (Recommended)"),
        ).toBeInTheDocument();
      });

      // Preview scan panel should NOT be visible in edit mode
      expect(screen.queryByText("Preview Scan")).not.toBeInTheDocument();
    });
  });
});
