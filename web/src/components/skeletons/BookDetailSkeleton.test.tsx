import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { BookDetailSkeleton } from "./BookDetailSkeleton";

describe("BookDetailSkeleton", () => {
  it("renders the book-detail shape placeholder", () => {
    renderWithProviders(<BookDetailSkeleton />);
    expect(screen.getByTestId("book-detail-skeleton")).toBeInTheDocument();
  });
});
