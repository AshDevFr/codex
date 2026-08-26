import { userSeriesReaderSettingsApi } from "@/api/userSeriesReaderSettings";
import {
  SERIES_KEY_SUFFIX,
  STORAGE_KEY_PREFIX,
} from "@/components/reader/hooks/useSeriesReaderSettings";
import type { ReadingDirection } from "@/store/readerStore";

/** Marks a user as migrated so this runs once, not on every load. */
const MIGRATED_FLAG_PREFIX = "codex-reader-direction-migrated-";

const VALID_DIRECTIONS: readonly string[] = ["ltr", "rtl", "ttb", "webtoon"];

export function migratedFlagKey(userId: string): string {
  return `${MIGRATED_FLAG_PREFIX}${userId}`;
}

interface PendingMigration {
  storageKey: string;
  seriesId: string;
  direction: ReadingDirection;
  blob: Record<string, unknown>;
}

/** Every per-series blob for this user that still carries a reading direction. */
function findPending(userId: string): PendingMigration[] {
  const prefix = `${STORAGE_KEY_PREFIX}${userId}${SERIES_KEY_SUFFIX}`;
  const pending: PendingMigration[] = [];

  for (let index = 0; index < localStorage.length; index++) {
    const storageKey = localStorage.key(index);
    if (!storageKey?.startsWith(prefix)) continue;

    const seriesId = storageKey.slice(prefix.length);
    if (!seriesId) continue;

    const raw = localStorage.getItem(storageKey);
    if (!raw) continue;

    try {
      const blob: unknown = JSON.parse(raw);
      if (typeof blob !== "object" || blob === null) continue;

      const record = blob as Record<string, unknown>;
      const direction = record.readingDirection;
      if (
        typeof direction !== "string" ||
        !VALID_DIRECTIONS.includes(direction)
      ) {
        continue;
      }

      pending.push({
        storageKey,
        seriesId,
        direction: direction as ReadingDirection,
        blob: record,
      });
    } catch {
      // An unparseable blob has nothing to migrate. The reader already ignores
      // it, so leave it where it is rather than destroying something a future
      // version might recover.
    }
  }

  return pending;
}

/** Rewrite a blob without its direction, leaving the device settings alone. */
function stripDirection(storageKey: string, blob: Record<string, unknown>) {
  const { readingDirection: _dropped, ...rest } = blob;
  try {
    localStorage.setItem(storageKey, JSON.stringify(rest));
  } catch {
    // Out of quota, or storage is unavailable. The server already has the
    // direction, and the local copy is ignored from here on, so this is not
    // worth failing the migration over.
  }
}

/**
 * Move per-series reading directions from this browser to the account.
 *
 * They used to live in `localStorage`, which meant they died with a browser
 * profile and never reached a second device. Running this once per user turns
 * an upgrade into something they do not notice, rather than one where their
 * per-series directions silently vanish.
 *
 * A direction already stored on the server wins: it was set through the current
 * UI, so it is the more recent intent, and overwriting it with a stale local
 * value would undo a correction the user just made.
 *
 * Returns the number of series migrated. Safe to call repeatedly: it stops at
 * the flag, and a failed upload leaves the local value in place so the next
 * load tries again.
 */
export async function migrateSeriesReadingDirections(
  userId: string,
): Promise<number> {
  const flagKey = migratedFlagKey(userId);

  try {
    if (localStorage.getItem(flagKey)) return 0;
  } catch {
    // No storage means nothing to migrate.
    return 0;
  }

  const pending = findPending(userId);
  if (pending.length === 0) {
    try {
      localStorage.setItem(flagKey, "1");
    } catch {
      // Nothing was migrated, so a missing flag only costs another empty scan.
    }
    return 0;
  }

  let migrated = 0;
  let allHandled = true;

  for (const entry of pending) {
    try {
      const existing = await userSeriesReaderSettingsApi.get(entry.seriesId);
      if (!existing.readingDirection) {
        await userSeriesReaderSettingsApi.patch(entry.seriesId, {
          readingDirection: entry.direction,
        });
        migrated++;
      }
      stripDirection(entry.storageKey, entry.blob);
    } catch (error) {
      // A series that no longer exists cannot be migrated and never will be, so
      // drop the local value rather than retrying it forever.
      const status = (error as { response?: { status?: number } })?.response
        ?.status;
      if (status === 404) {
        stripDirection(entry.storageKey, entry.blob);
        continue;
      }

      // Anything else is probably transient. Leave the local value and the
      // flag alone so the next load picks it up again.
      allHandled = false;
    }
  }

  if (allHandled) {
    try {
      localStorage.setItem(flagKey, "1");
    } catch {
      // Worst case the scan runs again and finds nothing left to do.
    }
  }

  return migrated;
}
