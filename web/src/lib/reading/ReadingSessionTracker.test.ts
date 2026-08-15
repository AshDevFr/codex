/**
 * The shared case table for reading-session measurement.
 *
 * The iOS client implements the same state machine and is tested against this
 * same list of cases. If a case changes here it has to change there too, or
 * aggregate reading time silently becomes a blend of two different metrics.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_CHECKPOINT_INTERVAL_MS,
  DEFAULT_IDLE_TIMEOUT_MS,
  listCheckpointedBookIds,
  type ReadingSessionPayload,
  ReadingSessionTracker,
  recoverOrphanedSessions,
} from "./ReadingSessionTracker";

const BOOK = "book-1";
const DEVICE = "device-1";
const MINUTE = 60 * 1000;

/** An in-memory Storage, so tests never depend on jsdom's localStorage state. */
function memoryStorage(): Storage {
  const map = new Map<string, string>();
  return {
    get length() {
      return map.size;
    },
    clear: () => map.clear(),
    getItem: (k: string) => map.get(k) ?? null,
    key: (i: number) => Array.from(map.keys())[i] ?? null,
    removeItem: (k: string) => {
      map.delete(k);
    },
    setItem: (k: string, v: string) => {
      map.set(k, v);
    },
  } as Storage;
}

function setup(overrides: { idleTimeoutMs?: number } = {}) {
  let clock = 0;
  let counter = 0;
  const emitted: ReadingSessionPayload[] = [];
  const storage = memoryStorage();

  const tracker = new ReadingSessionTracker({
    bookId: BOOK,
    deviceId: DEVICE,
    deviceName: "Test Device",
    emit: (sessions) => emitted.push(...sessions),
    now: () => clock,
    newId: () => `session-${++counter}`,
    storage,
    ...overrides,
  });

  return {
    tracker,
    emitted,
    storage,
    advance: (ms: number) => {
      clock += ms;
    },
    at: () => clock,
  };
}

describe("ReadingSessionTracker", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  describe("measuring active time", () => {
    it("accumulates time between activity events", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 2 });
      advance(MINUTE);
      tracker.recordActivity({ page: 3 });
      tracker.stop();

      expect(emitted).toHaveLength(1);
      expect(emitted[0].activeDurationMs).toBe(2 * MINUTE);
    });

    it("does not count a gap longer than the idle timeout", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 2 });

      // Away for an hour, then back. The gap closes the session.
      advance(60 * MINUTE);
      tracker.recordActivity({ page: 3 });
      tracker.stop();

      const total = emitted.reduce(
        (sum, s) => sum + (s.activeDurationMs ?? 0),
        0,
      );
      expect(total).toBe(MINUTE);
    });

    it("splits a sitting into separate sessions across an idle gap", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 2 });

      advance(DEFAULT_IDLE_TIMEOUT_MS + MINUTE);
      tracker.recordActivity({ page: 3 });
      advance(2 * MINUTE);
      tracker.recordActivity({ page: 4 });
      tracker.stop();

      expect(emitted).toHaveLength(2);
      expect(emitted[0].activeDurationMs).toBe(MINUTE);
      expect(emitted[1].activeDurationMs).toBe(2 * MINUTE);
    });

    it("counts a long but sub-timeout pause on one page as reading", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      // Four minutes on a dense page is reading, not idling.
      advance(4 * MINUTE);
      tracker.recordActivity({ page: 2 });
      tracker.stop();

      expect(emitted[0].activeDurationMs).toBe(4 * MINUTE);
    });

    it("stops the clock while paused", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.pause();

      advance(3 * MINUTE); // backgrounded
      tracker.resume();

      advance(MINUTE);
      tracker.recordActivity({ page: 2 });
      tracker.stop();

      expect(emitted[0].activeDurationMs).toBe(2 * MINUTE);
    });

    it("closes the session when a pause outlasts the idle timeout", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.pause();

      advance(DEFAULT_IDLE_TIMEOUT_MS + MINUTE);
      tracker.resume();

      expect(emitted).toHaveLength(1);
      expect(emitted[0].activeDurationMs).toBe(MINUTE);
      expect(tracker.isTracking).toBe(false);
    });

    it("never reports more active time than the session's own span", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.pause();
      advance(10 * MINUTE);
      tracker.resume();
      advance(MINUTE);
      tracker.stop();

      const session = emitted[0];
      const span =
        new Date(session.clientEndedAt).getTime() -
        new Date(session.clientStartedAt).getTime();
      expect(session.activeDurationMs ?? 0).toBeLessThanOrEqual(span);
    });
  });

  describe("position and pages", () => {
    it("reports the last position reached", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 40 });
      tracker.stop();

      expect(emitted[0].toPage).toBe(40);
    });

    it("reports a deliberate rewind as the final position", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 50 });
      advance(MINUTE);
      tracker.recordActivity({ page: 49 });
      tracker.stop();

      expect(emitted[0].toPage).toBe(49);
    });

    it("counts distinct pages rather than page events", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 2 });
      tracker.recordActivity({ page: 3 });
      tracker.recordActivity({ page: 2 }); // back
      tracker.recordActivity({ page: 3 }); // forward again
      tracker.stop();

      expect(emitted[0].pagesRead).toBe(3);
    });

    it("carries an EPUB percentage instead of a page", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ percentage: 0.1 });
      advance(MINUTE);
      tracker.recordActivity({ percentage: 0.42 });
      tracker.stop();

      expect(emitted[0].toPercentage).toBeCloseTo(0.42);
      expect(emitted[0].toPage).toBeUndefined();
    });
  });

  describe("completion and reset", () => {
    it("emits a completed session", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.markCompleted({ page: 100 });

      expect(emitted).toHaveLength(1);
      expect(emitted[0].kind).toBe("completed");
      expect(emitted[0].toPage).toBe(100);
    });

    it("emits a reset as its own event", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 20 });
      advance(MINUTE);
      tracker.markReset();

      expect(emitted).toHaveLength(2);
      expect(emitted[0].kind).toBe("progress");
      expect(emitted[1].kind).toBe("reset");
    });

    it("closes tracking after completing", () => {
      const { tracker, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.markCompleted({ page: 100 });

      expect(tracker.isTracking).toBe(false);
    });
  });

  describe("session hygiene", () => {
    it("does not emit a session with nothing to report", () => {
      const { tracker, emitted } = setup();

      tracker.start();
      tracker.stop();

      expect(emitted).toHaveLength(0);
    });

    it("does not start a second session on a repeated start", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.stop();

      expect(emitted).toHaveLength(1);
    });

    it("ignores resume without a preceding pause", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.resume();
      advance(MINUTE);
      tracker.stop();

      expect(emitted[0].activeDurationMs).toBe(2 * MINUTE);
    });

    it("gives each session a distinct id", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.stop();

      tracker.start({ page: 2 });
      advance(MINUTE);
      tracker.stop();

      expect(emitted[0].id).not.toBe(emitted[1].id);
    });

    it("emits ISO timestamps that bracket the session", () => {
      const { tracker, emitted, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.stop();

      const { clientStartedAt, clientEndedAt } = emitted[0];
      expect(new Date(clientEndedAt).getTime()).toBeGreaterThanOrEqual(
        new Date(clientStartedAt).getTime(),
      );
    });
  });

  describe("checkpointing and crash recovery", () => {
    it("checkpoints after an interval of active time", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(DEFAULT_CHECKPOINT_INTERVAL_MS + 1000);
      tracker.recordActivity({ page: 2 });

      expect(listCheckpointedBookIds(storage)).toEqual([BOOK]);
    });

    it("recovers a session left by a tab that died", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(2 * MINUTE);
      tracker.recordActivity({ page: 12 });
      tracker.checkpointNow();

      // The tab dies here: no stop(), no pagehide.
      const recovered = recoverOrphanedSessions([BOOK], storage);

      expect(recovered).toHaveLength(1);
      expect(recovered[0].toPage).toBe(12);
      expect(recovered[0].activeDurationMs).toBe(2 * MINUTE);
    });

    it("closes a recovered session at its checkpoint, not at recovery time", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.recordActivity({ page: 5 });
      tracker.checkpointNow();

      const recovered = recoverOrphanedSessions([BOOK], storage);
      const span =
        new Date(recovered[0].clientEndedAt).getTime() -
        new Date(recovered[0].clientStartedAt).getTime();

      expect(span).toBe(MINUTE);
    });

    it("loses at most the time since the last checkpoint", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(DEFAULT_CHECKPOINT_INTERVAL_MS);
      tracker.recordActivity({ page: 2 }); // triggers a checkpoint
      advance(10 * 1000); // unrecorded when the tab dies

      const recovered = recoverOrphanedSessions([BOOK], storage);
      expect(recovered[0].activeDurationMs).toBe(
        DEFAULT_CHECKPOINT_INTERVAL_MS,
      );
    });

    it("clears the checkpoint on a clean close so nothing is double-counted", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.stop();

      expect(listCheckpointedBookIds(storage)).toEqual([]);
      expect(recoverOrphanedSessions([BOOK], storage)).toEqual([]);
    });

    it("consumes a recovered checkpoint so it is not recovered twice", () => {
      const { tracker, storage, advance } = setup();

      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.checkpointNow();

      expect(recoverOrphanedSessions([BOOK], storage)).toHaveLength(1);
      expect(recoverOrphanedSessions([BOOK], storage)).toHaveLength(0);
    });

    it("discards an unparseable checkpoint rather than retrying forever", () => {
      const storage = memoryStorage();
      storage.setItem("codex.reading.session.book-9", "{not json");

      expect(recoverOrphanedSessions(["book-9"], storage)).toEqual([]);
      expect(listCheckpointedBookIds(storage)).toEqual([]);
    });
  });

  describe("degraded environments", () => {
    it("keeps measuring when storage is unavailable", () => {
      const emitted: ReadingSessionPayload[] = [];
      let clock = 0;
      const tracker = new ReadingSessionTracker({
        bookId: BOOK,
        deviceId: DEVICE,
        emit: (s) => emitted.push(...s),
        now: () => clock,
        newId: () => "session-1",
        storage: null,
      });

      tracker.start({ page: 1 });
      clock += MINUTE;
      tracker.stop();

      expect(emitted[0].activeDurationMs).toBe(MINUTE);
    });

    it("survives a storage that throws on write", () => {
      const throwing = {
        ...memoryStorage(),
        setItem: () => {
          throw new Error("QuotaExceededError");
        },
      } as unknown as Storage;

      const emitted: ReadingSessionPayload[] = [];
      let clock = 0;
      const tracker = new ReadingSessionTracker({
        bookId: BOOK,
        deviceId: DEVICE,
        emit: (s) => emitted.push(...s),
        now: () => clock,
        newId: () => "session-1",
        storage: throwing,
      });

      expect(() => {
        tracker.start({ page: 1 });
        clock += MINUTE;
        tracker.stop();
      }).not.toThrow();
      expect(emitted).toHaveLength(1);
    });
  });
});
