/**
 * Startup recovery for sessions left behind by a tab that died.
 *
 * A crash, an out-of-memory kill, or a force quit fires no teardown event, so
 * the last checkpoint is all that survives. This sweeps those up on the next
 * load and delivers them, which is what bounds crash loss to one checkpoint
 * interval instead of a whole sitting.
 */

import { readingSessionsApi } from "@/api/readingSessions";
import {
  listCheckpointedBookIds,
  recoverOrphanedSessions,
} from "./ReadingSessionTracker";

/**
 * Deliver any orphaned sessions. Resolves with how many were recovered.
 *
 * Never throws: this runs during bootstrap, and a failure here must not stop
 * the app from starting. Recovered sessions that cannot be sent immediately go
 * to the offline outbox like any other write.
 */
export async function recoverAndFlushSessions(): Promise<number> {
  try {
    const bookIds = listCheckpointedBookIds();
    if (bookIds.length === 0) return 0;

    const sessions = recoverOrphanedSessions(bookIds);
    if (sessions.length === 0) return 0;

    await readingSessionsApi.record(sessions);
    return sessions.length;
  } catch {
    // Recovery is best-effort. The checkpoints have already been consumed, so
    // a failure here loses statistics rather than corrupting anything.
    return 0;
  }
}
