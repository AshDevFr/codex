import { describe, expect, it } from "vitest";
import { cssVariablesResolver, theme } from "./theme";

// Minimal WCAG 2.1 contrast helper. Kept in the test file because the only
// place we compute contrast today is the dark-mode contrast assertions
// below; pulling in a dedicated dependency for ~15 lines of arithmetic is
// overkill.
const channelToLinear = (channel: number): number => {
  const c = channel / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const relativeLuminance = (hex: string): number => {
  const value = hex.replace("#", "");
  const r = Number.parseInt(value.slice(0, 2), 16);
  const g = Number.parseInt(value.slice(2, 4), 16);
  const b = Number.parseInt(value.slice(4, 6), 16);
  return (
    0.2126 * channelToLinear(r) +
    0.7152 * channelToLinear(g) +
    0.0722 * channelToLinear(b)
  );
};
const contrastRatio = (fg: string, bg: string): number => {
  const l1 = relativeLuminance(fg);
  const l2 = relativeLuminance(bg);
  const [lighter, darker] = l1 >= l2 ? [l1, l2] : [l2, l1];
  return (lighter + 0.05) / (darker + 0.05);
};

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

  it("points legacy body / card tokens at the iOS elevation ladder", () => {
    // Dark mode body sits at --surface-1, cards at --surface-2. Light mode
    // body stays white while app-shell-main warms slightly (covered in its
    // own test below).
    expect(resolved.dark["--mantine-color-body"]).toBe("#1c1c1e");
    expect(resolved.dark["--card-bg"]).toBe("#2c2c2e");
    expect(resolved.light["--mantine-color-body"]).toBe("#ffffff");
    expect(resolved.light["--card-bg"]).toBe("#ffffff");
  });

  it("warms the light-mode app-shell-main surface", () => {
    // Main content area sits one notch warmer than the pure-white body so
    // cards have a hair more contrast against it.
    expect(resolved.light["--app-shell-main-bg"]).toBe("#f7f7f9");
    expect(resolved.light["--surface-1"]).toBe("#f7f7f9");
    // Dark app-shell-main collapses onto the body in the iOS ladder.
    expect(resolved.dark["--app-shell-main-bg"]).toBe("#1c1c1e");
  });

  it("keeps primaryBlue[8] aligned with the light-mode brand hue", () => {
    // Mantine defaults `primaryShade.dark = 8`. Steps 6 and 8 deliberately
    // share `#1d4ed8` (Tailwind blue-700) so primary buttons read the same
    // in both schemes and line up with the PWA `theme_color`. Catches the
    // regression where someone reverts step 8 to the older navy `#1e3a8a`.
    const blue = theme.colors?.blue;
    expect(blue).toBeDefined();
    expect(blue?.[8]).toBe("#1d4ed8");
  });

  it("keeps dimmed text WCAG AA on the dark-mode card surface", () => {
    // Mantine's default gray[5] (`#909296`) lands at ~4.47:1 against the
    // `#2c2c2e` card surface — just below WCAG AA's 4.5:1 floor for normal
    // text. The dimmed token is nudged so dimmed/caption text on cards
    // stays compliant. Computed here so a future revert can't silently
    // land sub-AA contrast.
    const dimmed = resolved.dark["--mantine-color-dimmed"] ?? "";
    expect(contrastRatio(dimmed, "#2c2c2e")).toBeGreaterThanOrEqual(4.5);
    // Main text comfortably exceeds AAA on both body and card surfaces.
    const text = resolved.dark["--mantine-color-text"] ?? "";
    expect(contrastRatio(text, "#1c1c1e")).toBeGreaterThanOrEqual(7);
    expect(contrastRatio(text, "#2c2c2e")).toBeGreaterThanOrEqual(7);
  });

  it("keeps light-mode text WCAG AA on the app-shell-main surface", () => {
    const text = resolved.light["--mantine-color-text"] ?? "";
    const dimmed = resolved.light["--mantine-color-dimmed"] ?? "";
    expect(contrastRatio(text, "#f7f7f9")).toBeGreaterThanOrEqual(7);
    expect(contrastRatio(dimmed, "#f7f7f9")).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(dimmed, "#ffffff")).toBeGreaterThanOrEqual(4.5);
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

  it("sets spring-feel transition defaults on Drawer and Modal", () => {
    // Phase 3 routes drawers and modals through the shared `--ease-out`
    // curve so reader-side drawers, settings modals, and the mobile search
    // sheet all read as the same motion language. Catches the regression
    // where someone reverts Mantine back to its default linear easing.
    const drawerProps = theme.components?.Drawer?.defaultProps as
      | Record<string, unknown>
      | undefined;
    const drawerTransition = drawerProps?.transitionProps as
      | { duration?: number; timingFunction?: string }
      | undefined;
    expect(drawerTransition?.timingFunction).toBe("var(--ease-out)");
    expect(drawerTransition?.duration).toBeGreaterThanOrEqual(200);

    const modalProps = theme.components?.Modal?.defaultProps as
      | Record<string, unknown>
      | undefined;
    const modalTransition = modalProps?.transitionProps as
      | { duration?: number; timingFunction?: string }
      | undefined;
    expect(modalTransition?.timingFunction).toBe("var(--ease-out)");
    expect(modalTransition?.duration).toBeGreaterThanOrEqual(200);
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
