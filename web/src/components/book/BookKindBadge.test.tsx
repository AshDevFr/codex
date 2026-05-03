import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { BookKindBadge } from "./BookKindBadge";

describe("BookKindBadge", () => {
  it("renders Vol N when only volume is set", () => {
    renderWithProviders(<BookKindBadge volume={5} chapter={null} />);
    expect(screen.getByText("Vol 5")).toBeInTheDocument();
  });

  it("renders Ch N when only chapter is set", () => {
    renderWithProviders(<BookKindBadge volume={null} chapter={42} />);
    expect(screen.getByText("Ch 42")).toBeInTheDocument();
  });

  it("renders Ch N preserving fractional chapters", () => {
    renderWithProviders(<BookKindBadge volume={null} chapter={42.5} />);
    expect(screen.getByText("Ch 42.5")).toBeInTheDocument();
  });

  it("renders combined Vol V · Ch C when both are set", () => {
    renderWithProviders(<BookKindBadge volume={15} chapter={126} />);
    expect(screen.getByText("Vol 15 · Ch 126")).toBeInTheDocument();
  });

  it("uses the chapter color (grape) when both volume and chapter are set", () => {
    renderWithProviders(<BookKindBadge volume={15} chapter={126} />);
    const badge = screen
      .getByText("Vol 15 · Ch 126")
      .closest(".mantine-Badge-root") as HTMLElement | null;
    expect(badge).not.toBeNull();
    // Mantine renders the badge color into a CSS custom property on the root
    const styleColor = badge?.getAttribute("style") ?? "";
    expect(styleColor).toMatch(/grape/);
  });

  it("uses the volume color (blue) when only volume is set", () => {
    renderWithProviders(<BookKindBadge volume={7} chapter={null} />);
    const badge = screen
      .getByText("Vol 7")
      .closest(".mantine-Badge-root") as HTMLElement | null;
    expect(badge).not.toBeNull();
    const styleColor = badge?.getAttribute("style") ?? "";
    expect(styleColor).toMatch(/blue/);
  });

  it("renders muted Vol fallback when neither is set", () => {
    renderWithProviders(<BookKindBadge volume={null} chapter={null} />);
    // The badge text is just "Vol" (no number)
    expect(screen.getByText("Vol")).toBeInTheDocument();
    // Should be the outline variant (gray, default-to-volume signal)
    const badge = screen.getByText("Vol").closest(".mantine-Badge-root");
    expect(badge).toHaveAttribute("data-variant", "outline");
  });

  it("treats undefined as null for both fields", () => {
    renderWithProviders(
      <BookKindBadge volume={undefined} chapter={undefined} />,
    );
    expect(screen.getByText("Vol")).toBeInTheDocument();
  });
});
