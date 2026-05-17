import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderStore } from "@/store/readerStore";
import { useTouchNav } from "./useTouchNav";

describe("useTouchNav", () => {
  let element: HTMLDivElement;
  let mockNextPage: ReturnType<typeof vi.fn>;
  let mockPrevPage: ReturnType<typeof vi.fn>;
  let mockTap: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    useReaderStore.setState({
      settings: {
        ...useReaderStore.getState().settings,
        readingDirection: "ltr",
      },
      readingDirectionOverride: null,
    });

    element = document.createElement("div");
    document.body.appendChild(element);

    mockNextPage = vi.fn();
    mockPrevPage = vi.fn();
    mockTap = vi.fn();
  });

  afterEach(() => {
    document.body.removeChild(element);
  });

  type PointerKind = "touch" | "mouse" | "pen";

  interface PointerInit {
    pointerType?: PointerKind;
    pointerId?: number;
    isPrimary?: boolean;
    button?: number;
    timeStamp?: number;
  }

  // jsdom doesn't ship a PointerEvent constructor; build one from MouseEvent
  // and add the pointer fields the hook reads. We assign timeStamp explicitly
  // so each test can control gesture duration deterministically.
  const createPointerEvent = (
    type: "pointerdown" | "pointerup" | "pointercancel",
    x: number,
    y: number,
    init: PointerInit = {},
  ): PointerEvent => {
    const {
      pointerType = "touch",
      pointerId = 1,
      isPrimary = true,
      button = 0,
      timeStamp = 0,
    } = init;

    const event = new MouseEvent(type, {
      clientX: x,
      clientY: y,
      button,
      bubbles: true,
      cancelable: true,
    }) as MouseEvent & {
      pointerId: number;
      pointerType: PointerKind;
      isPrimary: boolean;
    };

    Object.defineProperty(event, "pointerId", { value: pointerId });
    Object.defineProperty(event, "pointerType", { value: pointerType });
    Object.defineProperty(event, "isPrimary", { value: isPrimary });
    Object.defineProperty(event, "timeStamp", { value: timeStamp });

    return event as unknown as PointerEvent;
  };

  const simulateSwipe = async (
    startX: number,
    startY: number,
    endX: number,
    endY: number,
    init: PointerInit = {},
    duration = 100,
  ) => {
    await act(async () => {
      element.dispatchEvent(
        createPointerEvent("pointerdown", startX, startY, {
          ...init,
          timeStamp: 0,
        }),
      );
    });
    await act(async () => {
      element.dispatchEvent(
        createPointerEvent("pointerup", endX, endY, {
          ...init,
          timeStamp: duration,
        }),
      );
    });
  };

  describe("LTR mode (touch)", () => {
    it("calls onNextPage when swiping left", async () => {
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

      await simulateSwipe(200, 100, 100, 100);

      expect(mockNextPage).toHaveBeenCalledTimes(1);
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("calls onPrevPage when swiping right", async () => {
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

      await simulateSwipe(100, 100, 200, 100);

      expect(mockPrevPage).toHaveBeenCalledTimes(1);
      expect(mockNextPage).not.toHaveBeenCalled();
    });

    it("does not trigger navigation for small swipes", async () => {
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

      await simulateSwipe(100, 100, 120, 100);

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("LTR mode (mouse drag) — R10-4 desktop emulation support", () => {
    it("treats a horizontal mouse drag the same as a touch swipe", async () => {
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

      await simulateSwipe(300, 200, 150, 200, { pointerType: "mouse" });

      expect(mockNextPage).toHaveBeenCalledTimes(1);
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("ignores non-primary mouse buttons (right-click drag)", async () => {
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

      // button = 2 → right-click; should be ignored entirely.
      await simulateSwipe(300, 200, 150, 200, {
        pointerType: "mouse",
        button: 2,
      });

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("treats a mouse click in place as a tap", async () => {
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

      await simulateSwipe(200, 200, 201, 201, { pointerType: "mouse" });

      expect(mockTap).toHaveBeenCalledTimes(1);
    });
  });

  describe("RTL mode", () => {
    beforeEach(() => {
      useReaderStore.setState({
        readingDirectionOverride: "rtl",
      });
    });

    it("calls onPrevPage when swiping left (reversed)", async () => {
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

      await simulateSwipe(200, 100, 100, 100);

      expect(mockPrevPage).toHaveBeenCalledTimes(1);
      expect(mockNextPage).not.toHaveBeenCalled();
    });

    it("calls onNextPage when swiping right (reversed)", async () => {
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

      await simulateSwipe(100, 100, 200, 100);

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

    it("calls onNextPage when swiping up", async () => {
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

      await simulateSwipe(100, 200, 100, 100);

      expect(mockNextPage).toHaveBeenCalledTimes(1);
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("calls onPrevPage when swiping down", async () => {
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

      await simulateSwipe(100, 100, 100, 200);

      expect(mockPrevPage).toHaveBeenCalledTimes(1);
      expect(mockNextPage).not.toHaveBeenCalled();
    });

    it("ignores horizontal swipes in TTB mode", async () => {
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

      await simulateSwipe(200, 100, 100, 100);

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

    it("uses vertical navigation like TTB", async () => {
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

      await simulateSwipe(100, 200, 100, 100);

      expect(mockNextPage).toHaveBeenCalledTimes(1);
    });
  });

  describe("tap detection", () => {
    it("calls onTap for minimal movement", async () => {
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

      await simulateSwipe(100, 100, 102, 102);

      expect(mockTap).toHaveBeenCalledTimes(1);
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("does not call onTap for swipes", async () => {
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

      await simulateSwipe(200, 100, 100, 100);

      expect(mockTap).not.toHaveBeenCalled();
    });
  });

  describe("disabled state", () => {
    it("does not respond when disabled", async () => {
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

      await simulateSwipe(200, 100, 100, 100);

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("pointer cancel", () => {
    it("handles pointer cancel gracefully", async () => {
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
        element.dispatchEvent(createPointerEvent("pointerdown", 200, 100));
      });
      await act(async () => {
        element.dispatchEvent(createPointerEvent("pointercancel", 150, 100));
      });
      await act(async () => {
        element.dispatchEvent(createPointerEvent("pointerup", 100, 100));
      });

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("ref management", () => {
    it("cleans up listeners when ref changes", () => {
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

      // Old element should no longer have listeners; swipe on it must not fire.
      act(() => {
        element.dispatchEvent(createPointerEvent("pointerdown", 200, 100));
        element.dispatchEvent(createPointerEvent("pointerup", 100, 100));
      });

      expect(mockNextPage).not.toHaveBeenCalled();

      document.body.removeChild(element2);
    });

    it("handles null ref without throwing", () => {
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

      expect(() => {
        act(() => {
          element.dispatchEvent(createPointerEvent("pointerdown", 200, 100));
        });
      }).not.toThrow();
    });
  });

  describe("uses store actions when no custom handlers", () => {
    it("uses store nextPage when no onNextPage provided", async () => {
      const storeNextPage = vi.spyOn(useReaderStore.getState(), "nextPage");

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

      await simulateSwipe(200, 100, 100, 100);

      expect(storeNextPage).toHaveBeenCalled();
    });
  });
});
