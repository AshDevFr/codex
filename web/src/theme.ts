import {
  type CSSVariablesResolver,
  createTheme,
  type MantineColorsTuple,
  type MantineTheme,
  type ModalProps,
} from "@mantine/core";

// Primary blue color palette (inspired by Komga).
//
// Mantine defaults `primaryShade` to `{ light: 6, dark: 8 }`. Steps 6 and 8
// deliberately share a hue (Tailwind blue-700, `#1d4ed8`) so the brand blue
// reads identically in light and dark schemes and lines up with the PWA
// `theme_color`. Step 9 keeps a darker navy (`#1e3a8a`) for hover/active
// layers.
const primaryBlue: MantineColorsTuple = [
  "#e6f2ff",
  "#cce4ff",
  "#99c9ff",
  "#66adff",
  "#3b82f6", // Main blue
  "#2563eb",
  "#1d4ed8",
  "#1e40af",
  "#1d4ed8",
  "#1e3a8a",
];

export const theme = createTheme({
  primaryColor: "blue",
  colors: {
    blue: primaryBlue,
  },

  // Only show focus ring for keyboard navigation, not mouse clicks
  focusRing: "auto",

  // Base text color (dark theme defaults)
  black: "#121212",
  white: "#e0e0e0",

  // Component defaults
  defaultRadius: "md",

  // Spacing
  spacing: {
    xs: "0.5rem",
    sm: "0.75rem",
    md: "1rem",
    lg: "1.5rem",
    xl: "2rem",
  },

  // Breakpoints (em, matching Mantine's default scheme).
  //
  // We override `xs` to a phone-only line at ~482px (30.125em). This is below the
  // common iPhone Pro Max portrait width (~430px) but above smaller phones, giving
  // us a clean "phone vs tablet" cutoff. `sm` (768px) is kept at Mantine's default
  // so existing `visibleFrom="sm"` / `hiddenFrom="sm"` sites are unaffected; new
  // phone-tight behavior should use `xs` instead.
  breakpoints: {
    xs: "30.125em",
    sm: "48em",
    md: "62em",
    lg: "75em",
    xl: "88em",
  },

  // Custom properties for layout
  other: {
    sidebarWidth: 240,
    headerHeight: 64,
  },

  components: {
    AppShell: {
      defaultProps: {
        padding: "md",
      },
    },
    Card: {
      defaultProps: {
        shadow: "sm",
        radius: "md",
      },
    },
    Button: {
      defaultProps: {
        radius: "md",
      },
    },
    // Spring-feel default transition. Mantine's `<Transition>` is a CSS
    // easing wrapper rather than a true spring; pinning the curve to the
    // shared `--ease-out` token + a slightly longer 280ms duration reads as
    // "soft spring" without pulling motion-lib into the portal mount path
    // (Mantine controls its own portal mount/unmount schedule and fights
    // `<AnimatePresence>`). Reader-side drawers inherit the same defaults
    // and keep their text legibility because the rule only changes timing,
    // not opacity or transform.
    Drawer: {
      defaultProps: {
        transitionProps: {
          duration: 280,
          timingFunction: "var(--ease-out)",
        },
      },
    },
    Modal: {
      defaultProps: {
        transitionProps: {
          duration: 240,
          timingFunction: "var(--ease-out)",
        },
      },
      styles: {
        content: {
          // Make modals wider on desktop
          maxWidth: "min(90vw, var(--modal-size))",
        },
      },
      vars: (_theme: MantineTheme, props: ModalProps) => {
        // Increase modal sizes for desktop
        const sizeMap: Record<string, string> = {
          xs: "400px",
          sm: "500px",
          md: "600px",
          lg: "800px",
          xl: "1000px",
        };
        const size = props.size as string | undefined;
        return {
          root: {
            "--modal-size": sizeMap[size || "md"] || size || "600px",
          },
        };
      },
    },
  },
});

// CSS variables resolver for light/dark mode specific customizations.
//
// Surface and shadow tokens are defined here so later polish phases can lean
// on a stable depth ladder.
//
// Elevation ladder (--surface-1/2/3):
//   1 = body / app-shell-main background
//   2 = raised card sitting on top of body
//   3 = elevated menu/popover/modal floating above cards
// Dark-mode steps follow the iOS systemBackground family (#1c1c1e / #2c2c2e
// / #3a3a3c) so cards visibly separate from the body. Light-mode steps keep
// today's values to avoid an uncoordinated palette shift before later
// phases apply the depth refresh.
//
// Shadows (--shadow-xs/sm/md/lg/xl) carry the depth language that Phase 2
// uses to replace the current 1px-solid-border treatment. Dark-mode alphas
// are higher because shadows over a near-black body need more contrast to
// be perceptible.
//
// Phase 2 also exposes:
//   --surface-border-hairline: an almost-invisible 1px line used in place of
//     the old `--mantine-color-default-border` solid line on the header,
//     sidebar, card, menu and modal surfaces. The hairline lets us keep a
//     pixel-precise edge without the "drawn-on" look the solid border had.
//   --card-border-hairline: same idea, slightly stronger so cards still
//     read as discrete tiles in dense grids.
//   --shadow-card-mobile / --shadow-card-desktop: the card shadow split
//     into two scales so the mobile 2-column grid doesn't get shadow bleed
//     between neighbouring tiles while desktop keeps a more confident lift.
export const cssVariablesResolver: CSSVariablesResolver = (_theme) => ({
  variables: {
    // Scheme-independent variables
  },
  light: {
    // Light mode: Use a clean white background with better contrast
    "--mantine-color-body": "#ffffff",

    // Improve text contrast in light mode
    "--mantine-color-text": "#1a1b1e",
    "--mantine-color-dimmed": "#495057",

    // AppShell colors for light mode
    "--mantine-color-default": "#ffffff",
    "--mantine-color-default-hover": "#f8f9fa",
    "--mantine-color-default-border": "#dee2e6",

    // App-shell-main sits one notch warmer than the body so pure-white
    // cards separate cleanly from the page; cards stay pure white.
    "--app-shell-main-bg": "#f7f7f9",
    "--card-bg": "#ffffff",
    "--card-border": "#e9ecef",

    // Navbar styling in light mode
    "--mantine-color-gray-light": "#f1f3f5",
    "--mantine-color-gray-light-hover": "#e9ecef",

    // Elevation ladder. Light mode keeps the raised / elevated tiers at
    // pure white and leans on shadows for depth; the body (surface-1)
    // mirrors the warmed app-shell-main above.
    "--surface-1": "#f7f7f9",
    "--surface-2": "#ffffff",
    "--surface-3": "#ffffff",

    // Shadow scale (light): low alpha so cards float without weight.
    "--shadow-xs": "0 1px 2px rgba(15, 23, 42, 0.04)",
    "--shadow-sm":
      "0 1px 2px rgba(15, 23, 42, 0.06), 0 1px 3px rgba(15, 23, 42, 0.04)",
    "--shadow-md":
      "0 2px 4px rgba(15, 23, 42, 0.06), 0 4px 12px rgba(15, 23, 42, 0.06)",
    "--shadow-lg":
      "0 4px 8px rgba(15, 23, 42, 0.08), 0 12px 24px rgba(15, 23, 42, 0.08)",
    "--shadow-xl":
      "0 8px 16px rgba(15, 23, 42, 0.10), 0 24px 48px rgba(15, 23, 42, 0.12)",

    // Hairline borders replace the solid 1px Mantine defaults. Light scheme
    // keeps these near-black at very low alpha so they read as a soft edge
    // rather than a drawn-on line.
    "--surface-border-hairline": "rgba(15, 23, 42, 0.06)",
    "--card-border-hairline": "rgba(15, 23, 42, 0.05)",

    // Mobile cards live in a 2-column grid with only ~12px of gutter; the
    // desktop shadow scale would bleed across that gap. Cap the mobile blur
    // at 8px so neighbouring tiles stay visually distinct.
    "--shadow-card-mobile":
      "0 1px 2px rgba(15, 23, 42, 0.05), 0 2px 6px rgba(15, 23, 42, 0.04)",
    "--shadow-card-desktop":
      "0 1px 2px rgba(15, 23, 42, 0.06), 0 4px 12px rgba(15, 23, 42, 0.05)",
  },
  dark: {
    // Dark mode follows the iOS systemBackground family: a near-black body
    // with a slight blue undertone, then a distinct raised tier for cards
    // and a third tier for elevated menus / modals. Cards visibly separate
    // from the body without relying on a 1px solid border.
    "--mantine-color-body": "#1c1c1e",
    "--mantine-color-text": "#e0e0e0",
    // Dimmed text needs to clear WCAG AA (4.5:1) against the `#2c2c2e`
    // card surface; Mantine's default gray[5] (`#909296`) lands at ~4.47:1
    // there. This value bumps just enough to pass while preserving the
    // visual hierarchy against `--mantine-color-text`.
    "--mantine-color-dimmed": "#9a9da1",

    // AppShell colors for dark mode follow the new body tier so input
    // surfaces, action defaults, and hover states stay aligned with the
    // elevation ladder.
    "--mantine-color-default": "#1c1c1e",
    "--mantine-color-default-hover": "#2c2c2e",
    "--mantine-color-default-border": "#373a40",

    // Card surfaces land on the raised tier so a card on the body reads
    // one elevation step above. Legacy `--card-bg` stays in sync with
    // `--surface-2` so consumers that haven't migrated to the new token
    // name still pick up the right tier.
    "--app-shell-main-bg": "#1c1c1e",
    "--card-bg": "#2c2c2e",
    "--card-border": "#373a40",

    // iOS-aligned elevation ladder (systemBackground / secondarySystemBackground
    // / tertiarySystemBackground). The legacy body / card tokens above
    // point into the first two tiers; menus and modals pull `--surface-3`
    // for the elevated step.
    "--surface-1": "#1c1c1e",
    "--surface-2": "#2c2c2e",
    "--surface-3": "#3a3a3c",

    // Shadow scale (dark): higher alpha than light because shadows over a
    // near-black body need more contrast to be perceptible.
    "--shadow-xs": "0 1px 2px rgba(0, 0, 0, 0.20)",
    "--shadow-sm":
      "0 1px 2px rgba(0, 0, 0, 0.24), 0 1px 3px rgba(0, 0, 0, 0.18)",
    "--shadow-md":
      "0 2px 4px rgba(0, 0, 0, 0.28), 0 4px 12px rgba(0, 0, 0, 0.22)",
    "--shadow-lg":
      "0 4px 8px rgba(0, 0, 0, 0.32), 0 12px 24px rgba(0, 0, 0, 0.28)",
    "--shadow-xl":
      "0 8px 16px rgba(0, 0, 0, 0.38), 0 24px 48px rgba(0, 0, 0, 0.36)",

    // Dark hairlines use a faint white instead of a faint black so the line
    // still reads against the near-black body. Card hairline a touch
    // stronger so grid tiles remain visible even when shadow alpha drops.
    "--surface-border-hairline": "rgba(255, 255, 255, 0.06)",
    "--card-border-hairline": "rgba(255, 255, 255, 0.08)",

    // Mobile card shadow uses a tighter blur but higher alpha to remain
    // visible against the near-black body; desktop keeps a slightly
    // larger spread for a more confident lift.
    "--shadow-card-mobile":
      "0 1px 2px rgba(0, 0, 0, 0.32), 0 2px 6px rgba(0, 0, 0, 0.26)",
    "--shadow-card-desktop":
      "0 1px 2px rgba(0, 0, 0, 0.30), 0 4px 12px rgba(0, 0, 0, 0.24)",
  },
});
