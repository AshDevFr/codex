import { useEffect } from "react";

const VIEWPORT_SELECTOR = 'meta[name="viewport"]';

/** Tokens that disable native pinch / double-tap zoom of the visual viewport. */
const LOCK_TOKENS: Record<string, string> = {
  "maximum-scale": "1",
  "user-scalable": "no",
};

const DEFAULT_LOCKED_CONTENT =
  "width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover";

/** Parse a viewport `content` string into an ordered key→value map (value null for bare tokens). */
function parseViewport(content: string): Map<string, string | null> {
  const map = new Map<string, string | null>();
  for (const part of content.split(",")) {
    const token = part.trim();
    if (!token) continue;
    const eq = token.indexOf("=");
    if (eq === -1) {
      map.set(token.toLowerCase(), null);
    } else {
      map.set(
        token.slice(0, eq).trim().toLowerCase(),
        token.slice(eq + 1).trim(),
      );
    }
  }
  return map;
}

function serializeViewport(map: Map<string, string | null>): string {
  return Array.from(map.entries())
    .map(([key, value]) => (value === null ? key : `${key}=${value}`))
    .join(", ");
}

/**
 * Disable the browser's native visual-viewport zoom (pinch + double-tap) for as
 * long as the calling component is mounted, restoring the prior state on unmount.
 *
 * Used by the full-screen reader, where native zoom is undesirable: it scales the
 * whole UI (toolbar included) rather than the page, and on iOS Safari it ignores
 * `user-scalable=no` and can't be reliably zoomed back out. Content-only zoom is
 * implemented separately on the page element.
 *
 * Two levers, because no single one covers every browser:
 * - Viewport `<meta>` gains `maximum-scale=1, user-scalable=no` (Android/others).
 * - `gesturestart`/`gesturechange` are `preventDefault`ed (the only thing that
 *   stops pinch-zoom on iOS Safari, which ignores the meta tag). App-level pinch
 *   uses Pointer Events, which fire independently of these proprietary events.
 */
export function useViewportZoomLock(active = true): void {
  useEffect(() => {
    if (!active || typeof document === "undefined") return;

    const existing = document.querySelector<HTMLMetaElement>(VIEWPORT_SELECTOR);
    const created = existing === null;
    const originalContent = existing?.getAttribute("content") ?? null;

    let meta = existing;
    if (meta) {
      const map = parseViewport(originalContent ?? "");
      for (const [key, value] of Object.entries(LOCK_TOKENS)) {
        map.set(key, value);
      }
      meta.setAttribute("content", serializeViewport(map));
    } else {
      meta = document.createElement("meta");
      meta.setAttribute("name", "viewport");
      meta.setAttribute("content", DEFAULT_LOCKED_CONTENT);
      document.head.appendChild(meta);
    }

    // iOS Safari pinch-zoom fires the proprietary gesture* events; blocking them
    // is the only reliable way to stop it scaling the viewport.
    const preventGesture = (event: Event) => event.preventDefault();
    document.addEventListener("gesturestart", preventGesture, {
      passive: false,
    });
    document.addEventListener("gesturechange", preventGesture, {
      passive: false,
    });

    return () => {
      document.removeEventListener("gesturestart", preventGesture);
      document.removeEventListener("gesturechange", preventGesture);
      if (created) {
        meta?.remove();
      } else if (originalContent !== null) {
        meta?.setAttribute("content", originalContent);
      } else {
        meta?.removeAttribute("content");
      }
    };
  }, [active]);
}
