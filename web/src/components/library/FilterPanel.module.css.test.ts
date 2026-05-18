import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Read the CSS module source directly so we can assert on the raw rules
// without booting jsdom (which doesn't evaluate CSS module class hashes).
// Vitest runs from `web/` so this path is stable across `vitest run`
// and the CI matrix.
const css = readFileSync(
  resolve(process.cwd(), "src/components/library/FilterPanel.module.css"),
  "utf8",
);

describe("FilterPanel.module.css — Phase 9 info-design", () => {
  it("renders the section header sticky to the top of its scroll container", () => {
    // Sticky section headers must pin to the top of the drawer body /
    // sheet body so users keep their bearings as they scroll between
    // groups. Anchoring them only to the viewport would cause them to
    // float off when the sheet is partially translated down.
    const block = /\.sectionHeader\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(block).toMatch(/position:\s*sticky/);
    expect(block).toMatch(/top:\s*0/);
    // The hairline below the header is the box-shadow trick — using a
    // border would always render, even before content scrolled under it.
    expect(block).toMatch(
      /box-shadow:\s*0 1px 0\s*var\(--surface-border-hairline\)/,
    );
  });

  it("pins the action footer with a top hairline and upward shadow", () => {
    // The footer must read as anchored, not as part of scroll content.
    // Sticky bottom + top hairline + upward shadow does that without
    // restructuring the markup.
    const block = /\.footer\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(block).toMatch(/position:\s*sticky/);
    expect(block).toMatch(/bottom:\s*0/);
    expect(block).toMatch(
      /border-top:\s*1px solid\s*var\(--surface-border-hairline\)/,
    );
    expect(block).toMatch(/box-shadow:\s*0 -4px 12px/);
    // Safe-area-inset bottom must compose so the home indicator
    // doesn't cover the Clear all / Apply buttons in PWA standalone.
    expect(block).toMatch(/safe-area-inset-bottom/);
  });

  it("anchors the mobile bottom sheet to the viewport with rounded top corners", () => {
    // The bottom sheet is fixed to the bottom edge of the viewport,
    // tall enough to hold the full filter list at the `full` snap. The
    // rounded top corners are the visual cue that it lifts off the
    // grid below.
    const block = /\.bottomSheet\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(block).toMatch(/position:\s*fixed/);
    expect(block).toMatch(/inset:\s*auto 0 0 0/);
    expect(block).toMatch(/height:\s*90dvh/);
    expect(block).toMatch(/border-top-left-radius:\s*16px/);
    expect(block).toMatch(/border-top-right-radius:\s*16px/);
  });

  it("renders the drag handle at 36×4 px", () => {
    const block = /\.bottomSheetHandle\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(block).toMatch(/width:\s*36px/);
    expect(block).toMatch(/height:\s*4px/);
    expect(block).toMatch(/border-radius:\s*999px/);
  });
});
