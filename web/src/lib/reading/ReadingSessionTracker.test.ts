/**
 * Drives the tracker through the shared reading-session case table.
 *
 * The cases are not written here. They live in `contracts/reading-sessions.json`
 * at the repository root, and the iOS client is tested against that same file.
 * If a case changes it changes for both, or aggregate reading time silently
 * becomes a blend of two different metrics.
 *
 * Only the mapping from a case's events onto this tracker's API belongs in this
 * file, along with the browser-specific storage failures the contract
 * deliberately leaves to each platform.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import contractDocument from "../../../../contracts/reading-sessions.json";
import {
  DEFAULT_CHECKPOINT_INTERVAL_MS,
  DEFAULT_IDLE_TIMEOUT_MS,
  listCheckpointedBookIds,
  type ReadingPosition,
  type ReadingSessionPayload,
  ReadingSessionTracker,
  recoverOrphanedSessions,
} from "./ReadingSessionTracker";

const CHECKPOINT_KEY_PREFIX = "codex.reading.session.";

interface ContractEvent extends ReadingPosition {
  kind:
    | "start"
    | "activity"
    | "pause"
    | "resume"
    | "stop"
    | "complete"
    | "reset"
    | "checkpoint"
    | "crash"
    | "recover";
  atMs: number;
}

/** A `null` means the field must be absent; `spanMs` is derived from the timestamps. */
type ExpectedSession = Record<string, string | number | null>;

interface ContractCase {
  name: string;
  group: string;
  why?: string;
  bookId?: string;
  persistence?: "unavailable";
  seedCheckpoints?: { bookId: string; raw: string }[];
  events: ContractEvent[];
  expect: {
    sessions?: ExpectedSession[];
    recoveries?: ExpectedSession[][];
    totalActiveDurationMs?: number;
    tracking?: boolean;
    checkpointedBookIds?: string[];
  };
}

interface Contract {
  version: number;
  subject: { bookId: string; deviceId: string; deviceName: string };
  thresholds: { idleTimeoutMs: number; checkpointIntervalMs: number };
  cases: ContractCase[];
}

const contract = contractDocument as unknown as Contract;

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

interface CaseResult {
  emitted: ReadingSessionPayload[];
  recoveries: ReadingSessionPayload[][];
  tracking: boolean;
  checkpointedBookIds: string[];
}

function runCase(testCase: ContractCase): CaseResult {
  const bookId = testCase.bookId ?? contract.subject.bookId;
  const storage =
    testCase.persistence === "unavailable" ? null : memoryStorage();

  for (const seed of testCase.seedCheckpoints ?? []) {
    storage?.setItem(`${CHECKPOINT_KEY_PREFIX}${seed.bookId}`, seed.raw);
  }

  let clock = 0;
  let counter = 0;
  const emitted: ReadingSessionPayload[] = [];
  const recoveries: ReadingSessionPayload[][] = [];

  let tracker: ReadingSessionTracker | null = new ReadingSessionTracker({
    bookId,
    deviceId: contract.subject.deviceId,
    deviceName: contract.subject.deviceName,
    emit: (sessions) => emitted.push(...sessions),
    now: () => clock,
    newId: () => `session-${++counter}`,
    storage,
    idleTimeoutMs: contract.thresholds.idleTimeoutMs,
    checkpointIntervalMs: contract.thresholds.checkpointIntervalMs,
  });

  for (const event of testCase.events) {
    clock = event.atMs;
    const position: ReadingPosition = {};
    if (typeof event.page === "number") position.page = event.page;
    if (typeof event.percentage === "number") {
      position.percentage = event.percentage;
    }

    switch (event.kind) {
      case "start":
        tracker?.start(position);
        break;
      case "activity":
        tracker?.recordActivity(position);
        break;
      case "pause":
        tracker?.pause();
        break;
      case "resume":
        tracker?.resume();
        break;
      case "stop":
        tracker?.stop();
        break;
      case "complete":
        tracker?.markCompleted(position);
        break;
      case "reset":
        tracker?.markReset();
        break;
      case "checkpoint":
        tracker?.checkpointNow();
        break;
      case "crash":
        // The process dies with no chance to close: drop the tracker with the
        // store exactly as its last checkpoint left it.
        tracker = null;
        break;
      case "recover":
        recoveries.push(recoverOrphanedSessions([bookId], storage));
        break;
    }
  }

  return {
    emitted,
    recoveries,
    tracking: tracker?.isTracking ?? false,
    checkpointedBookIds: listCheckpointedBookIds(storage).sort(),
  };
}

function assertSessions(
  actual: ReadingSessionPayload[],
  expected: ExpectedSession[],
  label: string,
): void {
  expect(actual, `${label}: session count`).toHaveLength(expected.length);

  expected.forEach((expectation, index) => {
    const session = actual[index] as unknown as Record<string, unknown>;
    const where = `${label}[${index}]`;

    for (const [field, value] of Object.entries(expectation)) {
      if (field === "spanMs") {
        const span =
          new Date(actual[index].clientEndedAt).getTime() -
          new Date(actual[index].clientStartedAt).getTime();
        expect(span, `${where}.spanMs`).toBe(value);
      } else if (value === null) {
        expect(
          session[field],
          `${where}.${field} must be absent`,
        ).toBeUndefined();
      } else if (typeof value === "number" && !Number.isInteger(value)) {
        expect(session[field] as number, `${where}.${field}`).toBeCloseTo(
          value,
        );
      } else {
        expect(session[field], `${where}.${field}`).toBe(value);
      }
    }
  });
}

function assertInvariants(sessions: ReadingSessionPayload[]): void {
  const ids = new Set<string>();

  for (const session of sessions) {
    const startedAt = new Date(session.clientStartedAt).getTime();
    const endedAt = new Date(session.clientEndedAt).getTime();

    expect(endedAt, "clientEndedAt >= clientStartedAt").toBeGreaterThanOrEqual(
      startedAt,
    );
    expect(
      session.activeDurationMs ?? 0,
      "activeDurationMs <= session span",
    ).toBeLessThanOrEqual(endedAt - startedAt);

    expect(ids.has(session.id), `duplicate session id ${session.id}`).toBe(
      false,
    );
    ids.add(session.id);
  }
}

describe("reading-session contract", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("is the version the tracker's defaults were written for", () => {
    expect(contract.version).toBe(1);
    expect(contract.thresholds.idleTimeoutMs).toBe(DEFAULT_IDLE_TIMEOUT_MS);
    expect(contract.thresholds.checkpointIntervalMs).toBe(
      DEFAULT_CHECKPOINT_INTERVAL_MS,
    );
  });

  const groups = [...new Set(contract.cases.map((c) => c.group))];

  for (const group of groups) {
    describe(group, () => {
      for (const testCase of contract.cases.filter((c) => c.group === group)) {
        it(testCase.name, () => {
          const result = runCase(testCase);

          if (testCase.expect.sessions) {
            assertSessions(
              result.emitted,
              testCase.expect.sessions,
              "sessions",
            );
          }

          if (testCase.expect.recoveries) {
            expect(result.recoveries, "recovery count").toHaveLength(
              testCase.expect.recoveries.length,
            );
            testCase.expect.recoveries.forEach((expected, index) => {
              assertSessions(
                result.recoveries[index],
                expected,
                `recoveries[${index}]`,
              );
            });
          }

          if (testCase.expect.totalActiveDurationMs !== undefined) {
            const total = result.emitted.reduce(
              (sum, session) => sum + (session.activeDurationMs ?? 0),
              0,
            );
            expect(total, "totalActiveDurationMs").toBe(
              testCase.expect.totalActiveDurationMs,
            );
          }

          if (testCase.expect.tracking !== undefined) {
            expect(result.tracking, "tracking").toBe(testCase.expect.tracking);
          }

          if (testCase.expect.checkpointedBookIds) {
            expect(result.checkpointedBookIds, "checkpointedBookIds").toEqual(
              testCase.expect.checkpointedBookIds,
            );
          }

          assertInvariants([...result.emitted, ...result.recoveries.flat()]);
        });
      }
    });
  }
});

/**
 * The contract says measurement must survive a store it cannot write to, and
 * leaves it to each platform to say how that happens. On the web there are two
 * routes into it, and both have to land on the same behaviour.
 */
describe("browser storage failures", () => {
  const MINUTE = 60 * 1000;

  function trackerWith(storage: Storage | null) {
    const emitted: ReadingSessionPayload[] = [];
    let clock = 0;
    const tracker = new ReadingSessionTracker({
      bookId: contract.subject.bookId,
      deviceId: contract.subject.deviceId,
      emit: (sessions) => emitted.push(...sessions),
      now: () => clock,
      newId: () => "session-1",
      storage,
    });
    return { tracker, emitted, advance: (ms: number) => (clock += ms) };
  }

  it("keeps measuring when storage is unavailable", () => {
    const { tracker, emitted, advance } = trackerWith(null);

    tracker.start({ page: 1 });
    advance(MINUTE);
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
    const { tracker, emitted, advance } = trackerWith(throwing);

    expect(() => {
      tracker.start({ page: 1 });
      advance(MINUTE);
      tracker.stop();
    }).not.toThrow();
    expect(emitted).toHaveLength(1);
  });
});
