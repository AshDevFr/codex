import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useTouchNav } from "./useTouchNav";
import { useReaderStore } from "@/store/readerStore";

describe("useTouchNav", () => {
	let element: HTMLDivElement;
	let mockNextPage: ReturnType<typeof vi.fn>;
	let mockPrevPage: ReturnType<typeof vi.fn>;
	let mockTap: ReturnType<typeof vi.fn>;

	beforeEach(() => {
		// Reset store state
		useReaderStore.setState({
			settings: {
				...useReaderStore.getState().settings,
				readingDirection: "ltr",
			},
			readingDirectionOverride: null,
		});

		// Create a mock element
		element = document.createElement("div");
		document.body.appendChild(element);

		// Create mocks
		mockNextPage = vi.fn();
		mockPrevPage = vi.fn();
		mockTap = vi.fn();
	});

	afterEach(() => {
		document.body.removeChild(element);
	});

	// Helper to create touch events
	const createTouchEvent = (
		type: "touchstart" | "touchend" | "touchcancel",
		x: number,
		y: number,
	): TouchEvent => {
		const touch = {
			clientX: x,
			clientY: y,
			identifier: 0,
			target: element,
			screenX: x,
			screenY: y,
			pageX: x,
			pageY: y,
			radiusX: 0,
			radiusY: 0,
			rotationAngle: 0,
			force: 0,
		} as Touch;

		return new TouchEvent(type, {
			touches: type === "touchend" || type === "touchcancel" ? [] : [touch],
			changedTouches: [touch],
			bubbles: true,
		});
	};

	// Helper to simulate swipe
	const simulateSwipe = async (
		startX: number,
		startY: number,
		endX: number,
		endY: number,
	) => {
		await act(async () => {
			element.dispatchEvent(createTouchEvent("touchstart", startX, startY));
		});
		await act(async () => {
			element.dispatchEvent(createTouchEvent("touchend", endX, endY));
		});
	};

	describe("LTR mode", () => {
		it("should call onNextPage when swiping left", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe left

			expect(mockNextPage).toHaveBeenCalledTimes(1);
			expect(mockPrevPage).not.toHaveBeenCalled();
		});

		it("should call onPrevPage when swiping right", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 100, 200, 100); // Swipe right

			expect(mockPrevPage).toHaveBeenCalledTimes(1);
			expect(mockNextPage).not.toHaveBeenCalled();
		});

		it("should not trigger navigation for small swipes", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 100, 120, 100); // Small swipe (20px)

			expect(mockNextPage).not.toHaveBeenCalled();
			expect(mockPrevPage).not.toHaveBeenCalled();
		});
	});

	describe("RTL mode", () => {
		beforeEach(() => {
			useReaderStore.setState({
				readingDirectionOverride: "rtl",
			});
		});

		it("should call onPrevPage when swiping left (reversed)", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe left

			expect(mockPrevPage).toHaveBeenCalledTimes(1);
			expect(mockNextPage).not.toHaveBeenCalled();
		});

		it("should call onNextPage when swiping right (reversed)", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 100, 200, 100); // Swipe right

			expect(mockNextPage).toHaveBeenCalledTimes(1);
			expect(mockPrevPage).not.toHaveBeenCalled();
		});
	});

	describe("TTB mode", () => {
		beforeEach(() => {
			useReaderStore.setState({
				readingDirectionOverride: "ttb",
			});
		});

		it("should call onNextPage when swiping up", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 200, 100, 100); // Swipe up

			expect(mockNextPage).toHaveBeenCalledTimes(1);
			expect(mockPrevPage).not.toHaveBeenCalled();
		});

		it("should call onPrevPage when swiping down", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 100, 100, 200); // Swipe down

			expect(mockPrevPage).toHaveBeenCalledTimes(1);
			expect(mockNextPage).not.toHaveBeenCalled();
		});

		it("should ignore horizontal swipes in TTB mode", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe left (horizontal)

			expect(mockNextPage).not.toHaveBeenCalled();
			expect(mockPrevPage).not.toHaveBeenCalled();
		});
	});

	describe("webtoon mode", () => {
		beforeEach(() => {
			useReaderStore.setState({
				readingDirectionOverride: "webtoon",
			});
		});

		it("should use vertical navigation like TTB", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 200, 100, 100); // Swipe up

			expect(mockNextPage).toHaveBeenCalledTimes(1);
		});
	});

	describe("tap detection", () => {
		it("should call onTap for minimal movement", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					onTap: mockTap,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(100, 100, 102, 102); // Minimal movement (tap)

			expect(mockTap).toHaveBeenCalledTimes(1);
			expect(mockNextPage).not.toHaveBeenCalled();
			expect(mockPrevPage).not.toHaveBeenCalled();
		});

		it("should not call onTap for swipes", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
					onTap: mockTap,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe

			expect(mockTap).not.toHaveBeenCalled();
		});
	});

	describe("disabled state", () => {
		it("should not respond when disabled", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: false,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe left

			expect(mockNextPage).not.toHaveBeenCalled();
			expect(mockPrevPage).not.toHaveBeenCalled();
		});
	});

	describe("touch cancel", () => {
		it("should handle touch cancel gracefully", async () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
					onPrevPage: mockPrevPage,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await act(async () => {
				element.dispatchEvent(createTouchEvent("touchstart", 200, 100));
			});
			await act(async () => {
				element.dispatchEvent(createTouchEvent("touchcancel", 150, 100));
			});
			await act(async () => {
				element.dispatchEvent(createTouchEvent("touchend", 100, 100));
			});

			// Should not trigger after cancel
			expect(mockNextPage).not.toHaveBeenCalled();
			expect(mockPrevPage).not.toHaveBeenCalled();
		});
	});

	describe("ref management", () => {
		it("should clean up listeners when ref changes", () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
				}),
			);

			const element2 = document.createElement("div");
			document.body.appendChild(element2);

			act(() => {
				result.current.touchRef(element);
			});

			act(() => {
				result.current.touchRef(element2);
			});

			// Old element should no longer have listeners
			// (Testing this indirectly by checking new element works)
			act(() => {
				element.dispatchEvent(createTouchEvent("touchstart", 200, 100));
				element.dispatchEvent(createTouchEvent("touchend", 100, 100));
			});

			expect(mockNextPage).not.toHaveBeenCalled();

			document.body.removeChild(element2);
		});

		it("should handle null ref", () => {
			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					onNextPage: mockNextPage,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			act(() => {
				result.current.touchRef(null);
			});

			// Should not throw
			expect(() => {
				act(() => {
					element.dispatchEvent(createTouchEvent("touchstart", 200, 100));
				});
			}).not.toThrow();
		});
	});

	describe("uses store actions when no custom handlers", () => {
		it("should use store nextPage when no onNextPage provided", async () => {
			const storeNextPage = vi.spyOn(
				useReaderStore.getState(),
				"nextPage",
			);

			useReaderStore.setState({
				currentPage: 1,
				totalPages: 10,
			});

			const { result } = renderHook(() =>
				useTouchNav({
					enabled: true,
					minSwipeDistance: 50,
				}),
			);

			act(() => {
				result.current.touchRef(element);
			});

			await simulateSwipe(200, 100, 100, 100); // Swipe left

			expect(storeNextPage).toHaveBeenCalled();
		});
	});
});
