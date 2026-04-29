import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { Book } from "@/types";
import { SeriesBookTable } from "./SeriesBookTable";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual =
    await vi.importActual<typeof import("react-router-dom")>(
      "react-router-dom",
    );
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

function makeBook(overrides: Partial<Book> = {}): Book {
  return {
    id: "book-1",
    libraryId: "lib-1",
    libraryName: "Comics",
    seriesId: "series-1",
    seriesName: "Test Series",
    title: "Test Book Title That Is Quite Long Indeed",
    number: 1,
    pageCount: 32,
    fileFormat: "cbz",
    fileHash: "hash",
    filePath: "/x.cbz",
    fileSize: 1000,
    deleted: false,
    analyzed: true,
    createdAt: "2024-01-15T10:30:00Z",
    updatedAt: "2024-01-15T10:30:00Z",
    ...overrides,
  } as Book;
}

describe("SeriesBookTable", () => {
  beforeEach(() => {
    mockNavigate.mockReset();
  });

  it("renders rows with full untruncated titles", () => {
    const books = [
      makeBook({ id: "b1", title: "Volume One: A Very Long Subtitle Here" }),
      makeBook({ id: "b2", number: 2, title: "Volume Two" }),
    ];

    renderWithProviders(
      <SeriesBookTable
        books={books}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(
      screen.getByText("Volume One: A Very Long Subtitle Here"),
    ).toBeInTheDocument();
    expect(screen.getByText("Volume Two")).toBeInTheDocument();
  });

  it("renders en-dash for missing book number", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1", number: null, title: "No Number" })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("renders the selection checkbox column even when not in selection mode", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1", title: "First" })]}
        onSelect={onSelect}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    const checkbox = screen.getByLabelText(/Select First/i);
    expect(checkbox).toBeInTheDocument();

    // Clicking the checkbox triggers selection (which globally flips selection mode on)
    await user.click(checkbox);
    expect(onSelect).toHaveBeenCalledWith("b1", false, 0);
    // No navigation occurred because the checkbox cell stops propagation
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("forwards shiftKey on row click while in selection mode", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const books = [
      makeBook({ id: "b1", title: "First" }),
      makeBook({ id: "b2", title: "Second" }),
    ];

    renderWithProviders(
      <SeriesBookTable
        books={books}
        onSelect={onSelect}
        selectedIds={new Set()}
        isSelectionMode={true}
        canBeSelected={true}
      />,
    );

    // Plain click on the row
    await user.click(screen.getByTestId("series-book-row-b1"));
    expect(onSelect).toHaveBeenCalledWith("b1", false, 0);

    // Shift-click on the second row
    await user.keyboard("{Shift>}");
    await user.click(screen.getByTestId("series-book-row-b2"));
    await user.keyboard("{/Shift}");
    expect(onSelect).toHaveBeenCalledWith("b2", true, 1);

    // No navigation in selection mode
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it("renders a Read action button that navigates to the reader from page 1 when no progress", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1", title: "Fresh Book" })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    const readButton = screen.getByRole("button", { name: /Read Fresh Book/i });
    await user.click(readButton);
    expect(mockNavigate).toHaveBeenCalledWith("/reader/b1?page=1");
  });

  it("renders a Continue button that resumes from currentPage when progress exists", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SeriesBookTable
        books={[
          makeBook({
            id: "b1",
            title: "In Progress Book",
            pageCount: 100,
            readProgress: {
              id: "p1",
              currentPage: 42,
              progressPercentage: 0.42,
              completed: false,
              startedAt: "2024-01-01T00:00:00Z",
            } as Book["readProgress"],
          }),
        ]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    const continueButton = screen.getByRole("button", {
      name: /Continue reading In Progress Book/i,
    });
    await user.click(continueButton);
    expect(mockNavigate).toHaveBeenCalledWith("/reader/b1?page=42");
  });

  it("does not render the Read button for soft-deleted books", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1", title: "Gone Book", deleted: true })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(
      screen.queryByRole("button", { name: /Read Gone Book/i }),
    ).not.toBeInTheDocument();
  });

  it("shows a Read badge for completed books", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[
          makeBook({
            id: "b1",
            readProgress: {
              id: "p1",
              currentPage: 100,
              completed: true,
              startedAt: "2024-01-01T00:00:00Z",
            } as Book["readProgress"],
          }),
        ]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(screen.getByText("Read")).toBeInTheDocument();
  });

  it("shows an Unread badge for books with no progress", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1" })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(screen.getByText("Unread")).toBeInTheDocument();
  });

  it("shows a percentage for in-progress books", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[
          makeBook({
            id: "b1",
            pageCount: 100,
            readProgress: {
              id: "p1",
              currentPage: 30,
              progressPercentage: 0.3,
              completed: false,
              startedAt: "2024-01-01T00:00:00Z",
            } as Book["readProgress"],
          }),
        ]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(screen.getByText("30%")).toBeInTheDocument();
  });

  it("navigates to book detail when clicking a row outside selection mode", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1" })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    await user.click(screen.getByTestId("series-book-row-b1"));
    expect(mockNavigate).toHaveBeenCalledWith("/books/b1");
  });

  it("shows a Deleted badge for soft-deleted books", () => {
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1", deleted: true })]}
        onSelect={vi.fn()}
        selectedIds={new Set()}
        isSelectionMode={false}
        canBeSelected={true}
      />,
    );

    expect(screen.getByText("Deleted")).toBeInTheDocument();
  });

  it("does not call onSelect when canBeSelected is false in selection mode", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <SeriesBookTable
        books={[makeBook({ id: "b1" })]}
        onSelect={onSelect}
        selectedIds={new Set()}
        isSelectionMode={true}
        canBeSelected={false}
      />,
    );

    await user.click(screen.getByTestId("series-book-row-b1"));
    expect(onSelect).not.toHaveBeenCalled();
    expect(mockNavigate).not.toHaveBeenCalled();
  });
});
