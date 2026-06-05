import { useCallback, useSyncExternalStore } from "react";
import {
  fetchPendingTaskCounts,
  fetchTasksByStatus,
  type PendingTaskCounts,
  subscribeToTaskProgress,
} from "@/api/tasks";
import { usePermissions } from "@/hooks/usePermissions";
import { useAuthStore } from "@/store/authStore";
import type { ActiveTask, TaskProgressEvent, TaskStatus } from "@/types";
import { PERMISSIONS } from "@/types/permissions";

type ConnectionState = "connecting" | "connected" | "disconnected" | "failed";

/** Poll cadence for the processing-tasks / pending-counts backstop. SSE pushes
 * live progress, so polling only catches what the event stream missed — and is
 * adaptive: fast while work is in flight, slow when idle. With nothing running
 * there is nothing to reconcile, so a long idle interval keeps an open tab from
 * hammering `/tasks` + `/tasks/stats` forever (SSE still announces new work the
 * instant it starts, which flips polling back to the active cadence). */
const POLL_ACTIVE_MS = 10_000;
const POLL_IDLE_MS = 60_000;
/** How long a completed/failed task lingers in the active list so the UI can
 * show its terminal state before it disappears. */
const COMPLETED_TASK_LINGER_MS = 5_000;

type TaskProgressSnapshot = {
  activeTasks: ActiveTask[];
  connectionState: ConnectionState;
  pendingCounts: PendingTaskCounts;
};

const EMPTY_SNAPSHOT: TaskProgressSnapshot = {
  activeTasks: [],
  connectionState: "disconnected",
  pendingCounts: {},
};

const sortTasks = (tasks: ActiveTask[]): ActiveTask[] =>
  [...tasks].sort((a, b) => a.taskType.localeCompare(b.taskType));

const is401 = (error: unknown): boolean =>
  error instanceof Error && error.message.includes("401");

/** Convert a `GET /api/v1/tasks` row into the frontend ActiveTask shape.
 * Titles come from the polling snapshot; SSE events do not carry them. */
function convertTaskToEvent(task: {
  id: string;
  taskType: string;
  status: string;
  libraryId?: string | null;
  seriesId?: string | null;
  bookId?: string | null;
  startedAt?: string | null;
  bookTitle?: string | null;
  seriesTitle?: string | null;
  libraryName?: string | null;
}): ActiveTask {
  // Map "processing" status to "running" for UI consistency.
  const status: TaskStatus =
    task.status === "processing" ? "running" : (task.status as TaskStatus);

  return {
    taskId: task.id,
    taskType: task.taskType,
    status,
    progress: undefined,
    error: undefined,
    startedAt: task.startedAt ?? new Date().toISOString(),
    completedAt: undefined,
    libraryId: task.libraryId ?? undefined,
    seriesId: task.seriesId ?? undefined,
    bookId: task.bookId ?? undefined,
    bookTitle: task.bookTitle ?? undefined,
    seriesTitle: task.seriesTitle ?? undefined,
    libraryName: task.libraryName ?? undefined,
  };
}

/**
 * Single shared source for task-progress state.
 *
 * `useTaskProgress` is mounted in many places at once (the global progress
 * indicator + notification badge, the library page, release hooks, several
 * settings pages, …). If every instance ran its own poll + SSE handler, a busy
 * server produced N copies of the same `GET /tasks?status=processing` +
 * `/tasks/stats` requests every interval — a self-inflicted request storm
 * visible in the network panel. This manager owns exactly one poll loop and one
 * SSE subscription, reference-counted across all hook subscribers (mirroring the
 * SSE manager in `@/api/tasks`), and broadcasts an immutable snapshot via
 * `useSyncExternalStore`. No matter how many components read task progress, the
 * server sees one poller.
 */
class TaskProgressManager {
  private tasks = new Map<string, ActiveTask>();
  private pendingCounts: PendingTaskCounts = {};
  private connectionState: ConnectionState = "disconnected";
  private readonly listeners = new Set<() => void>();
  private pollTimer: ReturnType<typeof setTimeout> | null = null;
  /** Current cadence of the armed poll timer, so re-evaluation can skip
   * re-arming when the cadence hasn't changed (avoids timer thrash while a
   * stream of SSE progress events flows). `null` means no timer is armed. */
  private pollMode: "active" | "idle" | null = null;
  private sseUnsubscribe: (() => void) | null = null;
  /** Per-task deletion timers for terminal tasks (the 5s linger). Tracked so
   * teardown can clear them and avoid firing after the last unsubscribe. */
  private readonly lingerTimers = new Map<
    string,
    ReturnType<typeof setTimeout>
  >();
  /** Cached immutable snapshot. `useSyncExternalStore` requires a stable
   * reference between changes, so this is only rebuilt inside `commit`. */
  private snapshot: TaskProgressSnapshot = EMPTY_SNAPSHOT;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    if (this.listeners.size === 1) {
      this.start();
    }
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size === 0) {
        this.stop();
      }
    };
  };

  getSnapshot = (): TaskProgressSnapshot => this.snapshot;

  /** Rebuild the cached snapshot from current state and notify subscribers.
   * Also re-evaluates the poll cadence: any state change (poll result, SSE
   * event, terminal-task removal) can flip the active/idle decision. */
  private commit() {
    this.snapshot = {
      activeTasks: sortTasks(Array.from(this.tasks.values())),
      connectionState: this.connectionState,
      pendingCounts: this.pendingCounts,
    };
    for (const listener of this.listeners) {
      listener();
    }
    this.scheduleNextPoll();
  }

  /** Is there work worth polling for? Running/pending tasks need progress
   * reconciliation; a non-zero pending count means work is about to start.
   * When neither holds, there is nothing for the backstop poll to catch, so it
   * drops to the idle cadence (SSE still announces new work instantly). */
  private hasActiveWork(): boolean {
    for (const task of this.tasks.values()) {
      if (task.status === "running" || task.status === "pending") {
        return true;
      }
    }
    for (const count of Object.values(this.pendingCounts)) {
      if (count > 0) {
        return true;
      }
    }
    return false;
  }

  /** (Re)arm the single backstop poll timer at the cadence matching current
   * activity. No-op when the correct cadence is already armed, so a burst of
   * SSE events doesn't keep resetting (and starving) the timer. */
  private scheduleNextPoll() {
    if (this.listeners.size === 0) {
      return; // stopped — don't arm a timer with no subscribers
    }
    const desired: "active" | "idle" = this.hasActiveWork() ? "active" : "idle";
    if (this.pollTimer !== null && this.pollMode === desired) {
      return;
    }
    if (this.pollTimer !== null) {
      clearTimeout(this.pollTimer);
    }
    this.pollMode = desired;
    this.pollTimer = setTimeout(
      () => {
        this.pollTimer = null;
        this.pollMode = null;
        void this.poll();
      },
      desired === "active" ? POLL_ACTIVE_MS : POLL_IDLE_MS,
    );
  }

  /** One backstop poll cycle. Each refresh commits, which re-arms the next
   * poll; the trailing schedule guarantees the loop survives even if both
   * fetches error (and thus skip their commit). */
  private poll = async () => {
    await this.refreshPendingCounts();
    await this.refreshProcessingTasks("preserve");
    this.scheduleNextPoll();
  };

  private start() {
    // Prime immediately; the commit from each refresh arms the backstop poll at
    // the right cadence. Arm an idle timer up front too, so the loop runs even
    // if the initial fetches never resolve.
    void this.refreshPendingCounts();
    void this.refreshProcessingTasks("replace");
    this.sseUnsubscribe = subscribeToTaskProgress(
      this.handleEvent,
      this.handleError,
      this.handleConnectionStateChange,
    );
    this.scheduleNextPoll();
  }

  private stop() {
    if (this.pollTimer) {
      clearTimeout(this.pollTimer);
      this.pollTimer = null;
    }
    this.pollMode = null;
    this.sseUnsubscribe?.();
    this.sseUnsubscribe = null;
    for (const timer of this.lingerTimers.values()) {
      clearTimeout(timer);
    }
    this.lingerTimers.clear();
    this.tasks.clear();
    this.pendingCounts = {};
    this.connectionState = "disconnected";
    // No subscribers remain — reset the cached snapshot so the next subscriber
    // starts clean without an extra notify.
    this.snapshot = EMPTY_SNAPSHOT;
  }

  private refreshPendingCounts = async () => {
    try {
      this.pendingCounts = await fetchPendingTaskCounts();
      this.commit();
    } catch (error) {
      if (!is401(error)) {
        console.error("Failed to fetch pending task counts:", error);
      }
      // Keep the backstop loop alive even when a poll fails (no commit ran).
      this.scheduleNextPoll();
    }
  };

  /**
   * Poll the processing list and reconcile it into `tasks`.
   *
   * - `"replace"` (initial load): trust the snapshot fully, overwriting rows.
   * - `"preserve"` (subsequent polls): SSE is authoritative for live progress,
   *   so only fill in tasks the stream hasn't already enriched (new tasks, or
   *   stale `running` rows with no progress yet). Either way, drop `running`
   *   tasks that have fallen out of the processing list; keep terminal
   *   (completed/failed) tasks so their linger timer removes them.
   */
  private refreshProcessingTasks = async (mode: "replace" | "preserve") => {
    try {
      const tasks = await fetchTasksByStatus("processing", 100);
      const currentIds = new Set(tasks.map((task) => task.id));

      for (const [taskId, task] of this.tasks) {
        if (task.status === "running" && !currentIds.has(taskId)) {
          this.tasks.delete(taskId);
        }
      }

      for (const task of tasks) {
        const event = convertTaskToEvent(task);
        if (mode === "replace") {
          this.tasks.set(event.taskId, event);
          continue;
        }
        const existing = this.tasks.get(event.taskId);
        if (
          !existing ||
          (existing.status === "running" &&
            !existing.progress &&
            !existing.completedAt)
        ) {
          this.tasks.set(event.taskId, event);
        }
      }

      this.commit();
    } catch (error) {
      if (!is401(error)) {
        console.error("Failed to fetch processing tasks:", error);
      }
    }
  };

  private handleEvent = (event: TaskProgressEvent) => {
    if (event.status === "completed" || event.status === "failed") {
      const pending = this.lingerTimers.get(event.taskId);
      if (pending) {
        clearTimeout(pending);
      }
      this.lingerTimers.set(
        event.taskId,
        setTimeout(() => {
          this.tasks.delete(event.taskId);
          this.lingerTimers.delete(event.taskId);
          this.commit();
        }, COMPLETED_TASK_LINGER_MS),
      );
    }

    // SSE events do not carry resolved target titles. Preserve any titles
    // stashed from the most recent polling snapshot so the UI keeps showing the
    // human-readable label across progress updates.
    const existing = this.tasks.get(event.taskId);
    this.tasks.set(event.taskId, {
      ...event,
      bookTitle: existing?.bookTitle,
      seriesTitle: existing?.seriesTitle,
      libraryName: existing?.libraryName,
    });
    this.commit();
  };

  private handleError = (error: Error) => {
    console.error("Task progress subscription error:", error);
  };

  private handleConnectionStateChange = (state: ConnectionState) => {
    this.connectionState = state;
    this.commit();
  };
}

const taskProgressManager = new TaskProgressManager();

/**
 * Hook to read task-progress state (active tasks, pending counts, connection
 * state). All instances share one poll loop and one SSE subscription via
 * {@link TaskProgressManager}, so mounting this hook in many components costs no
 * extra network traffic.
 *
 * Permission-aware: a user without `TASKS_READ` (or while unauthenticated) never
 * subscribes, so the shared poller/SSE never starts for them.
 */
export function useTaskProgress() {
  const { isAuthenticated } = useAuthStore();
  const { hasPermission } = usePermissions();
  const enabled = isAuthenticated && hasPermission(PERMISSIONS.TASKS_READ);

  const subscribe = useCallback(
    (listener: () => void) =>
      enabled ? taskProgressManager.subscribe(listener) : () => {},
    [enabled],
  );
  const getSnapshot = useCallback(
    () => (enabled ? taskProgressManager.getSnapshot() : EMPTY_SNAPSHOT),
    [enabled],
  );

  const { activeTasks, connectionState, pendingCounts } = useSyncExternalStore(
    subscribe,
    getSnapshot,
    getSnapshot,
  );

  return {
    /** Active tasks, sorted by task_type for consistent UI ordering. */
    activeTasks,
    /** Current SSE connection state. */
    connectionState,
    /** Pending task counts by type. */
    pendingCounts,
    /** All tasks with a specific status (already sorted by task_type). */
    getTasksByStatus: (status: TaskStatus): ActiveTask[] =>
      activeTasks.filter((task) => task.status === status),
    /** All tasks for a specific library (already sorted by task_type). */
    getTasksByLibrary: (libraryId: string): ActiveTask[] =>
      activeTasks.filter((task) => task.libraryId === libraryId),
    /** A specific task by ID. */
    getTask: (taskId: string): ActiveTask | undefined =>
      activeTasks.find((task) => task.taskId === taskId),
  };
}
