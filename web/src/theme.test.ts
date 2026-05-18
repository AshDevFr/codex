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
});
