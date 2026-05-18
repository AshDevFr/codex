import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { MetadataLabel } from "./MetadataLabel";

describe("MetadataLabel", () => {
  it("renders children with the Phase 7 typography defaults", () => {
    renderWithProviders(<MetadataLabel>PUBLISHER</MetadataLabel>);

    const label = screen.getByText("PUBLISHER");
    expect(label).toBeInTheDocument();

    // Mantine 8's Text translates the fz/fw/lts/w props into inline CSS
    // properties on the element (rem-scaled where appropriate). A
    // regression here would mean the typography tokens stopped flowing
    // through.
    const style = label.getAttribute("style") ?? "";
    // 11px → 0.6875rem (11/16) wrapped in Mantine's scale calc.
    expect(style).toMatch(/font-size:\s*calc\(0\.6875rem/);
    expect(style).toMatch(/font-weight:\s*600/);
    expect(style).toMatch(/letter-spacing:\s*0\.04em/);
    // 100px → 6.25rem (100/16) wrapped in Mantine's scale calc.
    expect(style).toMatch(/width:\s*calc\(6\.25rem/);
  });

  it("applies uppercase transform and dimmed colour", () => {
    renderWithProviders(<MetadataLabel>publisher</MetadataLabel>);

    const label = screen.getByText("publisher");
    expect(label.className).toMatch(/mantine-Text-root/);
    const style = label.getAttribute("style") ?? "";
    expect(style).toMatch(/text-transform:\s*uppercase/);
    expect(style).toContain("var(--mantine-color-dimmed)");
  });

  it("forwards extra props to the underlying Text", () => {
    renderWithProviders(
      <MetadataLabel data-testid="custom" style={{ flexShrink: 0 }}>
        EXTERNAL IDS
      </MetadataLabel>,
    );

    const label = screen.getByTestId("custom");
    expect(label.textContent).toBe("EXTERNAL IDS");
    expect(label.getAttribute("style")).toContain("flex-shrink");
  });
});
