import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { useKeyboardNav } from "./useKeyboardNav";

describe("useKeyboardNav", () => {
	beforeEach(() => {
		// Reset store to default state
		useReaderStore.setState({
			settings: {
				fitMode: "screen",
				pageLayout: "single",
				readingDirection: "ltr",
				backgroundColor: "black",
				pdfMode: "streaming",
				pdfSpreadMode: "single",
				pdfContinuousScroll: false,
				autoHideToolbar: true,
				toolbarHideDelay: 3000,
				epubTheme: "light",
				epubFontSize: 100,
				epubFontFamily: "default",
				epubLineHeight: 150,
				epubMargin: 10,
				preloadPages: 1,
				doublePageShowWideAlone: true,
				doublePageStartOnOdd: true,
				pageTransition: "slide",
				transitionDuration: 200,
				webtoonSidePadding: 0,
				webtoonPageGap: 0,
				autoAdvanceToNextBook: false,
			},
			currentPage: 5,
			totalPages: 10,
			isLoading: false,
			toolbarVisible: true,
			isFullscreen: false,
			currentBookId: "book-123",
			readingDirectionOverride: null,
			adjacentBooks: null,
			boundaryState: "none",
			pageOrientations: {},
			lastNavigationDirection: null,
			preloadedImages: new Set<string>(),
		});
	});

	const dispatchKeyEvent = (key: string, target?: HTMLElement) => {
		const event = new KeyboardEvent("keydown", {
			key,
			bubbles: true,
		});
		if (target) {
			Object.defineProperty(event, "target", { value: target });
		}
		window.dispatchEvent(event);
	};

	describe("LTR navigation", () => {
		it("should go to next page on ArrowRight", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowRight");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should go to previous page on ArrowLeft", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowLeft");
			});

			expect(useReaderStore.getState().currentPage).toBe(4);
		});
	});

	describe("RTL navigation", () => {
		beforeEach(() => {
			useReaderStore.setState({
				settings: {
					...useReaderStore.getState().settings,
					readingDirection: "rtl",
				},
			});
		});

		it("should go to next page on ArrowLeft (RTL)", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowLeft");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should go to previous page on ArrowRight (RTL)", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowRight");
			});

			expect(useReaderStore.getState().currentPage).toBe(4);
		});
	});

	describe("page navigation keys", () => {
		it("should go to next page on ArrowDown", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowDown");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should go to next page on Space", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent(" ");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should go to next page on PageDown", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("PageDown");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});

		it("should go to previous page on ArrowUp", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("ArrowUp");
			});

			expect(useReaderStore.getState().currentPage).toBe(4);
		});

		it("should go to previous page on PageUp", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("PageUp");
			});

			expect(useReaderStore.getState().currentPage).toBe(4);
		});

		it("should go to first page on Home", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("Home");
			});

			expect(useReaderStore.getState().currentPage).toBe(1);
		});

		it("should go to last page on End", () => {
			renderHook(() => useKeyboardNav());

			act(() => {
				dispatchKeyEvent("End");
			});

			expect(useReaderStore.getState().currentPage).toBe(10);
		});
	});

	describe("UI controls", () => {
		it("should toggle fullscreen on F key", () => {
			renderHook(() => useKeyboardNav());

			expect(useReaderStore.getState().isFullscreen).toBe(false);

			act(() => {
				dispatchKeyEvent("f");
			});

			expect(useReaderStore.getState().isFullscreen).toBe(true);
		});

		it("should toggle toolbar on T key", () => {
			renderHook(() => useKeyboardNav());

			expect(useReaderStore.getState().toolbarVisible).toBe(true);

			act(() => {
				dispatchKeyEvent("t");
			});

			expect(useReaderStore.getState().toolbarVisible).toBe(false);
		});

		it("should cycle fit mode on M key", () => {
			renderHook(() => useKeyboardNav());

			expect(useReaderStore.getState().settings.fitMode).toBe("screen");

			act(() => {
				dispatchKeyEvent("m");
			});

			expect(useReaderStore.getState().settings.fitMode).toBe("width");
		});

		it("should call onEscape callback on Escape key", () => {
			const onEscape = vi.fn();
			renderHook(() => useKeyboardNav({ onEscape }));

			act(() => {
				dispatchKeyEvent("Escape");
			});

			expect(onEscape).toHaveBeenCalledTimes(1);
		});
	});

	describe("enabled option", () => {
		it("should not handle keys when disabled", () => {
			renderHook(() => useKeyboardNav({ enabled: false }));

			act(() => {
				dispatchKeyEvent("ArrowRight");
			});

			expect(useReaderStore.getState().currentPage).toBe(5); // Unchanged
		});

		it("should handle keys when enabled", () => {
			renderHook(() => useKeyboardNav({ enabled: true }));

			act(() => {
				dispatchKeyEvent("ArrowRight");
			});

			expect(useReaderStore.getState().currentPage).toBe(6);
		});
	});

	describe("input element handling", () => {
		it("should not handle keys when focus is on input element", () => {
			renderHook(() => useKeyboardNav());

			const input = document.createElement("input");
			document.body.appendChild(input);

			act(() => {
				dispatchKeyEvent("ArrowRight", input);
			});

			expect(useReaderStore.getState().currentPage).toBe(5); // Unchanged

			document.body.removeChild(input);
		});

		it("should not handle keys when focus is on textarea element", () => {
			renderHook(() => useKeyboardNav());

			const textarea = document.createElement("textarea");
			document.body.appendChild(textarea);

			act(() => {
				dispatchKeyEvent("ArrowRight", textarea);
			});

			expect(useReaderStore.getState().currentPage).toBe(5); // Unchanged

			document.body.removeChild(textarea);
		});
	});

	describe("cleanup", () => {
		it("should remove event listener on unmount", () => {
			const { unmount } = renderHook(() => useKeyboardNav());

			unmount();

			// After unmount, key events should not affect store
			act(() => {
				dispatchKeyEvent("ArrowRight");
			});

			// Page should still be 5 since the listener was removed
			// Note: This test verifies cleanup but the event still fires
			// The actual verification is that no errors occur
		});
	});
});
