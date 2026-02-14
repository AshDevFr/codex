import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { CoverEditor, type CoverItem } from "./CoverEditor";

const defaultProps = {
  covers: [] as CoverItem[],
  coverLocked: false,
  onCoverLockChange: vi.fn(),
  onUpload: vi.fn(),
  onSelect: vi.fn(),
  onReset: vi.fn(),
  onDelete: vi.fn(),
  getCoverImageUrl: (coverId: string) => `/covers/${coverId}/image`,
  getCoverSourceLabel: (source: string) => source,
};

const mockCovers: CoverItem[] = [
  { id: "cover-1", isSelected: true, source: "custom" },
  { id: "cover-2", isSelected: false, source: "embedded" },
  { id: "cover-3", isSelected: false, source: "plugin:openlibrary" },
];

describe("CoverEditor", () => {
  it("renders dropzone and description text", () => {
    renderWithProviders(<CoverEditor {...defaultProps} />);

    expect(
      screen.getByText(
        "Upload custom cover images or select from existing covers.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Drop image here or click to upload"),
    ).toBeInTheDocument();
  });

  it("shows unlock state by default", () => {
    renderWithProviders(<CoverEditor {...defaultProps} />);

    expect(screen.getByLabelText("Lock cover")).toBeInTheDocument();
    expect(screen.getByText("Cover selection unlocked")).toBeInTheDocument();
  });

  it("shows lock state when coverLocked is true", () => {
    renderWithProviders(<CoverEditor {...defaultProps} coverLocked={true} />);

    expect(screen.getByLabelText("Unlock cover")).toBeInTheDocument();
    expect(screen.getByText("Cover selection locked")).toBeInTheDocument();
  });

  it("toggles lock when lock button is clicked", async () => {
    const onCoverLockChange = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <CoverEditor {...defaultProps} onCoverLockChange={onCoverLockChange} />,
    );

    await user.click(screen.getByLabelText("Lock cover"));
    expect(onCoverLockChange).toHaveBeenCalledWith(true);
  });

  it("shows empty state when no covers exist", () => {
    renderWithProviders(<CoverEditor {...defaultProps} />);

    expect(
      screen.getByText("No covers uploaded yet. Upload an image above."),
    ).toBeInTheDocument();
  });

  it("renders cover grid when covers exist", () => {
    renderWithProviders(<CoverEditor {...defaultProps} covers={mockCovers} />);

    expect(screen.getByText("Available Covers")).toBeInTheDocument();
    // Each cover has a delete button
    const deleteButtons = screen.getAllByLabelText("Delete cover");
    expect(deleteButtons).toHaveLength(3);
  });

  it("shows selected badge on the selected cover", () => {
    renderWithProviders(<CoverEditor {...defaultProps} covers={mockCovers} />);

    // Only one cover should show "Selected"
    expect(screen.getByText("Selected")).toBeInTheDocument();
  });

  it("shows reset button when a cover is selected", () => {
    renderWithProviders(<CoverEditor {...defaultProps} covers={mockCovers} />);

    expect(
      screen.getByRole("button", { name: /Reset to Default Cover/i }),
    ).toBeInTheDocument();
  });

  it("hides reset button when no cover is selected", () => {
    const unselectedCovers = mockCovers.map((c) => ({
      ...c,
      isSelected: false,
    }));

    renderWithProviders(
      <CoverEditor {...defaultProps} covers={unselectedCovers} />,
    );

    expect(
      screen.queryByRole("button", { name: /Reset to Default Cover/i }),
    ).not.toBeInTheDocument();
  });

  it("shows default cover message when no cover is selected", () => {
    const unselectedCovers = mockCovers.map((c) => ({
      ...c,
      isSelected: false,
    }));

    renderWithProviders(
      <CoverEditor
        {...defaultProps}
        covers={unselectedCovers}
        defaultCoverMessage="Using default (embedded cover)"
      />,
    );

    expect(
      screen.getByText("Using default (embedded cover)"),
    ).toBeInTheDocument();
  });

  it("calls onReset when reset button is clicked", async () => {
    const onReset = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <CoverEditor {...defaultProps} covers={mockCovers} onReset={onReset} />,
    );

    await user.click(
      screen.getByRole("button", { name: /Reset to Default Cover/i }),
    );
    expect(onReset).toHaveBeenCalled();
  });

  it("calls onDelete when delete button is clicked", async () => {
    const onDelete = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <CoverEditor {...defaultProps} covers={mockCovers} onDelete={onDelete} />,
    );

    const deleteButtons = screen.getAllByLabelText("Delete cover");
    await user.click(deleteButtons[0]);
    expect(onDelete).toHaveBeenCalledWith("cover-1");
  });

  it("calls onSelect when an unselected cover is clicked", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();

    renderWithProviders(
      <CoverEditor {...defaultProps} covers={mockCovers} onSelect={onSelect} />,
    );

    // The source labels are rendered via getCoverSourceLabel
    // cover-2 is unselected, click its card area (the source label text)
    const sourceLabels = screen.getAllByText(
      /embedded|custom|plugin:openlibrary/,
    );
    // Find the "embedded" one (cover-2, unselected)
    const embeddedLabel = sourceLabels.find(
      (el) => el.textContent === "embedded",
    );
    // Click the parent card
    if (embeddedLabel) {
      const card = embeddedLabel.closest("[class*='Card']");
      if (card) {
        await user.click(card as HTMLElement);
      }
    }

    expect(onSelect).toHaveBeenCalledWith("cover-2");
  });

  it("uses custom resetButtonLabel", () => {
    renderWithProviders(
      <CoverEditor
        {...defaultProps}
        covers={mockCovers}
        resetButtonLabel="Reset to Default (Use First Book Cover)"
      />,
    );

    expect(
      screen.getByRole("button", {
        name: /Reset to Default \(Use First Book Cover\)/i,
      }),
    ).toBeInTheDocument();
  });

  it("uses getCoverSourceLabel to display source names", () => {
    const getCoverSourceLabel = (source: string) => {
      if (source === "custom") return "Custom Upload";
      if (source === "embedded") return "Embedded";
      return source;
    };

    renderWithProviders(
      <CoverEditor
        {...defaultProps}
        covers={mockCovers}
        getCoverSourceLabel={getCoverSourceLabel}
      />,
    );

    expect(screen.getByText("Custom Upload")).toBeInTheDocument();
    expect(screen.getByText("Embedded")).toBeInTheDocument();
  });

  it("uses getCoverImageUrl for cover images", () => {
    const getCoverImageUrl = (coverId: string) =>
      `/api/v1/books/123/covers/${coverId}/image`;

    renderWithProviders(
      <CoverEditor
        {...defaultProps}
        covers={mockCovers}
        getCoverImageUrl={getCoverImageUrl}
      />,
    );

    const coverImages = screen.getAllByAltText("Cover");
    expect(coverImages[0]).toHaveAttribute(
      "src",
      "/api/v1/books/123/covers/cover-1/image",
    );
  });
});
