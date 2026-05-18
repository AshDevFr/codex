/**
 * Shared motion timing tokens. Every drawer, modal, card press, and stagger
 * in the app should pull its curve from this file so the product reads as
 * "one motion language" rather than a patchwork of per-component eases.
 *
 * The cubic-beziers below are tuned to feel close to the iOS default
 * spring-out curve at common interaction durations (200–400ms). The CSS
 * equivalents live in `index.css` as `--ease-out` / `--ease-in-out` so
 * non-motion-lib transitions (Mantine `<Transition>`, plain CSS) can sit on
 * the same curve.
 */

export const EASE_OUT = [0.32, 0.72, 0, 1] as const;
export const EASE_IN_OUT = [0.65, 0, 0.35, 1] as const;

export const SPRING_SOFT = {
  type: "spring",
  stiffness: 220,
  damping: 30,
} as const;

export const SPRING_QUICK = {
  type: "spring",
  stiffness: 360,
  damping: 32,
} as const;
