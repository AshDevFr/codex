/**
 * Stable per-install identity for reading sessions.
 *
 * Reading statistics break down by device, and the server uses the device to
 * decide which sessions may merge with each other, so this has to survive
 * reloads and be distinct per browser profile. It is deliberately *not* the
 * `codex-web` constant used by R2Progression: that one identifies "the Codex
 * web reader" as a kind of client so the EPUB reader can tell whether a
 * position came from somewhere else, and making it per-install would break
 * that comparison.
 *
 * Not a security boundary and not tied to a user. Clearing site data yields a
 * new device, which shows up as a new row in statistics and is the honest
 * outcome: it genuinely is a fresh install as far as anything here can tell.
 */

const STORAGE_KEY = "codex.reading.deviceId";

function randomId(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return crypto.randomUUID();
  }
  // Older Safari and non-secure contexts have no randomUUID. The id only has
  // to be unlikely to collide across one user's own devices.
  return `dev-${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`;
}

/**
 * The identifier for this install, creating and persisting one on first call.
 *
 * Falls back to a per-session id when storage is unavailable (private mode,
 * blocked cookies). Statistics then treat each page load as its own device,
 * which is wrong but harmless, and much better than throwing inside a reader.
 */
export function getDeviceId(
  storage: Storage | undefined = safeStorage(),
): string {
  if (!storage) return randomId();

  try {
    const existing = storage.getItem(STORAGE_KEY);
    if (existing) return existing;

    const fresh = randomId();
    storage.setItem(STORAGE_KEY, fresh);
    return fresh;
  } catch {
    return randomId();
  }
}

/**
 * A human-readable label for this device, shown in reading statistics.
 *
 * Deliberately coarse. The point is to tell "my laptop" from "my phone" in a
 * list, not to fingerprint the browser, so this reports the broad platform and
 * nothing more.
 */
export function getDeviceName(): string {
  if (typeof navigator === "undefined") return "Codex Web";

  const ua = navigator.userAgent;
  if (/iPad/i.test(ua)) return "Codex Web (iPad)";
  if (/iPhone|iPod/i.test(ua)) return "Codex Web (iPhone)";
  if (/Android/i.test(ua)) return "Codex Web (Android)";
  if (/Macintosh/i.test(ua)) return "Codex Web (Mac)";
  if (/Windows/i.test(ua)) return "Codex Web (Windows)";
  if (/Linux/i.test(ua)) return "Codex Web (Linux)";
  return "Codex Web";
}

function safeStorage(): Storage | undefined {
  try {
    return typeof localStorage === "undefined" ? undefined : localStorage;
  } catch {
    return undefined;
  }
}

/** Test-only: forget the stored identity. */
export function _resetDeviceIdForTests(
  storage: Storage | undefined = safeStorage(),
): void {
  try {
    storage?.removeItem(STORAGE_KEY);
  } catch {
    // Nothing to clear.
  }
}
