import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { HorizontalCarousel } from "./HorizontalCarousel";

describe("HorizontalCarousel", () => {
  it("renders its title and children", () => {
    renderWithProviders(
      <HorizontalCarousel title="Keep Reading">
        <div>Item</div>
      </HorizontalCarousel>,
    );

    expect(screen.getByText("Keep Reading")).toBeInTheDocument();
    expect(screen.getByText("Item")).toBeInTheDocument();
  });

  it("renders a 'See all' link pointing at seeAllLink when provided", () => {
    renderWithProviders(
      <HorizontalCarousel
        title="Keep Reading"
        seeAllLink="/libraries/all/keep-reading"
      >
        <div>Item</div>
      </HorizontalCarousel>,
    );

    const link = screen.getByRole("link", { name: /see all/i });
    expect(link).toHaveAttribute("href", "/libraries/all/keep-reading");
  });

  it("does not render a 'See all' link when seeAllLink is omitted", () => {
    renderWithProviders(
      <HorizontalCarousel title="Keep Reading">
        <div>Item</div>
      </HorizontalCarousel>,
    );

    expect(
      screen.queryByRole("link", { name: /see all/i }),
    ).not.toBeInTheDocument();
  });
});
