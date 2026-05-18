import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Read the CSS source directly so we can assert on raw rules without
// booting a browser or jsdom. Vitest runs from `web/` so this path is
// stable across both `vitest run` and the CI matrix.
const css = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");

describe("index.css — Phase 7 typography micro-pass", () => {
  it("enables tabular numerals and SF Pro alternates on the body", () => {
    // The plan calls for `tnum` (tabular figures for date/metric columns)
    // plus `ss01` / `cv01` (SF Pro stylistic alternates, no-op elsewhere).
    // A regression here would mean the Users table dates and the storage
    // quota meter lose their vertical alignment.
    const bodyBlock = /body\s*\{[\s\S]*?\}/.exec(css)?.[0] ?? "";
    expect(bodyBlock).toMatch(/font-feature-settings:\s*[^;]*"tnum"/);
    expect(bodyBlock).toMatch(/font-feature-settings:\s*[^;]*"ss01"/);
    expect(bodyBlock).toMatch(/font-feature-settings:\s*[^;]*"cv01"/);
  });

  it("tightens display-size h1 letter-spacing at the xs breakpoint and above", () => {
    // iOS-style negative tracking on h1 — but only at desktop widths. The
    // mobile scale (1.5rem from the section-4 rule) needs the default
    // tracking for legibility, so the rule must be gated by min-width.
    expect(css).toMatch(
      /@media \(min-width: 30\.125em\)\s*\{[^}]*\.mantine-Title-root\[data-order="1"\]\s*\{[^}]*letter-spacing:\s*-0\.015em/,
    );
  });

  it("uses the tighter 1.2 line-height for h1 on mobile", () => {
    // Phase 7 tightens mobile h1 from `--title-lh: 1.3` to `1.2` so wrap-
    // prone titles (Solo Leveling, Friend in Need) feel less verbose. h2
    // and h3 stay at 1.3 because they generally render on a single line.
    const mobileBlock =
      /@media \(max-width: 30\.0625em\)\s*\{([\s\S]*?)\n\}/m.exec(css)?.[1] ??
      "";
    expect(mobileBlock).toMatch(
      /\.mantine-Title-root\[data-order="1"\][^}]*--title-lh:\s*1\.2/,
    );
    // h2 unchanged.
    expect(mobileBlock).toMatch(
      /\.mantine-Title-root\[data-order="2"\][^}]*--title-lh:\s*1\.3/,
    );
  });
});
