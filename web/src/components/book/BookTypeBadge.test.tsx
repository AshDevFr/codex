import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { BookTypeBadge } from "./BookTypeBadge";

describe("BookTypeBadge", () => {
  it("renders nothing when bookType is null", () => {
    renderWithProviders(<BookTypeBadge bookType={null} />);
    // No badge should be rendered
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders nothing when bookType is undefined", () => {
    renderWithProviders(<BookTypeBadge bookType={undefined} />);
    // No badge should be rendered
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders comic badge correctly", () => {
    renderWithProviders(<BookTypeBadge bookType="comic" />);
    expect(screen.getByText("Comic")).toBeInTheDocument();
  });

  it("renders manga badge correctly", () => {
    renderWithProviders(<BookTypeBadge bookType="manga" />);
    expect(screen.getByText("Manga")).toBeInTheDocument();
  });

  it("renders novel badge correctly", () => {
    renderWithProviders(<BookTypeBadge bookType="novel" />);
    expect(screen.getByText("Novel")).toBeInTheDocument();
  });

  it("renders graphic_novel badge with proper display name", () => {
    renderWithProviders(<BookTypeBadge bookType="graphic_novel" />);
    expect(screen.getByText("Graphic Novel")).toBeInTheDocument();
  });

  it("handles uppercase input", () => {
    renderWithProviders(<BookTypeBadge bookType="MANGA" />);
    expect(screen.getByText("Manga")).toBeInTheDocument();
  });

  it("handles unknown book types gracefully", () => {
    renderWithProviders(<BookTypeBadge bookType="unknown_type" />);
    expect(screen.getByText("Unknown type")).toBeInTheDocument();
  });

  it("renders with icon when withIcon is true", () => {
    renderWithProviders(<BookTypeBadge bookType="manga" withIcon />);
    expect(screen.getByText("Manga")).toBeInTheDocument();
    // Icon should be present as an SVG
    const badge = screen.getByText("Manga").closest(".mantine-Badge-root");
    expect(badge?.querySelector("svg")).toBeInTheDocument();
  });

  it("applies correct size prop", () => {
    renderWithProviders(<BookTypeBadge bookType="comic" size="lg" />);
    const badge = screen.getByText("Comic").closest(".mantine-Badge-root");
    expect(badge).toHaveAttribute("data-size", "lg");
  });

  it("applies correct variant prop", () => {
    renderWithProviders(<BookTypeBadge bookType="novel" variant="filled" />);
    const badge = screen.getByText("Novel").closest(".mantine-Badge-root");
    expect(badge).toHaveAttribute("data-variant", "filled");
  });
});
