/**
 * Build the API URL for a book's cover thumbnail.
 *
 * Mirrors the convention used across the app (e.g. MediaCard, BookDetail):
 * `/api/v1/books/{id}/thumbnail`. Pair with `useAuthenticatedImage` when the
 * image must be fetched through the authenticated API client.
 */
export function bookCoverUrl(bookId: string): string {
  return `/api/v1/books/${bookId}/thumbnail`;
}
