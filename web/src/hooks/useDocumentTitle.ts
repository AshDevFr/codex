import { useEffect } from "react";
import { useAppName } from "./useAppName";

/**
 * Hook to set the document title with the app name.
 *
 * Sets the document title to either:
 * - `{pageTitle} - {appName}` when a page title is provided
 * - `{appName}` when no page title is provided
 *
 * The title is automatically updated when the app name changes.
 *
 * @param pageTitle - Optional page-specific title (e.g., "Libraries", "Settings")
 *
 * @example
 * ```tsx
 * function LibrariesPage() {
 *   useDocumentTitle("Libraries");
 *   // Sets title to "Libraries - Codex" (or custom app name)
 *   return <div>...</div>;
 * }
 * ```
 *
 * @example
 * ```tsx
 * function HomePage() {
 *   useDocumentTitle();
 *   // Sets title to "Codex" (or custom app name)
 *   return <div>...</div>;
 * }
 * ```
 */
export function useDocumentTitle(pageTitle?: string): void {
	const appName = useAppName();

	useEffect(() => {
		const title = pageTitle ? `${pageTitle} - ${appName}` : appName;
		document.title = title;

		// Cleanup: We don't reset the title on unmount since the next page
		// will set its own title. This avoids a flash of the old title.
	}, [pageTitle, appName]);
}

/**
 * Hook to set a dynamic document title with the app name.
 *
 * Use this when the page title depends on data that may change or be undefined.
 *
 * @param pageTitle - Dynamic page title (can be undefined while loading)
 * @param fallbackTitle - Title to use while pageTitle is undefined
 *
 * @example
 * ```tsx
 * function BookDetailPage({ bookId }) {
 *   const { data: book, isLoading } = useBook(bookId);
 *   useDynamicDocumentTitle(book?.title, "Loading...");
 *   // Shows "Loading... - Codex" then "Book Title - Codex"
 *   return <div>...</div>;
 * }
 * ```
 */
export function useDynamicDocumentTitle(
	pageTitle: string | undefined,
	fallbackTitle?: string,
): void {
	const title = pageTitle ?? fallbackTitle;
	useDocumentTitle(title);
}
