import { LazyMotion, m } from "motion/react";
import type { ReactNode } from "react";

// `domAnimation` is loaded via dynamic import so Vite splits the motion
// feature bundle out of the main app chunk. Resolved once on first mount;
// `LazyMotion` caches the promise internally so we pay it exactly once per
// page load even though many `<m.*>` consumers will mount across the app.
const loadDomAnimation = () =>
  import("./domAnimation").then((mod) => mod.default);

/**
 * Wraps the app tree so callers can use the slim `motion/react-m` `<m.*>`
 * components without pulling the full motion runtime into the main bundle.
 *
 * `strict` enforces the slim component path: importing the full `motion`
 * helper anywhere underneath will throw at runtime in dev so we catch
 * accidental bundle-size regressions early.
 *
 * The hidden `<m.div />` placeholder guarantees the feature bundle is
 * requested at app mount so the first user-visible animation (drawer,
 * modal, card press) does not pay a cold-load latency.
 */
export function MotionProvider({ children }: { children: ReactNode }) {
  return (
    <LazyMotion features={loadDomAnimation} strict>
      <m.div aria-hidden style={{ display: "none" }} />
      {children}
    </LazyMotion>
  );
}
