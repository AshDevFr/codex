import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent, waitFor } from "@/test/utils";
import { BulkMetadataEditModal } from "./BulkMetadataEditModal";

// Mock the bulk metadata API
const mockPatchSeriesMetadata = vi.fn().mockResolvedValue({
  updatedCount: 3,
  message: "Updated metadata for 3 series",
});
const mockPatchBookMetadata = vi.fn().mockResolvedValue({
  updatedCount: 5,
  message: "Updated metadata for 5 books",
});
const mockModifySeriesTags = vi.fn().mockResolvedValue({
  updatedCount: 3,
  message: "Updated tags for 3 series",
});
const mockModifyBookTags = vi.fn().mockResolvedValue({
  updatedCount: 5,
  message: "Updated tags for 5 books",
});
const mockModifySeriesGenres = vi.fn().mockResolvedValue({
  updatedCount: 3,
  message: "Updated genres for 3 series",
});
const mockModifyBookGenres = vi.fn().mockResolvedValue({
  updatedCount: 5,
  message: "Updated genres for 5 books",
});
const mockUpdateSeriesLocks = vi.fn().mockResolvedValue({
  updatedCount: 3,
  message: "Updated locks for 3 series",
});
const mockUpdateBookLocks = vi.fn().mockResolvedValue({
  updatedCount: 5,
  message: "Updated locks for 5 books",
});

vi.mock("@/api/bulkMetadata", () => ({
  bulkMetadataApi: {
    patchSeriesMetadata: (...args: unknown[]) =>
      mockPatchSeriesMetadata(...args),
    patchBookMetadata: (...args: unknown[]) => mockPatchBookMetadata(...args),
    modifySeriesTags: (...args: unknown[]) => mockModifySeriesTags(...args),
    modifyBookTags: (...args: unknown[]) => mockModifyBookTags(...args),
    modifySeriesGenres: (...args: unknown[]) => mockModifySeriesGenres(...args),
    modifyBookGenres: (...args: unknown[]) => mockModifyBookGenres(...args),
    updateSeriesLocks: (...args: unknown[]) => mockUpdateSeriesLocks(...args),
    updateBookLocks: (...args: unknown[]) => mockUpdateBookLocks(...args),
  },
}));

vi.mock("@/api/genres", () => ({
  genresApi: {
    getAll: vi.fn().mockResolvedValue([
      { id: "g1", name: "Action" },
      { id: "g2", name: "Comedy" },
      { id: "g3", name: "Drama" },
    ]),
  },
}));

vi.mock("@/api/tags", () => ({
  tagsApi: {
    getAll: vi.fn().mockResolvedValue([
      { id: "t1", name: "Favorite" },
      { id: "t2", name: "Completed" },
      { id: "t3", name: "Dropped" },
    ]),
  },
}));

describe("BulkMetadataEditModal", () => {
  const defaultSeriesProps = {
    opened: true,
    onClose: vi.fn(),
    selectedIds: ["series-1", "series-2", "series-3"],
    selectionType: "series" as const,
    onSuccess: vi.fn(),
  };

  const defaultBookProps = {
    opened: true,
    onClose: vi.fn(),
    selectedIds: ["book-1", "book-2", "book-3", "book-4", "book-5"],
    selectionType: "book" as const,
    onSuccess: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ===========================================================================
  // Rendering
  // ===========================================================================

  describe("rendering", () => {
    it("renders with series selection", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);
      expect(
        screen.getByText("Bulk Edit Metadata (3 series)"),
      ).toBeInTheDocument();
    });

    it("renders with book selection", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultBookProps} />);
      expect(
        screen.getByText("Bulk Edit Metadata (5 books)"),
      ).toBeInTheDocument();
    });

    it("renders four tabs", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);
      expect(
        screen.getByRole("tab", { name: /^metadata$/i }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("tab", { name: /tags & genres/i }),
      ).toBeInTheDocument();
      expect(screen.getByRole("tab", { name: /locks/i })).toBeInTheDocument();
      expect(
        screen.getByRole("tab", { name: /custom metadata/i }),
      ).toBeInTheDocument();
    });

    it("shows series-specific fields when type is series", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);
      // Use getAllByText because "Publisher" appears in metadata tab AND locks tab
      expect(screen.getAllByText("Publisher").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("Status").length).toBeGreaterThanOrEqual(1);
      expect(
        screen.getAllByText("Reading Direction").length,
      ).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("Age Rating").length).toBeGreaterThanOrEqual(
        1,
      );
    });

    it("shows book-specific fields when type is book", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultBookProps} />);
      expect(screen.getAllByText("Publisher").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("Book Type").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("Translator").length).toBeGreaterThanOrEqual(
        1,
      );
      expect(screen.getAllByText("Edition").length).toBeGreaterThanOrEqual(1);
    });

    it("does not render when opened is false", () => {
      renderWithProviders(
        <BulkMetadataEditModal {...defaultSeriesProps} opened={false} />,
      );
      expect(screen.queryByText("Bulk Edit Metadata")).not.toBeInTheDocument();
    });
  });

  // ===========================================================================
  // Metadata Tab
  // ===========================================================================

  describe("metadata tab", () => {
    it("shows modified badge when a field is changed", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      // Get the first "Leave empty to skip" input (publisher)
      const inputs = screen.getAllByPlaceholderText("Leave empty to skip");
      await user.type(inputs[0], "DC Comics");

      expect(screen.getByText("modified")).toBeInTheDocument();
    });

    it("disables apply button when no fields are modified", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);
      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      expect(applyButton).toBeDisabled();
    });

    it("enables apply button when a field is modified", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      const inputs = screen.getAllByPlaceholderText("Leave empty to skip");
      await user.type(inputs[0], "DC");

      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      expect(applyButton).toBeEnabled();
    });

    it("submits series metadata patch with only touched fields", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      // Modify only publisher (first "Leave empty to skip" input)
      const inputs = screen.getAllByPlaceholderText("Leave empty to skip");
      await user.type(inputs[0], "DC Comics");

      // Click apply
      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      await user.click(applyButton);

      await waitFor(() => {
        expect(mockPatchSeriesMetadata).toHaveBeenCalledWith(
          expect.objectContaining({
            seriesIds: ["series-1", "series-2", "series-3"],
            publisher: "DC Comics",
          }),
        );
      });
    });

    it("submits book metadata patch with only touched fields", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultBookProps} />);

      // Modify only publisher (first "Leave empty to skip" input)
      const inputs = screen.getAllByPlaceholderText("Leave empty to skip");
      await user.type(inputs[0], "Marvel");

      const applyButton = screen.getByRole("button", {
        name: /apply to 5 books/i,
      });
      await user.click(applyButton);

      await waitFor(() => {
        expect(mockPatchBookMetadata).toHaveBeenCalledWith(
          expect.objectContaining({
            bookIds: ["book-1", "book-2", "book-3", "book-4", "book-5"],
            publisher: "Marvel",
          }),
        );
      });
    });

    it("shows Add Author button and submits authors when added", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      // Click "Add Author" button
      const addAuthorButton = screen.getByRole("button", {
        name: /add author/i,
      });
      await user.click(addAuthorButton);

      // Fill in author name
      const authorInput = screen.getByPlaceholderText("Author name");
      await user.type(authorInput, "Stan Lee");

      // Apply
      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      await user.click(applyButton);

      await waitFor(() => {
        expect(mockPatchSeriesMetadata).toHaveBeenCalledWith(
          expect.objectContaining({
            seriesIds: ["series-1", "series-2", "series-3"],
            authors: [{ name: "Stan Lee", role: "author" }],
          }),
        );
      });
    });

    it("shows replace warning text for authors", () => {
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);
      expect(
        screen.getByText(
          "This will replace all authors on the selected items.",
        ),
      ).toBeInTheDocument();
    });

    it("switches to custom metadata tab and shows editor", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /custom metadata/i }));

      // The editor should be visible with its help text
      expect(screen.getByText(/merge patch semantics/i)).toBeInTheDocument();
    });
  });

  // ===========================================================================
  // Tags & Genres Tab
  // ===========================================================================

  describe("tags & genres tab", () => {
    it("switches to tags tab and shows add/remove inputs", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /tags & genres/i }));

      expect(screen.getByText("Add tags")).toBeInTheDocument();
      expect(screen.getByText("Remove tags")).toBeInTheDocument();
      expect(screen.getByText("Add genres")).toBeInTheDocument();
      expect(screen.getByText("Remove genres")).toBeInTheDocument();
    });
  });

  // ===========================================================================
  // Locks Tab
  // ===========================================================================

  describe("locks tab", () => {
    it("switches to locks tab and shows lock fields for series", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));

      // These fields appear in the locks tab for series
      expect(screen.getByText("Title")).toBeInTheDocument();
      expect(screen.getByText("Title Sort")).toBeInTheDocument();
      expect(screen.getByText("Alternate Titles")).toBeInTheDocument();
    });

    it("shows Lock All and Unlock All buttons", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));

      expect(
        screen.getByRole("button", { name: "Lock All" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "Unlock All" }),
      ).toBeInTheDocument();
    });

    it("enables apply button after Lock All is clicked", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));
      await user.click(screen.getByRole("button", { name: "Lock All" }));

      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      expect(applyButton).toBeEnabled();
    });

    it("submits series lock changes", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));
      await user.click(screen.getByRole("button", { name: "Lock All" }));

      const applyButton = screen.getByRole("button", {
        name: /apply to 3 series/i,
      });
      await user.click(applyButton);

      await waitFor(() => {
        expect(mockUpdateSeriesLocks).toHaveBeenCalledWith(
          expect.objectContaining({
            seriesIds: ["series-1", "series-2", "series-3"],
          }),
        );
      });
    });

    it("clears lock changes with Clear button", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultSeriesProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));
      await user.click(screen.getByRole("button", { name: "Lock All" }));

      // Apply should be enabled now
      expect(
        screen.getByRole("button", { name: /apply to 3 series/i }),
      ).toBeEnabled();

      // Click Clear
      await user.click(screen.getByRole("button", { name: "Clear" }));

      // Apply should be disabled again
      expect(
        screen.getByRole("button", { name: /apply to 3 series/i }),
      ).toBeDisabled();
    });

    it("shows book-specific lock fields for book selection", async () => {
      const user = userEvent.setup();
      renderWithProviders(<BulkMetadataEditModal {...defaultBookProps} />);

      await user.click(screen.getByRole("tab", { name: /locks/i }));

      expect(screen.getByText("ISBNs")).toBeInTheDocument();
      expect(screen.getByText("Subjects")).toBeInTheDocument();
      expect(screen.getByText("Volume")).toBeInTheDocument();
    });
  });

  // ===========================================================================
  // Modal behavior
  // ===========================================================================

  describe("modal behavior", () => {
    it("calls onClose when Cancel is clicked", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      renderWithProviders(
        <BulkMetadataEditModal {...defaultSeriesProps} onClose={onClose} />,
      );

      await user.click(screen.getByRole("button", { name: /cancel/i }));
      expect(onClose).toHaveBeenCalled();
    });

    it("calls onSuccess and onClose after successful save", async () => {
      const user = userEvent.setup();
      const onClose = vi.fn();
      const onSuccess = vi.fn();
      renderWithProviders(
        <BulkMetadataEditModal
          {...defaultSeriesProps}
          onClose={onClose}
          onSuccess={onSuccess}
        />,
      );

      // Make a change (first "Leave empty to skip" input is publisher)
      const inputs = screen.getAllByPlaceholderText("Leave empty to skip");
      await user.type(inputs[0], "Test");

      // Apply
      await user.click(
        screen.getByRole("button", { name: /apply to 3 series/i }),
      );

      await waitFor(() => {
        expect(onSuccess).toHaveBeenCalled();
        expect(onClose).toHaveBeenCalled();
      });
    });
  });
});
