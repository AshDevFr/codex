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
