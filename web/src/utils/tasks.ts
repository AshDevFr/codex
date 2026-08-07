import type { ActiveTask } from "@/types";

/**
 * Resolve the most-specific human-readable label for a task's target.
 *
 * Precedence: book title -> series title -> library name. Returns `null` when
 * none of the three are populated (e.g. library-wide cleanup tasks with no
 * scoped target).
 */
export function getTaskTarget(task: ActiveTask): string | null {
  return task.bookTitle ?? task.seriesTitle ?? task.libraryName ?? null;
}

/** Shown when a task has neither a live message nor a resolved target. */
const TASK_LABEL_FALLBACK = "Processing...";

/**
 * Resolve the single line of text describing what a task is doing right now.
 *
 * The two sources are complementary, and neither is sufficient alone: SSE
 * progress events carry a live message but only raw target IDs, while the
 * `GET /api/v1/tasks` snapshot carries resolved titles but no progress at all.
 * A live message is the most specific thing we have, so it wins; the resolved
 * target is the floor, which matters for handlers that emit a single progress
 * event at start (a page opened mid-task never receives it).
 */
export function getTaskLabel(task: ActiveTask): string {
  const message = task.progress?.message?.trim();
  return message || getTaskTarget(task) || TASK_LABEL_FALLBACK;
}
