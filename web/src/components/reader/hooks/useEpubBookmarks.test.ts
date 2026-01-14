import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useEpubBookmarks } from "./useEpubBookmarks";

describe("useEpubBookmarks", () => {
	const bookId = "test-book-123";
	const storageKey = `epub-bookmarks-${bookId}`;
	const originalConsoleWarn = console.warn;

	beforeEach(() => {
		localStorage.clear();
		vi.clearAllMocks();
		console.warn = vi.fn();
	});

	afterEach(() => {
		localStorage.clear();
		console.warn = originalConsoleWarn;
	});

	describe("initialization", () => {
		it("should initialize with empty bookmarks when no saved data", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			expect(result.current.bookmarks).toEqual([]);
		});

		it("should load bookmarks from localStorage on init", () => {
			const savedBookmarks = [
				{
					id: "bookmark-1",
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "Test note",
					chapterTitle: "Chapter 1",
					createdAt: Date.now(),
				},
			];
			localStorage.setItem(storageKey, JSON.stringify(savedBookmarks));

			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			expect(result.current.bookmarks).toHaveLength(1);
			expect(result.current.bookmarks[0].note).toBe("Test note");
		});

		it("should handle invalid localStorage data gracefully", () => {
			localStorage.setItem(storageKey, "invalid-json");

			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			expect(result.current.bookmarks).toEqual([]);
		});

		it("should return empty bookmarks when disabled", () => {
			const savedBookmarks = [
				{
					id: "bookmark-1",
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
					createdAt: Date.now(),
				},
			];
			localStorage.setItem(storageKey, JSON.stringify(savedBookmarks));

			const { result } = renderHook(() =>
				useEpubBookmarks({ bookId, enabled: false }),
			);

			expect(result.current.bookmarks).toEqual([]);
		});
	});

	describe("addBookmark", () => {
		it("should add a new bookmark", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
					chapterTitle: "Chapter 1",
				});
			});

			expect(result.current.bookmarks).toHaveLength(1);
			expect(result.current.bookmarks[0].cfi).toBe("epubcfi(/6/4!/4/2/1:0)");
			expect(result.current.bookmarks[0].percentage).toBe(0.25);
			expect(result.current.bookmarks[0].chapterTitle).toBe("Chapter 1");
		});

		it("should generate unique IDs for bookmarks", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/2:0)",
					percentage: 0.5,
					note: "",
				});
			});

			expect(result.current.bookmarks[0].id).not.toBe(
				result.current.bookmarks[1].id,
			);
		});

		it("should set createdAt timestamp", () => {
			const before = Date.now();
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			const after = Date.now();
			expect(result.current.bookmarks[0].createdAt).toBeGreaterThanOrEqual(
				before,
			);
			expect(result.current.bookmarks[0].createdAt).toBeLessThanOrEqual(after);
		});

		it("should return the created bookmark", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			let createdBookmark!: ReturnType<typeof result.current.addBookmark>;
			act(() => {
				createdBookmark = result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "Test",
				});
			});

			expect(createdBookmark?.id).toBeDefined();
			expect(createdBookmark?.cfi).toBe("epubcfi(/6/4!/4/2/1:0)");
		});

		it("should persist to localStorage", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			const stored = JSON.parse(localStorage.getItem(storageKey) || "[]");
			expect(stored).toHaveLength(1);
		});
	});

	describe("updateBookmark", () => {
		it("should update bookmark note", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			let bookmark: ReturnType<typeof result.current.addBookmark>;
			act(() => {
				bookmark = result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			act(() => {
				result.current.updateBookmark(bookmark?.id, { note: "Updated note" });
			});

			expect(result.current.bookmarks[0].note).toBe("Updated note");
		});

		it("should not modify other bookmarks", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			let bookmark1: ReturnType<typeof result.current.addBookmark>;
			act(() => {
				bookmark1 = result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "Note 1",
				});
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/2:0)",
					percentage: 0.5,
					note: "Note 2",
				});
			});

			act(() => {
				result.current.updateBookmark(bookmark1?.id, { note: "Updated" });
			});

			expect(result.current.bookmarks[0].note).toBe("Updated");
			expect(result.current.bookmarks[1].note).toBe("Note 2");
		});
	});

	describe("removeBookmark", () => {
		it("should remove a bookmark by id", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			let bookmark: ReturnType<typeof result.current.addBookmark>;
			act(() => {
				bookmark = result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			act(() => {
				result.current.removeBookmark(bookmark?.id);
			});

			expect(result.current.bookmarks).toHaveLength(0);
		});

		it("should only remove the specified bookmark", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			let bookmark1: ReturnType<typeof result.current.addBookmark>;
			act(() => {
				bookmark1 = result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/2:0)",
					percentage: 0.5,
					note: "",
				});
			});

			act(() => {
				result.current.removeBookmark(bookmark1?.id);
			});

			expect(result.current.bookmarks).toHaveLength(1);
			expect(result.current.bookmarks[0].percentage).toBe(0.5);
		});
	});

	describe("isBookmarked", () => {
		it("should return true for bookmarked CFI", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));
			const cfi = "epubcfi(/6/4!/4/2/1:0)";

			act(() => {
				result.current.addBookmark({
					cfi,
					percentage: 0.25,
					note: "",
				});
			});

			expect(result.current.isBookmarked(cfi)).toBe(true);
		});

		it("should return false for non-bookmarked CFI", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			expect(result.current.isBookmarked("epubcfi(/6/4!/4/2/2:0)")).toBe(false);
		});
	});

	describe("getBookmarkByCfi", () => {
		it("should return bookmark for given CFI", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));
			const cfi = "epubcfi(/6/4!/4/2/1:0)";

			act(() => {
				result.current.addBookmark({
					cfi,
					percentage: 0.25,
					note: "Test note",
				});
			});

			const bookmark = result.current.getBookmarkByCfi(cfi);
			expect(bookmark).toBeDefined();
			expect(bookmark?.note).toBe("Test note");
		});

		it("should return undefined for non-existent CFI", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			const bookmark = result.current.getBookmarkByCfi(
				"epubcfi(/6/4!/4/2/1:0)",
			);
			expect(bookmark).toBeUndefined();
		});
	});

	describe("clearBookmarks", () => {
		it("should remove all bookmarks", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/2:0)",
					percentage: 0.5,
					note: "",
				});
			});

			act(() => {
				result.current.clearBookmarks();
			});

			expect(result.current.bookmarks).toHaveLength(0);
		});

		it("should clear localStorage or set to empty array", () => {
			const { result } = renderHook(() => useEpubBookmarks({ bookId }));

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "",
				});
			});

			act(() => {
				result.current.clearBookmarks();
			});

			// After clearing, localStorage either has null or empty array (due to persistence effect)
			const stored = localStorage.getItem(storageKey);
			if (stored !== null) {
				expect(JSON.parse(stored)).toEqual([]);
			}
		});
	});

	describe("persistence", () => {
		it("should persist bookmarks across hook remounts", () => {
			const { result, unmount } = renderHook(() =>
				useEpubBookmarks({ bookId }),
			);

			act(() => {
				result.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "Persistent note",
				});
			});

			unmount();

			const { result: newResult } = renderHook(() =>
				useEpubBookmarks({ bookId }),
			);

			expect(newResult.current.bookmarks).toHaveLength(1);
			expect(newResult.current.bookmarks[0].note).toBe("Persistent note");
		});

		it("should use separate storage for different books", () => {
			const { result: result1 } = renderHook(() =>
				useEpubBookmarks({ bookId: "book-1" }),
			);
			const { result: result2 } = renderHook(() =>
				useEpubBookmarks({ bookId: "book-2" }),
			);

			act(() => {
				result1.current.addBookmark({
					cfi: "epubcfi(/6/4!/4/2/1:0)",
					percentage: 0.25,
					note: "Book 1 bookmark",
				});
			});

			expect(result1.current.bookmarks).toHaveLength(1);
			expect(result2.current.bookmarks).toHaveLength(0);
		});
	});
});
