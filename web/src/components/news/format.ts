/** Short, locale-aware date for a news entry, e.g. "Oct 16, 2026". */
export function formatNewsDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}
