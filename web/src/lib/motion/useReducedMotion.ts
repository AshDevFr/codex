import { useSyncExternalStore } from "react";

const QUERY = "(prefers-reduced-motion: reduce)";

const subscribe = (notify: () => void) => {
  if (typeof window === "undefined" || !window.matchMedia) {
    return () => {};
  }
  const mql = window.matchMedia(QUERY);
  mql.addEventListener("change", notify);
  return () => mql.removeEventListener("change", notify);
};

const getSnapshot = () => {
  if (typeof window === "undefined" || !window.matchMedia) return false;
  return window.matchMedia(QUERY).matches;
};

const getServerSnapshot = () => false;

/**
 * Returns `true` when the user has asked the OS for reduced motion.
 *
 * Motion-driven UI (staggered grids, spring drawers, hover scales) reads
 * this and degrades to instant transitions. CSS-only animations are also
 * collapsed via a global `prefers-reduced-motion` block in `index.css`;
 * this hook is the React-side counterpart so motion-lib variants and
 * gesture handlers can opt out at the props level.
 */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
