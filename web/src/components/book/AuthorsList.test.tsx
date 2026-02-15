import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import type { BookAuthor } from "@/types/book-metadata";
import { AuthorsCompact, AuthorsList } from "./AuthorsList";

describe("AuthorsList", () => {
  const mockAuthors: BookAuthor[] = [
    {
      name: "Brandon Sanderson",
      role: "author",
      sortName: "Sanderson, Brandon",
    },
    { name: "Dan Wells", role: "co_author" },
    { name: "Howard Tayler", role: "illustrator" },
  ];

  it("renders nothing when authors is null", () => {
    renderWithProviders(<AuthorsList authors={null} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when authors is undefined", () => {
    renderWithProviders(<AuthorsList authors={undefined} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when authors array is empty", () => {
    renderWithProviders(<AuthorsList authors={[]} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders authors from array", () => {
    renderWithProviders(<AuthorsList authors={mockAuthors} />);
    expect(screen.getByText("Brandon Sanderson")).toBeInTheDocument();
    expect(screen.getByText("Dan Wells")).toBeInTheDocument();
    expect(screen.getByText("Howard Tayler")).toBeInTheDocument();
  });

  it("parses and renders authors from JSON string", () => {
    const json = JSON.stringify(mockAuthors);
    renderWithProviders(<AuthorsList authors={json} />);
    expect(screen.getByText("Brandon Sanderson")).toBeInTheDocument();
    expect(screen.getByText("Dan Wells")).toBeInTheDocument();
  });

  it("limits displayed authors when maxDisplay is set", () => {
    renderWithProviders(<AuthorsList authors={mockAuthors} maxDisplay={2} />);
    expect(screen.getByText("Brandon Sanderson")).toBeInTheDocument();
    expect(screen.getByText("Dan Wells")).toBeInTheDocument();
    expect(screen.queryByText("Howard Tayler")).not.toBeInTheDocument();
    expect(screen.getByText("+1 more")).toBeInTheDocument();
  });

  it("groups authors by role when groupByRole is true", () => {
    renderWithProviders(<AuthorsList authors={mockAuthors} groupByRole />);
    expect(screen.getByText("Author")).toBeInTheDocument();
    expect(screen.getByText("Co-Author")).toBeInTheDocument();
    expect(screen.getByText("Illustrator")).toBeInTheDocument();
  });

  it("handles invalid JSON gracefully", () => {
    renderWithProviders(<AuthorsList authors="invalid json" />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });
});

describe("AuthorsCompact", () => {
  const mockAuthors: BookAuthor[] = [
    { name: "Author One", role: "author" },
    { name: "Author Two", role: "author" },
    { name: "Author Three", role: "author" },
    { name: "Author Four", role: "author" },
  ];

  it("renders nothing when authors is null", () => {
    renderWithProviders(<AuthorsCompact authors={null} />);
    expect(screen.queryByText(/,/)).not.toBeInTheDocument();
  });

  it("renders comma-separated author names", () => {
    renderWithProviders(<AuthorsCompact authors={mockAuthors.slice(0, 2)} />);
    expect(screen.getByText("Author One, Author Two")).toBeInTheDocument();
  });

  it("limits display and shows +N more", () => {
    renderWithProviders(
      <AuthorsCompact authors={mockAuthors} maxDisplay={3} />,
    );
    expect(
      screen.getByText("Author One, Author Two, Author Three +1 more"),
    ).toBeInTheDocument();
  });

  it("parses JSON string input", () => {
    const json = JSON.stringify(mockAuthors.slice(0, 2));
    renderWithProviders(<AuthorsCompact authors={json} />);
    expect(screen.getByText("Author One, Author Two")).toBeInTheDocument();
  });
});
