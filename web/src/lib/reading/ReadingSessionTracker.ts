/**
 * Measures reading sessions in the browser.
 *
 * Deliberately framework-agnostic: a plain class with an explicit event
 * interface, no React and no direct network access. The iOS client implements
 * the same state machine, and the two are tested against the same table of
 * cases, so aggregate reading time means the same thing wherever it was
 * recorded. Anything that would make the two diverge belongs outside this file.
 *
 * # Why duration is measured rather than derived
 *
 * `clientEndedAt - clientStartedAt` counts a book left open on the nightstand
 * as three hours of reading. So the timer accumulates only while there is
 * evidence the reader is actually reading, and pauses on backgrounding, on tab
 * hide, and after {@link DEFAULT_IDLE_TIMEOUT_MS} without activity.
 *
 * # Why it checkpoints
 *
 * A tab can die without warning: out-of-memory, crash, force quit. None of
 * those fire `pagehide`, so a tracker that only reports on a clean close loses
 * the whole session. Instead the running session is written to storage every
 * {@link DEFAULT_CHECKPOINT_INTERVAL_MS} of *active* time, and recovered on the
 * next load. Worst case one interval is lost rather than everything.
 */

export const DEFAULT_IDLE_TIMEOUT_MS = 5 * 60 * 1000;
export const DEFAULT_CHECKPOINT_INTERVAL_MS = 30 * 1000;

const CHECKPOINT_KEY_PREFIX = "codex.reading.session.";

export type ReadingSessionKind = "progress" | "completed" | "reset";

/** The wire shape of a session, matching `POST /api/v1/reading-sessions`. */
export interface ReadingSessionPayload {
  id: string;
  bookId: string;
  deviceId: string;
  deviceName?: string;
  kind: ReadingSessionKind;
  toPage?: number;
  toPercentage?: number;
  activeDurationMs?: number;
  pagesRead?: number;
  clientStartedAt: string;
  clientEndedAt: string;
}

/** Where the reader currently is. One of the two fields, by format. */
export interface ReadingPosition {
  page?: number;
  percentage?: number;
}

export interface ReadingSessionTrackerOptions {
  bookId: string;
  deviceId: string;
  deviceName?: string;
  /** Called with completed sessions. May be async; rejections are the caller's problem. */
  emit: (sessions: ReadingSessionPayload[]) => void;
  /** Injectable for tests. Defaults to `Date.now`. */
  now?: () => number;
  /** Injectable for tests. Defaults to `crypto.randomUUID`. */
  newId?: () => string;
  /** Injectable for tests. Defaults to `localStorage`. */
  storage?: Storage | null;
  idleTimeoutMs?: number;
  checkpointIntervalMs?: number;
}

/** A running session as persisted between checkpoints. */
interface Checkpoint {
  id: string;
  bookId: string;
  deviceId: string;
  deviceName?: string;
  startedAt: number;
  lastActivityAt: number;
  activeMs: number;
  pagesRead: number;
  page?: number;
  percentage?: number;
}

export class ReadingSessionTracker {
  private readonly options: Required<
    Pick<ReadingSessionTrackerOptions, "idleTimeoutMs" | "checkpointIntervalMs">
  > &
    ReadingSessionTrackerOptions;

  private readonly now: () => number;
  private readonly newId: () => string;
  private readonly storage: Storage | null;

  private current: Checkpoint | null = null;
  /** Wall-clock of the last accounted instant, while running. */
  private lastTickAt = 0;
  private running = false;
  private activeMsAtLastCheckpoint = 0;
  private readonly seenPages = new Set<number>();

  constructor(options: ReadingSessionTrackerOptions) {
    this.options = {
      idleTimeoutMs: DEFAULT_IDLE_TIMEOUT_MS,
      checkpointIntervalMs: DEFAULT_CHECKPOINT_INTERVAL_MS,
      ...options,
    };
    this.now = options.now ?? (() => Date.now());
    this.newId =
      options.newId ??
      (() =>
        typeof crypto !== "undefined" && crypto.randomUUID
          ? crypto.randomUUID()
          : `sess-${Math.random().toString(36).slice(2)}${Date.now().toString(36)}`);
    this.storage =
      options.storage !== undefined ? options.storage : safeStorage();
  }

  /** Whether a session is currently open. */
  get isTracking(): boolean {
    return this.current !== null;
  }

  /**
   * Begin tracking. Idempotent: calling it again while a session is open just
   * records activity, so a re-render cannot start a second session.
   */
  start(position: ReadingPosition = {}): void {
    if (this.current) {
      this.recordActivity(position);
      return;
    }

    const now = this.now();
    this.current = {
      id: this.newId(),
      bookId: this.options.bookId,
      deviceId: this.options.deviceId,
      deviceName: this.options.deviceName,
      startedAt: now,
      lastActivityAt: now,
      activeMs: 0,
      pagesRead: 0,
      ...positionFields(position),
    };
    this.seenPages.clear();
    this.notePage(position);
    this.lastTickAt = now;
    this.running = true;
    this.activeMsAtLastCheckpoint = 0;
    this.writeCheckpoint();
  }

  /**
   * Record reader activity: a page turn, a scroll, a pinch, a TOC jump.
   *
   * If the gap since the last activity exceeded the idle timeout, the previous
   * session is closed and emitted, and a new one begins. That is what stops a
   * book left open overnight from producing one enormous session.
   */
  recordActivity(position: ReadingPosition = {}): void {
    if (!this.current) {
      this.start(position);
      return;
    }

    const now = this.now();
    if (now - this.current.lastActivityAt > this.options.idleTimeoutMs) {
      // The user was away. Close what they actually read, then start fresh.
      this.finish("progress");
      this.start(position);
      return;
    }

    this.accrue(now);
    this.current.lastActivityAt = now;
    Object.assign(this.current, positionFields(position));
    this.notePage(position);
    this.maybeCheckpoint();
  }

  /**
   * Stop accumulating time without ending the session.
   *
   * For app backgrounding, tab hide, and screen lock. The session stays open so
   * returning within the idle timeout continues it rather than fragmenting one
   * sitting into several.
   */
  pause(): void {
    if (!this.current || !this.running) return;
    const now = this.now();
    this.accrue(now);
    // The dwell up to this instant was just credited as reading, so the pause
    // is the reference point for how long the reader was away. Left at the
    // last page turn, a seconds-long interruption late in a quiet dwell would
    // read as an idle gap and split the sitting in {@link resume}.
    this.current.lastActivityAt = now;
    this.running = false;
    this.writeCheckpoint();
  }

  /** Resume after {@link pause}. Time between the two is not counted. */
  resume(): void {
    if (!this.current || this.running) return;
    const now = this.now();

    // Away longer than the timeout: that was a separate sitting.
    if (now - this.current.lastActivityAt > this.options.idleTimeoutMs) {
      this.finish("progress");
      return;
    }

    this.lastTickAt = now;
    this.current.lastActivityAt = now;
    this.running = true;
  }

  /** Mark the book finished and close the session. */
  markCompleted(position: ReadingPosition = {}): void {
    if (!this.current) this.start(position);
    else Object.assign(this.current, positionFields(position));
    this.finish("completed");
  }

  /**
   * Mark the book unread, closing the session and starting a new pass.
   *
   * Sent as its own event rather than as a deletion so the server can order it
   * against a completion made on another device.
   */
  markReset(): void {
    const now = this.now();
    if (this.current) {
      this.finish("progress");
    }

    this.options.emit([
      {
        id: this.newId(),
        bookId: this.options.bookId,
        deviceId: this.options.deviceId,
        deviceName: this.options.deviceName,
        kind: "reset",
        clientStartedAt: new Date(now).toISOString(),
        clientEndedAt: new Date(now).toISOString(),
      },
    ]);
  }

  /** Close and emit the running session. Called when the reader is dismissed. */
  stop(): void {
    if (!this.current) return;
    this.finish("progress");
  }

  /**
   * Persist the running session now, without closing it.
   *
   * For `pagehide`, where the page may never come back and there is no time to
   * do anything but write.
   */
  checkpointNow(): void {
    if (!this.current) return;
    if (this.running) this.accrue(this.now());
    this.writeCheckpoint();
  }

  /** The running session as it would be emitted right now, or null. */
  peek(): ReadingSessionPayload | null {
    if (!this.current) return null;
    return toPayload(this.current, "progress", this.now());
  }

  // ----------------------------------------------------------------
  // Internals
  // ----------------------------------------------------------------

  /**
   * Add elapsed time up to `now` to the running total.
   *
   * A gap longer than the idle timeout contributes nothing: the reader was
   * away, and guessing how much of it was reading would be inventing data.
   */
  private accrue(now: number): void {
    if (!this.running || !this.current) return;
    const delta = now - this.lastTickAt;
    if (delta > 0 && delta <= this.options.idleTimeoutMs) {
      this.current.activeMs += delta;
    }
    this.lastTickAt = now;
  }

  private notePage(position: ReadingPosition): void {
    if (typeof position.page !== "number") return;
    if (this.seenPages.has(position.page)) return;
    this.seenPages.add(position.page);
    if (this.current) this.current.pagesRead = this.seenPages.size;
  }

  private maybeCheckpoint(): void {
    if (!this.current) return;
    const sinceLast = this.current.activeMs - this.activeMsAtLastCheckpoint;
    if (sinceLast >= this.options.checkpointIntervalMs) {
      this.writeCheckpoint();
    }
  }

  private writeCheckpoint(): void {
    if (!this.current || !this.storage) return;
    try {
      this.storage.setItem(
        checkpointKey(this.options.bookId),
        JSON.stringify(this.current),
      );
      this.activeMsAtLastCheckpoint = this.current.activeMs;
    } catch {
      // A full or unavailable store must not break reading. The cost is
      // losing this session if the tab dies, which is the pre-existing
      // behaviour rather than a regression.
    }
  }

  private clearCheckpoint(): void {
    if (!this.storage) return;
    try {
      this.storage.removeItem(checkpointKey(this.options.bookId));
    } catch {
      // Nothing to clear.
    }
  }

  private finish(kind: ReadingSessionKind): void {
    if (!this.current) return;
    const now = this.now();
    if (this.running) this.accrue(now);

    const payload = toPayload(this.current, kind, now);
    this.current = null;
    this.running = false;
    this.seenPages.clear();
    this.activeMsAtLastCheckpoint = 0;
    this.clearCheckpoint();

    // A session with no measured time and no position is not worth a row.
    if (
      kind === "progress" &&
      !payload.activeDurationMs &&
      payload.toPage === undefined &&
      payload.toPercentage === undefined
    ) {
      return;
    }

    this.options.emit([payload]);
  }
}

/**
 * Recover a session left behind by a tab that died without closing it.
 *
 * The recovered session is closed at its last checkpoint rather than at now:
 * everything after that point is unknown, and counting the gap would inflate
 * reading time by however long the browser was shut.
 */
export function recoverOrphanedSessions(
  bookIds: string[],
  storage: Storage | null = safeStorage(),
): ReadingSessionPayload[] {
  if (!storage) return [];

  const recovered: ReadingSessionPayload[] = [];
  for (const bookId of bookIds) {
    const key = checkpointKey(bookId);
    let raw: string | null = null;
    try {
      raw = storage.getItem(key);
    } catch {
      continue;
    }
    if (!raw) continue;

    try {
      const checkpoint = JSON.parse(raw) as Checkpoint;
      if (!checkpoint?.id || !checkpoint.bookId) {
        storage.removeItem(key);
        continue;
      }
      recovered.push(
        toPayload(checkpoint, "progress", checkpoint.lastActivityAt),
      );
    } catch {
      // Unparseable: drop it rather than retry forever.
    }
    try {
      storage.removeItem(key);
    } catch {
      // Best effort.
    }
  }
  return recovered;
}

/** Every book id with a checkpoint waiting, for recovery at startup. */
export function listCheckpointedBookIds(
  storage: Storage | null = safeStorage(),
): string[] {
  if (!storage) return [];
  const ids: string[] = [];
  try {
    for (let i = 0; i < storage.length; i += 1) {
      const key = storage.key(i);
      if (key?.startsWith(CHECKPOINT_KEY_PREFIX)) {
        ids.push(key.slice(CHECKPOINT_KEY_PREFIX.length));
      }
    }
  } catch {
    return [];
  }
  return ids;
}

function checkpointKey(bookId: string): string {
  return `${CHECKPOINT_KEY_PREFIX}${bookId}`;
}

function positionFields(position: ReadingPosition): Partial<Checkpoint> {
  const fields: Partial<Checkpoint> = {};
  if (typeof position.page === "number") fields.page = position.page;
  if (typeof position.percentage === "number") {
    fields.percentage = position.percentage;
  }
  return fields;
}

function toPayload(
  checkpoint: Checkpoint,
  kind: ReadingSessionKind,
  endedAt: number,
): ReadingSessionPayload {
  const payload: ReadingSessionPayload = {
    id: checkpoint.id,
    bookId: checkpoint.bookId,
    deviceId: checkpoint.deviceId,
    kind,
    clientStartedAt: new Date(checkpoint.startedAt).toISOString(),
    // A session cannot end before the time it accrued. Recovery ends a session
    // at its last recorded activity, and `checkpointNow` writes time that no
    // activity followed, so without this floor a session rescued from a crash
    // can report minutes of reading inside a zero-length span.
    clientEndedAt: new Date(
      Math.max(endedAt, checkpoint.startedAt + checkpoint.activeMs),
    ).toISOString(),
  };
  if (checkpoint.deviceName) payload.deviceName = checkpoint.deviceName;
  if (typeof checkpoint.page === "number") payload.toPage = checkpoint.page;
  if (typeof checkpoint.percentage === "number") {
    payload.toPercentage = checkpoint.percentage;
  }
  if (checkpoint.activeMs > 0) payload.activeDurationMs = checkpoint.activeMs;
  if (checkpoint.pagesRead > 0) payload.pagesRead = checkpoint.pagesRead;
  return payload;
}

function safeStorage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}
