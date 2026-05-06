/**
 * Format an elapsed duration in milliseconds as a compact human-readable
 * string for tooltip-style displays.
 *
 * Output examples:
 *   400  -> "0s"
 *   12_000 -> "12s"
 *   84_000 -> "1m 24s"
 *   7_500_000 -> "2h 5m"
 *
 * Hours suppress the seconds component to keep the string short. Negative
 * inputs are clamped to 0.
 */
export function formatElapsed(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) {
    return "0s";
  }
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

/**
 * Compute elapsed milliseconds between an ISO timestamp and `now`.
 * Returns 0 if `startedAt` is missing or unparseable.
 */
export function elapsedSince(
  startedAt: string | null | undefined,
  now: number = Date.now(),
): number {
  if (!startedAt) return 0;
  const started = Date.parse(startedAt);
  if (Number.isNaN(started)) return 0;
  return Math.max(0, now - started);
}
