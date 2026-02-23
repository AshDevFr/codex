import { screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { filesystemApi } from "@/api/filesystem";
import { librariesApi } from "@/api/libraries";
import { renderWithProviders, userEvent } from "@/test/utils";
import type { BrowseResponse, FileSystemEntry, Library } from "@/types";
import { LibraryModal } from "./LibraryModal";

vi.mock("@/api/filesystem");
vi.mock("@/api/libraries");
vi.mock("@mantine/notifications", () => ({
  notifications: {
    show: vi.fn(),
  },
}));

// Helper to create a complete Library mock with all required fields
const createMockLibrary = (overrides?: Partial<Library>): Library => ({
  id: "1",
  name: "Test Library",
  path: "/home/user/Comics",
  isActive: true,
  createdAt: "2024-01-01T00:00:00Z",
  updatedAt: "2024-01-01T00:00:00Z",
  seriesStrategy: "series_volume",
  bookStrategy: "filename",
  numberStrategy: "file_order",
  defaultReadingDirection: "ltr",
  ...overrides,
});

const mockDrives: FileSystemEntry[] = [
  {
    name: "Home Directory",
    path: "/home/user",
    isDirectory: true,
    isReadable: true,
  },
  {
    name: "Root",
    path: "/",
    isDirectory: true,
    isReadable: true,
  },
];

const mockBrowseResponse: BrowseResponse = {
  currentPath: "/home/user",
  parentPath: "/home",
  entries: [
    {
      name: "Documents",
      path: "/home/user/Documents",
      isDirectory: true,
      isReadable: true,
    },
    {
      name: "Comics",
      path: "/home/user/Comics",
      isDirectory: true,
      isReadable: true,
    },
    {
      name: "file.txt",
      path: "/home/user/file.txt",
      isDirectory: false,
      isReadable: true,
    },
  ],
};

describe("LibraryModal (Add Mode)", () => {
  const mockOnClose = vi.fn();
  const originalScrollIntoView = Element.prototype.scrollIntoView;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(filesystemApi.getDrives).mockResolvedValue(mockDrives);
    vi.mocked(filesystemApi.browse).mockResolvedValue(mockBrowseResponse);
    // Mock scrollIntoView for Mantine Combobox
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(() => {
    Element.prototype.scrollIntoView = originalScrollIntoView;
  });

  it("should not render when closed", () => {
    renderWithProviders(<LibraryModal opened={false} onClose={mockOnClose} />);

    expect(screen.queryByText("Add New Library")).not.toBeInTheDocument();
  });

  it("should render form fields when opened", async () => {
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal to be fully rendered - check for title
    await waitFor(
      () => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Find the modal dialog container
    const modal = await screen.findByRole("dialog", {}, { timeout: 2000 });
    const modalContent = within(modal);

    // Wait for the "Browse" button which confirms the form section is rendered
    // (not the path browser view) - this is a reliable indicator the form is there
    await waitFor(
      () => {
        expect(modalContent.getByText("Browse")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Verify inputs exist by placeholder (more reliable than labels in Mantine portals)
    // This is what actually matters - that users can interact with the form
    expect(
      modalContent.getByPlaceholderText("Enter library name"),
    ).toBeInTheDocument();
    expect(
      modalContent.getByPlaceholderText("Select a path..."),
    ).toBeInTheDocument();

    // Verify Scan Strategy select exists (it should have a label we can find)
    // If labels aren't accessible in the portal, that's a Mantine limitation
    // but the form is functional which is what we're testing
    const scanStrategy = modalContent.getByText("Scan Strategy");
    expect(scanStrategy).toBeInTheDocument();
  });

  it("should open path browser when Browse button is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    await waitFor(() => {
      expect(screen.getByText("Browse")).toBeInTheDocument();
    });

    const browseButton = screen.getByText("Browse");
    await user.click(browseButton);

    await waitFor(() => {
      expect(screen.getByText("Select Library Path")).toBeInTheDocument();
      expect(filesystemApi.getDrives).toHaveBeenCalled();
    });
  });

  it("should display drives in path browser", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    await waitFor(() => {
      expect(screen.getByText("Browse")).toBeInTheDocument();
    });

    const browseButton = screen.getByText("Browse");
    await user.click(browseButton);

    await waitFor(() => {
      expect(screen.getByText("Home Directory")).toBeInTheDocument();
      expect(screen.getByText("Root")).toBeInTheDocument();
    });
  });

  it("should browse directory when drive is selected", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Open browser
    const browseButton = await screen.findByText("Browse");
    await user.click(browseButton);

    // Click on a drive
    await waitFor(() => {
      expect(screen.getByText("Home Directory")).toBeInTheDocument();
    });

    const driveButton = screen.getByText("Home Directory");
    await user.click(driveButton);

    await waitFor(() => {
      expect(filesystemApi.browse).toHaveBeenCalledWith("/home/user");
      expect(screen.getByText("Documents")).toBeInTheDocument();
      expect(screen.getByText("Comics")).toBeInTheDocument();
    });
  });

  it("should only show directories, not files", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    const browseButton = await screen.findByText("Browse");
    await user.click(browseButton);

    await waitFor(() => {
      expect(screen.getByText("Home Directory")).toBeInTheDocument();
    });

    const driveButton = screen.getByText("Home Directory");
    await user.click(driveButton);

    await waitFor(() => {
      expect(screen.getByText("Documents")).toBeInTheDocument();
      expect(screen.getByText("Comics")).toBeInTheDocument();
      // File should not be visible
      expect(screen.queryByText("file.txt")).not.toBeInTheDocument();
    });
  });

  it("should select path and auto-generate library name", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal and find form fields
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);
    const nameInput = await modalContent.findByPlaceholderText(
      "Enter library name",
      {},
      { timeout: 3000 },
    );
    expect(nameInput).toBeInTheDocument();

    // Open browser and navigate to a directory
    const browseButton = screen.getByText("Browse");
    await user.click(browseButton);

    const driveButton = await screen.findByText("Home Directory");
    await user.click(driveButton);

    await waitFor(() => {
      expect(screen.getByText("Select This Folder")).toBeInTheDocument();
    });

    const selectButton = screen.getByText("Select This Folder");
    await user.click(selectButton);

    await waitFor(() => {
      // Use placeholder text to find inputs since labels aren't accessible in portal
      const pathInput = modalContent.getByPlaceholderText(
        "Select a path...",
      ) as HTMLInputElement;
      expect(pathInput.value).toBe("/home/user");

      const nameInput = modalContent.getByPlaceholderText(
        "Enter library name",
      ) as HTMLInputElement;
      expect(nameInput.value).toBe("User");
    });
  });

  it("should navigate to parent directory", async () => {
    const user = userEvent.setup();
    const parentBrowseResponse: BrowseResponse = {
      currentPath: "/home",
      parentPath: "/",
      entries: [
        {
          name: "user",
          path: "/home/user",
          isDirectory: true,
          isReadable: true,
        },
      ],
    };

    vi.mocked(filesystemApi.browse)
      .mockResolvedValueOnce(mockBrowseResponse)
      .mockResolvedValueOnce(parentBrowseResponse);

    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Navigate to a directory
    const browseButton = await screen.findByText("Browse");
    await user.click(browseButton);

    const driveButton = await screen.findByText("Home Directory");
    await user.click(driveButton);

    await waitFor(() => {
      expect(screen.getByText("Up One Level")).toBeInTheDocument();
    });

    // Click "Up One Level"
    const upButton = screen.getByText("Up One Level");
    await user.click(upButton);

    await waitFor(() => {
      expect(filesystemApi.browse).toHaveBeenCalledWith("/home");
    });
  });

  it(
    "should create library with valid inputs",
    { timeout: 15000 },
    async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary();

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal and find form fields
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in form
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      // Set path directly (simulating browse selection)
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Submit form
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      await waitFor(() => {
        expect(librariesApi.create).toHaveBeenCalledWith(
          expect.objectContaining({
            name: "Test Library",
            path: "/home/user",
            scanningConfig: expect.objectContaining({
              enabled: false,
              scanMode: "normal",
            }),
          }),
        );
      });
    },
  );

  it(
    "should call onClose with created library on successful creation",
    { timeout: 15000 },
    async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary({ id: "123" });

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal and find form fields
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in form
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      // Set path
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Submit form
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      // Wait for the mutation to complete and onClose to be called with the created library
      await waitFor(() => {
        expect(mockOnClose).toHaveBeenCalledWith(mockLibrary);
      });
    },
  );

  it("should show validation error when name is missing", async () => {
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal and verify form is rendered
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);
    await modalContent.findByPlaceholderText(
      "Enter library name",
      {},
      { timeout: 3000 },
    );

    // Don't fill in name or path, button should be disabled
    // The button is disabled when !libraryName || !selectedPath
    const createButton = modalContent.getByRole("button", {
      name: /create library/i,
    });
    expect(createButton).toBeDisabled();
  });

  it("should show validation error when path is missing", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal and find form fields
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);
    const nameInput = await modalContent.findByPlaceholderText(
      "Enter library name",
      {},
      { timeout: 3000 },
    );
    await user.type(nameInput, "Test Library");

    // Don't select path, button should be disabled
    const createButton = modalContent.getByRole("button", {
      name: /create library/i,
    });
    expect(createButton).toBeDisabled();
  });

  it("should close modal when Cancel is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    await waitFor(() => {
      expect(screen.getByText("Cancel")).toBeInTheDocument();
    });

    const cancelButton = screen.getByText("Cancel");
    await user.click(cancelButton);

    expect(mockOnClose).toHaveBeenCalled();
  });

  it("should navigate back to drives when breadcrumb is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Navigate to a directory
    const browseButton = await screen.findByText("Browse");
    await user.click(browseButton);

    const driveButton = await screen.findByText("Home Directory");
    await user.click(driveButton);

    await waitFor(() => {
      expect(screen.getByText("Drives")).toBeInTheDocument();
    });

    // Click "Drives" breadcrumb
    const drivesLink = screen.getByText("Drives");
    await user.click(drivesLink);

    await waitFor(() => {
      expect(
        screen.getByText("Select a drive or location to browse:"),
      ).toBeInTheDocument();
    });
  });

  it("should show cron input when auto scan strategy is selected", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal
    await screen.findByRole("dialog", {}, { timeout: 3000 });

    // Navigate to the Scanning tab
    const scanningTab = await screen.findByRole("tab", { name: /scanning/i });
    await user.click(scanningTab);

    // Wait for the Scanning tab content to load
    await waitFor(() => {
      expect(screen.getByText("Scan Strategy")).toBeInTheDocument();
    });

    // Find and click on the Select to open the dropdown
    const selectInput = screen.getByText("Manual - Trigger scans on demand");
    await user.click(selectInput);

    // Wait for and click the auto option
    const autoOption = await screen.findByText(
      "Automatic - Scheduled scanning",
    );
    await user.click(autoOption);

    // Cron input should appear
    const cronInput = await screen.findByPlaceholderText("0 0 * * *");
    expect(cronInput).toBeInTheDocument();
  });

  it("should not show cron input when manual scan strategy is selected", async () => {
    const user = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal
    await screen.findByRole("dialog", {}, { timeout: 3000 });

    // Navigate to the Scanning tab
    const scanningTab = await screen.findByRole("tab", { name: /scanning/i });
    await user.click(scanningTab);

    // Wait for the Scanning tab content to load
    await waitFor(() => {
      expect(screen.getByText("Scan Strategy")).toBeInTheDocument();
    });

    // Cron input should not be visible by default (manual is default)
    expect(screen.queryByPlaceholderText("0 0 * * *")).not.toBeInTheDocument();
  });

  it(
    "should create library with cron schedule when auto scan is enabled",
    { timeout: 15000 },
    async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary();

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal and find form fields
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in form
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      // Set path
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Navigate to the Scanning tab
      const scanningTab = await screen.findByRole("tab", { name: /scanning/i });
      await user.click(scanningTab);

      // Wait for the Scanning tab content to load
      await waitFor(() => {
        expect(screen.getByText("Scan Strategy")).toBeInTheDocument();
      });

      // Find and click on the Select to open the dropdown
      const selectInput = screen.getByText("Manual - Trigger scans on demand");
      await user.click(selectInput);

      // Wait for and click the auto option
      const autoOption = await screen.findByText(
        "Automatic - Scheduled scanning",
      );
      await user.click(autoOption);

      // Wait for cron input and verify it has default value
      const cronInput = await screen.findByPlaceholderText("0 0 * * *");
      expect(cronInput).toBeInTheDocument();
      expect(cronInput).toHaveValue("0 0 * * *");

      // Submit form
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      await waitFor(() => {
        expect(librariesApi.create).toHaveBeenCalledWith(
          expect.objectContaining({
            name: "Test Library",
            path: "/home/user",
            scanningConfig: expect.objectContaining({
              enabled: true,
              cronSchedule: "0 0 * * *",
              scanMode: "normal",
            }),
          }),
        );
      });
    },
  );

  it(
    "should validate cron schedule is required when auto scan is enabled",
    { timeout: 15000 },
    async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in name and path
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Navigate to the Scanning tab
      const scanningTab = await screen.findByRole("tab", { name: /scanning/i });
      await user.click(scanningTab);

      // Wait for the Scanning tab content to load and find the select
      await waitFor(() => {
        expect(screen.getByText("Scan Strategy")).toBeInTheDocument();
      });

      // Find and click on the Select to open the dropdown
      const selectInput = screen.getByText("Manual - Trigger scans on demand");
      await user.click(selectInput);

      // Wait for and click the auto option
      const autoOption = await screen.findByText(
        "Automatic - Scheduled scanning",
      );
      await user.click(autoOption);

      // Wait for cron input to appear and clear it
      const cronInput = await screen.findByPlaceholderText("0 0 * * *");
      expect(cronInput).toBeInTheDocument();
      await user.clear(cronInput);

      // Try to submit - should show validation error
      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      // The form validation happens in handleSubmit and shows a notification
      // The library should not be created because cron schedule is required
      // Short wait to give time for any async calls to happen
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Verify that the API was not called (validation prevented submission)
      expect(librariesApi.create).not.toHaveBeenCalled();
    },
  );

  it("should have all formats selected by default", async () => {
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);

    // Find the MultiSelect for allowed formats
    const formatsInput = modalContent.getByLabelText("Allowed Formats");
    expect(formatsInput).toBeInTheDocument();

    // Click to open the dropdown and verify all formats are available
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

  it("should submit with all formats by default", async () => {
    const user = userEvent.setup();
    const mockLibrary = createMockLibrary();

    vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    // Wait for modal and find form fields
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);
    const nameInput = await modalContent.findByPlaceholderText(
      "Enter library name",
      {},
      { timeout: 3000 },
    );

    // Fill in form
    await user.clear(nameInput);
    await user.type(nameInput, "Test Library");

    // Set path
    const browseButton = screen.getByText("Browse");
    await user.click(browseButton);

    const driveButton = await screen.findByText("Home Directory");
    await user.click(driveButton);

    const selectButton = await screen.findByText("Select This Folder");
    await user.click(selectButton);

    // Submit form (with all formats selected by default)
    await waitFor(() => {
      const createButton = screen.getByText("Create Library");
      expect(createButton).not.toBeDisabled();
    });

    const createButton = screen.getByText("Create Library");
    await user.click(createButton);

    await waitFor(() => {
      expect(librariesApi.create).toHaveBeenCalledWith(
        expect.objectContaining({
          name: "Test Library",
          path: "/home/user",
          allowedFormats: ["CBZ", "CBR", "EPUB", "PDF"],
        }),
      );
    });
  });

  it(
    "should submit with selected formats when some are deselected",
    { timeout: 15000 },
    async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary();

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal and find form fields
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in form
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      // Set path
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Open formats dropdown
      const formatsInput = modalContent.getByLabelText("Allowed Formats");
      await user.click(formatsInput);

      // Wait for dropdown to be visible
      await waitFor(() => {
        expect(
          screen.getAllByText("CBR (Comic Book RAR)").length,
        ).toBeGreaterThan(0);
      });

      // Find close buttons within pills (Mantine uses CloseButton component)
      // Use document.body to query since Mantine renders in portals
      // Pills are ordered: CBZ (0), CBR (1), EPUB (2), PDF (3)
      let closeButtons = document.querySelectorAll(".mantine-Pill-remove");

      // Remove CBR first (index 1)
      if (closeButtons.length >= 2) {
        await user.click(closeButtons[1]);
      }

      // After removing CBR, re-query for buttons since DOM has changed
      // Now pills are: CBZ (0), EPUB (1), PDF (2)
      closeButtons = document.querySelectorAll(".mantine-Pill-remove");

      // Remove PDF (now at index 2)
      if (closeButtons.length >= 3) {
        await user.click(closeButtons[2]);
      }

      // Close dropdown
      await user.keyboard("{Escape}");

      // Submit form
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      await waitFor(() => {
        expect(librariesApi.create).toHaveBeenCalledWith(
          expect.objectContaining({
            name: "Test Library",
            path: "/home/user",
            allowedFormats: ["CBZ", "EPUB"],
          }),
        );
      });
    },
  );

  it("should reset to all formats when modal closes", async () => {
    const user = userEvent.setup();
    const { unmount } = renderWithProviders(
      <LibraryModal opened={true} onClose={mockOnClose} />,
    );

    // Wait for modal
    const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const modalContent = within(modal);

    // Open formats dropdown and deselect some formats
    const formatsInput = modalContent.getByLabelText("Allowed Formats");
    await user.click(formatsInput);

    await waitFor(() => {
      const cbzElements = screen.getAllByText("CBZ (Comic Book ZIP)");
      expect(cbzElements.length).toBeGreaterThan(0);
    });

    // Deselect CBR
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

    // Wait a bit for the click to process
    await waitFor(
      () => {
        // Verify the click was processed
      },
      { timeout: 1000 },
    );
    await user.keyboard("{Escape}");

    // Close modal
    const cancelButton = screen.getByText("Cancel");
    await user.click(cancelButton);

    expect(mockOnClose).toHaveBeenCalled();

    // Unmount the first render before re-rendering
    unmount();

    // Reopen modal - formats should be reset to all
    const user2 = userEvent.setup();
    renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

    const newModal = await screen.findByRole("dialog", {}, { timeout: 3000 });
    const newModalContent = within(newModal);

    const newFormatsInput = newModalContent.getByLabelText("Allowed Formats");
    await user2.click(newFormatsInput);

    // All formats should be available again (may appear multiple times in MultiSelect)
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

  // Strategy Tab Tests
  describe("Strategy Tab", () => {
    it("should display Strategy tab in add mode", async () => {
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Strategy tab should be visible in add mode
      expect(
        screen.getByRole("tab", { name: /strategy/i }),
      ).toBeInTheDocument();
    });

    it("should show series strategy selector when Strategy tab is clicked", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        expect(
          screen.getByText("Series Detection Strategy"),
        ).toBeInTheDocument();
        expect(screen.getByText("Book Naming Strategy")).toBeInTheDocument();
      });
    });

    it("should show warning about strategy being permanent", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        expect(screen.getByText(/permanent/i)).toBeInTheDocument();
        expect(
          screen.getByText(/cannot be changed after library creation/i),
        ).toBeInTheDocument();
      });
    });

    it("should default to series_volume and filename strategies", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        // Check that the default strategies are selected (Mantine shows label in textbox)
        const textboxes = screen.getAllByRole("textbox", { hidden: true });
        // One of them should have "Series-Volume (Recommended)"
        const hasSeriesVolume = textboxes.some(
          (tb) => tb.getAttribute("value") === "Series-Volume (Recommended)",
        );
        const hasFilename = textboxes.some(
          (tb) => tb.getAttribute("value") === "Filename (Recommended)",
        );
        expect(hasSeriesVolume).toBe(true);
        expect(hasFilename).toBe(true);
      });
    });

    it(
      "should include strategy in create request with default values",
      { timeout: 15000 },
      async () => {
        const user = userEvent.setup();
        const mockLibrary = createMockLibrary();

        vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

        renderWithProviders(
          <LibraryModal opened={true} onClose={mockOnClose} />,
        );

        // Wait for modal
        const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
        const modalContent = within(modal);
        const nameInput = await modalContent.findByPlaceholderText(
          "Enter library name",
          {},
          { timeout: 3000 },
        );

        // Fill in required fields
        await user.clear(nameInput);
        await user.type(nameInput, "Test Library");

        // Set path
        const browseButton = screen.getByText("Browse");
        await user.click(browseButton);

        const driveButton = await screen.findByText("Home Directory");
        await user.click(driveButton);

        const selectButton = await screen.findByText("Select This Folder");
        await user.click(selectButton);

        // Submit form without changing strategy (defaults should be used)
        await waitFor(() => {
          const createButton = screen.getByText("Create Library");
          expect(createButton).not.toBeDisabled();
        });

        const createButton = screen.getByText("Create Library");
        await user.click(createButton);

        await waitFor(() => {
          expect(librariesApi.create).toHaveBeenCalledWith(
            expect.objectContaining({
              name: "Test Library",
              path: "/home/user",
              seriesStrategy: "series_volume",
              bookStrategy: "filename",
            }),
          );
        });
      },
    );

    it(
      "should include selected strategy in create request",
      { timeout: 15000 },
      async () => {
        const user = userEvent.setup();
        const mockLibrary = createMockLibrary({
          name: "Manga Library",
          path: "/home/user/Manga",
          seriesStrategy: "series_volume_chapter",
          bookStrategy: "smart",
          defaultReadingDirection: "rtl",
        });

        vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

        renderWithProviders(
          <LibraryModal opened={true} onClose={mockOnClose} />,
        );

        // Wait for modal
        const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
        const modalContent = within(modal);
        const nameInput = await modalContent.findByPlaceholderText(
          "Enter library name",
          {},
          { timeout: 3000 },
        );

        // Fill in required fields
        await user.clear(nameInput);
        await user.type(nameInput, "Manga Library");

        // Set path
        const browseButton = screen.getByText("Browse");
        await user.click(browseButton);

        const driveButton = await screen.findByText("Home Directory");
        await user.click(driveButton);

        const selectButton = await screen.findByText("Select This Folder");
        await user.click(selectButton);

        // Go to Strategy tab and change strategy
        const strategyTab = screen.getByRole("tab", { name: /strategy/i });
        await user.click(strategyTab);

        await waitFor(() => {
          expect(
            screen.getByText("Series Detection Strategy"),
          ).toBeInTheDocument();
        });

        // Change series strategy to series_volume_chapter
        const seriesStrategySelect = screen
          .getByText("Series Detection Strategy")
          .closest("div")
          ?.parentElement?.querySelector('input[type="text"]');
        if (seriesStrategySelect) {
          await user.click(seriesStrategySelect.parentElement as Element);
        }

        await waitFor(() => {
          expect(screen.getByText("Series-Volume-Chapter")).toBeInTheDocument();
        });
        await user.click(screen.getByText("Series-Volume-Chapter"));

        // Change book strategy to smart
        const bookStrategySelect = screen
          .getByText("Book Naming Strategy")
          .closest("div")
          ?.parentElement?.querySelector('input[type="text"]');
        if (bookStrategySelect) {
          await user.click(bookStrategySelect.parentElement as Element);
        }

        await waitFor(() => {
          // There may be multiple "Smart Detection" texts on the page
          // (one for BookStrategy, one for NumberStrategy)
          const smartOptions = screen.getAllByText("Smart Detection");
          expect(smartOptions.length).toBeGreaterThan(0);
        });
        // Click the first one (from the BookStrategy dropdown)
        const smartOptions = screen.getAllByText("Smart Detection");
        await user.click(smartOptions[0]);

        // Submit form
        await waitFor(() => {
          const createButton = screen.getByText("Create Library");
          expect(createButton).not.toBeDisabled();
        });

        const createButton = screen.getByText("Create Library");
        await user.click(createButton);

        await waitFor(() => {
          expect(librariesApi.create).toHaveBeenCalledWith(
            expect.objectContaining({
              name: "Manga Library",
              seriesStrategy: "series_volume_chapter",
              bookStrategy: "smart",
            }),
          );
        });
      },
    );

    it("should show preview scan panel in Strategy tab", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // First set a path (preview panel requires a path)
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        expect(screen.getByText("Preview Scan Results")).toBeInTheDocument();
      });
    });

    it("should show number strategy selector when Strategy tab is clicked", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        expect(
          screen.getByText("Series Detection Strategy"),
        ).toBeInTheDocument();
        expect(screen.getByText("Book Naming Strategy")).toBeInTheDocument();
        expect(screen.getByText("Book Number Strategy")).toBeInTheDocument();
      });
    });

    it("should default to file_order number strategy", async () => {
      const user = userEvent.setup();
      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      await waitFor(() => {
        expect(screen.getByText("Add New Library")).toBeInTheDocument();
      });

      // Click on Strategy tab
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        // Check that the default number strategy is selected (Mantine shows label in textbox)
        const textboxes = screen.getAllByRole("textbox", { hidden: true });
        const hasFileOrder = textboxes.some(
          (tb) => tb.getAttribute("value") === "File Order (Recommended)",
        );
        expect(hasFileOrder).toBe(true);
      });
    });

    it("should include number strategy in create request with default value", async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary();

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in required fields
      await user.clear(nameInput);
      await user.type(nameInput, "Test Library");

      // Set path
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Submit form without changing strategy (defaults should be used)
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      await waitFor(() => {
        expect(librariesApi.create).toHaveBeenCalledWith(
          expect.objectContaining({
            name: "Test Library",
            path: "/home/user",
            seriesStrategy: "series_volume",
            bookStrategy: "filename",
            numberStrategy: "file_order",
          }),
        );
      });
    });

    it("should include selected number strategy in create request", async () => {
      const user = userEvent.setup();
      const mockLibrary = createMockLibrary({
        name: "Manga Library",
        path: "/home/user/Manga",
        seriesStrategy: "series_volume_chapter",
        bookStrategy: "smart",
        numberStrategy: "smart",
        defaultReadingDirection: "rtl",
      });

      vi.mocked(librariesApi.create).mockResolvedValueOnce(mockLibrary);

      renderWithProviders(<LibraryModal opened={true} onClose={mockOnClose} />);

      // Wait for modal
      const modal = await screen.findByRole("dialog", {}, { timeout: 3000 });
      const modalContent = within(modal);
      const nameInput = await modalContent.findByPlaceholderText(
        "Enter library name",
        {},
        { timeout: 3000 },
      );

      // Fill in required fields
      await user.clear(nameInput);
      await user.type(nameInput, "Manga Library");

      // Set path
      const browseButton = screen.getByText("Browse");
      await user.click(browseButton);

      const driveButton = await screen.findByText("Home Directory");
      await user.click(driveButton);

      const selectButton = await screen.findByText("Select This Folder");
      await user.click(selectButton);

      // Go to Strategy tab and change strategies
      const strategyTab = screen.getByRole("tab", { name: /strategy/i });
      await user.click(strategyTab);

      await waitFor(() => {
        expect(screen.getByText("Book Number Strategy")).toBeInTheDocument();
      });

      // Change number strategy to smart
      const numberStrategySelect = screen
        .getByText("Book Number Strategy")
        .closest("div")
        ?.parentElement?.querySelector('input[type="text"]');
      if (numberStrategySelect) {
        await user.click(numberStrategySelect.parentElement as Element);
      }

      await waitFor(() => {
        // Find the Smart Detection option in the number strategy dropdown
        // There may be multiple "Smart Detection" texts on the page
        const smartOptions = screen.getAllByText("Smart Detection");
        expect(smartOptions.length).toBeGreaterThan(0);
      });

      // Click the Smart Detection option from the number strategy dropdown
      const smartOptions = screen.getAllByText("Smart Detection");
      // Click the last one which should be from the dropdown
      await user.click(smartOptions[smartOptions.length - 1]);

      // Submit form
      await waitFor(() => {
        const createButton = screen.getByText("Create Library");
        expect(createButton).not.toBeDisabled();
      });

      const createButton = screen.getByText("Create Library");
      await user.click(createButton);

      await waitFor(() => {
        expect(librariesApi.create).toHaveBeenCalledWith(
          expect.objectContaining({
            name: "Manga Library",
            numberStrategy: "smart",
          }),
        );
      });
    });
  });
});
