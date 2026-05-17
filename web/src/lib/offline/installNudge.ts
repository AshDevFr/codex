/**
 * iOS Safari install nudge (Phase 12 T10).
 *
 * On a non-installed iOS Safari tab the browser is allowed to evict our
 * IndexedDB + Cache Storage after ~7 days of inactivity, even after
 * `navigator.storage.persist()` returns true (it returns false on Safari
 * tabs). On every other surface (Chrome tab, Android, installed iOS PWA)
 * downloads are durable enough that warning the user would be noise.
 *
 * This module owns the "should we nudge?" predicate and the dismissal
 * persistence. It is intentionally framework-agnostic so any download
 * surface (per-book button, series-batch button) can call it before
 * kicking off its first download in a session.
 *
 * Dismissal:
 * - Persisted in localStorage under `INSTALL_NUDGE_DISMISSED_KEY` with a
 *   30-day TTL, matching the convention used by `InstallPrompt.tsx`.
 * - Both "Continue anyway" and "Show me how to install" (after the user
 *   reads the modal) record dismissal so we do not re-nag every tap.
 */

export const INSTALL_NUDGE_DISMISSED_KEY =
  "codex-offline-install-nudge-dismissed";
export const INSTALL_NUDGE_TTL_MS = 1000 * 60 * 60 * 24 * 30;

export function isIosUserAgent(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  const isIPad =
    /iPad/.test(ua) ||
    (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
  return /iPhone|iPod/.test(ua) || isIPad;
}

export function isStandaloneDisplay(): boolean {
  if (typeof window === "undefined") return false;
  const standaloneMedia = window.matchMedia?.(
    "(display-mode: standalone)",
  ).matches;
  const iosStandalone =
    "standalone" in window.navigator &&
    (window.navigator as { standalone?: boolean }).standalone === true;
  return Boolean(standaloneMedia || iosStandalone);
}

export function isNudgeDismissed(now: number = Date.now()): boolean {
  if (typeof window === "undefined") return true;
  try {
    const raw = window.localStorage.getItem(INSTALL_NUDGE_DISMISSED_KEY);
    if (!raw) return false;
    const ts = Number.parseInt(raw, 10);
    if (Number.isNaN(ts)) return false;
    return now - ts < INSTALL_NUDGE_TTL_MS;
  } catch {
    // Treat storage errors (private mode, etc.) as "dismissed" so we do
    // not loop the modal in environments where we cannot record consent.
    return true;
  }
}

export function recordNudgeDismissal(now: number = Date.now()): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(INSTALL_NUDGE_DISMISSED_KEY, String(now));
  } catch {
    /* storage unavailable — silently ignore */
  }
}

/**
 * Should the iOS install nudge be shown before the next download?
 *
 * True when the runtime is an iOS Safari tab that has not yet been added
 * to the home screen, and the user has not dismissed the modal in the
 * past 30 days. Returns false everywhere else (installed PWA, other
 * browsers, server-side rendering).
 */
export function shouldShowInstallNudge(): boolean {
  if (!isIosUserAgent()) return false;
  if (isStandaloneDisplay()) return false;
  if (isNudgeDismissed()) return false;
  return true;
}
