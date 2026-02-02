import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import type { ReadProgress } from "@/types";
import { BookProgress } from "./BookProgress";

// Helper to create a complete ReadProgress object
const createProgress = (
  overrides: Partial<ReadProgress> & {
    current_page: number;
    completed: boolean;
    completed_at?: string | null;
  },
): ReadProgress => ({
  book_id: "book-123",
  id: "progress-123",
  user_id: "user-123",
  started_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-15T00:00:00Z",
  ...overrides,
});

describe("BookProgress", () => {
  it("should show 'Not started' when no progress", () => {
    renderWithProviders(<BookProgress progress={null} pageCount={100} />);

    expect(screen.getByText("Not started")).toBeInTheDocument();
  });

  it("should show 'Not started' when progress is undefined", () => {
    renderWithProviders(<BookProgress progress={undefined} pageCount={100} />);

    expect(screen.getByText("Not started")).toBeInTheDocument();
  });

  it("should show 'Completed' when book is completed", () => {
    const progress = createProgress({
      current_page: 99,
      completed: true,
      completed_at: "2024-01-15T10:30:00Z",
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("should show completion date when book is completed", () => {
    const progress = createProgress({
      current_page: 99,
      completed: true,
      completed_at: "2024-01-15T10:30:00Z",
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    // The date format depends on locale, so just check for the 'on' prefix
    expect(screen.getByText(/^on/)).toBeInTheDocument();
  });

  it("should show progress bar when reading in progress", () => {
    const progress = createProgress({
      current_page: 50, // 1-indexed
      completed: false,
      completed_at: null,
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    expect(screen.getByText(/Page 50 of 100/)).toBeInTheDocument();
    expect(screen.getByText(/\(50%\)/)).toBeInTheDocument();
  });

  it("should calculate percentage correctly", () => {
    const progress = createProgress({
      current_page: 25, // 1-indexed
      completed: false,
      completed_at: null,
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    expect(screen.getByText(/Page 25 of 100/)).toBeInTheDocument();
    expect(screen.getByText(/\(25%\)/)).toBeInTheDocument();
  });

  it("should handle first page (1-indexed)", () => {
    const progress = createProgress({
      current_page: 1, // 1-indexed
      completed: false,
      completed_at: null,
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    expect(screen.getByText(/Page 1 of 100/)).toBeInTheDocument();
    expect(screen.getByText(/\(1%\)/)).toBeInTheDocument();
  });

  it("should handle edge case of zero page count", () => {
    const progress = createProgress({
      current_page: 1,
      completed: false,
      completed_at: null,
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={0} />);

    // Should show 0% when page count is 0 to avoid division by zero
    expect(screen.getByText(/\(0%\)/)).toBeInTheDocument();
  });

  it("should render progress bar element", () => {
    const progress = createProgress({
      current_page: 49,
      completed: false,
      completed_at: null,
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    // Check that a progress bar is rendered
    expect(document.querySelector('[role="progressbar"]')).toBeInTheDocument();
  });

  it("should not show progress bar when completed", () => {
    const progress = createProgress({
      current_page: 99,
      completed: true,
      completed_at: "2024-01-15T10:30:00Z",
    });

    renderWithProviders(<BookProgress progress={progress} pageCount={100} />);

    expect(
      document.querySelector('[role="progressbar"]'),
    ).not.toBeInTheDocument();
  });
});
