import { useEffect, useState } from "react";

/**
 * Track the height of the visual viewport while `active` is true.
 *
 * On iOS the on-screen keyboard shrinks only the *visual* viewport; the
 * layout viewport (what `100%`, `100vh`, and even `100dvh` resolve against)
 * keeps its full height. A full-height overlay therefore extends beneath the
 * keyboard and any content there is unreachable, because its scroll container
 * still believes it has the whole screen. `window.visualViewport` is the only
 * signal that reflects the keyboard-reduced height.
 *
 * Returns the current visual viewport height in px, or `null` when inactive
 * or when the Visual Viewport API is unavailable (callers should fall back
 * to their regular CSS sizing).
 */
export function useVisualViewportHeight(active = true): number | null {
  const [height, setHeight] = useState<number | null>(null);

  useEffect(() => {
    if (!active) {
      setHeight(null);
      return;
    }
    const viewport = window.visualViewport;
    if (!viewport) return;

    const update = () => setHeight(viewport.height);
    update();
    viewport.addEventListener("resize", update);
    return () => {
      viewport.removeEventListener("resize", update);
      setHeight(null);
    };
  }, [active]);

  return height;
}
