import { describe, expect, it } from "vitest";
import { cssVariablesResolver, theme } from "./theme";

const SURFACE_TOKENS = ["--surface-1", "--surface-2", "--surface-3"] as const;
const SHADOW_TOKENS = [
  "--shadow-xs",
  "--shadow-sm",
  "--shadow-md",
  "--shadow-lg",
  "--shadow-xl",
] as const;
const HAIRLINE_TOKENS = [
  "--surface-border-hairline",
  "--card-border-hairline",
] as const;
const CARD_SHADOW_TOKENS = [
  "--shadow-card-mobile",
  "--shadow-card-desktop",
] as const;

describe("cssVariablesResolver", () => {
  const resolved = cssVariablesResolver(theme as never);

  it("exposes the elevation ladder and shadow scale in light mode", () => {
    for (const token of SURFACE_TOKENS) {
      expect(resolved.light[token]).toBeTruthy();
    }
    for (const token of SHADOW_TOKENS) {
      expect(resolved.light[token]).toMatch(/rgba\(/);
    }
  });

  it("exposes the iOS dark elevation ladder", () => {
    expect(resolved.dark["--surface-1"]).toBe("#1c1c1e");
    expect(resolved.dark["--surface-2"]).toBe("#2c2c2e");
    expect(resolved.dark["--surface-3"]).toBe("#3a3a3c");
  });

  it("uses higher-alpha shadows in dark mode than light mode", () => {
    // Cheap sanity check: dark-mode shadow strings must contain alpha values
    // > 0.15 (light mode peaks at 0.12). Catches the most common regression:
    // shadow tokens copy-pasted between schemes.
    for (const token of SHADOW_TOKENS) {
      const dark = resolved.dark[token] ?? "";
      const matches = Array.from(dark.matchAll(/rgba\([^)]*,\s*([0-9.]+)\)/g));
      const maxAlpha = Math.max(
        ...matches.map((m) => Number.parseFloat(m[1] ?? "0")),
      );
      expect(maxAlpha).toBeGreaterThan(0.15);
    }
  });

  it("keeps legacy surface variables intact for backwards compatibility", () => {
    expect(resolved.dark["--mantine-color-body"]).toBe("#242424");
    expect(resolved.light["--mantine-color-body"]).toBe("#ffffff");
    expect(resolved.dark["--card-bg"]).toBe("#242424");
    expect(resolved.light["--card-bg"]).toBe("#ffffff");
  });

  it("exposes hairline border tokens for the Phase 2 depth refresh", () => {
    for (const token of HAIRLINE_TOKENS) {
      // Light uses near-black with low alpha; dark uses near-white with low
      // alpha. Both schemes must define both tokens so the depth refresh CSS
      // can reference them without scheme-specific fallbacks.
      expect(resolved.light[token]).toMatch(/rgba\(/);
      expect(resolved.dark[token]).toMatch(/rgba\(/);
    }
    // Dark hairline is faint-white (`255, 255, 255`) so it reads against the
    // near-black body. Catches regressions where the dark token gets copied
    // from the light scheme.
    expect(resolved.dark["--surface-border-hairline"]).toContain("255");
    expect(resolved.dark["--card-border-hairline"]).toContain("255");
  });

  it("exposes mobile- and desktop-tuned card shadow tokens", () => {
    for (const token of CARD_SHADOW_TOKENS) {
      expect(resolved.light[token]).toMatch(/rgba\(/);
      expect(resolved.dark[token]).toMatch(/rgba\(/);
    }
    // The mobile shadow must use a smaller blur radius than desktop so it
    // doesn't bleed into the 2-column grid gutter. Compare the largest blur
    // value in each token.
    const largestBlur = (value: string): number => {
      const matches = Array.from(
        value.matchAll(/(-?\d+)px\s+(-?\d+)px\s+(\d+)px/g),
      );
      return Math.max(...matches.map((m) => Number.parseInt(m[3] ?? "0", 10)));
    };
    for (const scheme of ["light", "dark"] as const) {
      const mobile = resolved[scheme]["--shadow-card-mobile"] ?? "";
      const desktop = resolved[scheme]["--shadow-card-desktop"] ?? "";
      expect(largestBlur(mobile)).toBeLessThanOrEqual(largestBlur(desktop));
      expect(largestBlur(mobile)).toBeLessThanOrEqual(8);
    }
  });
});
