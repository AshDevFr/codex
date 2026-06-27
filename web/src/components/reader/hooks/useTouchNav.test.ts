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
    vi.useRealTimers();
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
  // and add the pointer fields the hook reads.
  const createPointerEvent = (
    type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
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

  const simulateGesture = async (
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

  const dispatch = async (
    type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
    x: number,
    y: number,
    t: number,
    init: PointerInit = {},
  ) => {
    await act(async () => {
      element.dispatchEvent(
        createPointerEvent(type, x, y, { ...init, timeStamp: t }),
      );
    });
  };

  describe("zoom (pinch/pan) plumbing", () => {
    const makeSwipe = () => ({
      enabled: true,
      onStart: vi.fn(() => true),
      onMove: vi.fn(),
      onEnd: vi.fn(),
      onCancel: vi.fn(),
    });

    const makeZoom = (panActive = false) => ({
      panActive: vi.fn(() => panActive),
      onPan: vi.fn(),
      onPanEnd: vi.fn(),
      onPinch: vi.fn(),
      onPinchEnd: vi.fn(),
      onDoubleTap: vi.fn(),
    });

    const mountZoom = (
      swipe: ReturnType<typeof makeSwipe>,
      zoom: ReturnType<typeof makeZoom>,
    ) => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
          swipe,
          zoom,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });
    };

    const stubRect = (w = 1000, h = 1000) => {
      vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
        left: 0,
        top: 0,
        right: w,
        bottom: h,
        width: w,
        height: h,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
    };

    it("emits pinch with the incremental scale ratio and focal point", async () => {
      stubRect();
      const zoom = makeZoom();
      mountZoom(makeSwipe(), zoom);

      // Two fingers 200px apart, centered horizontally.
      await dispatch("pointerdown", 400, 500, 0, { pointerId: 1 });
      await dispatch("pointerdown", 600, 500, 0, {
        pointerId: 2,
        isPrimary: false,
      });
      // Spread the second finger to 300px separation.
      await dispatch("pointermove", 700, 500, 50, { pointerId: 2 });

      expect(zoom.onPinch).toHaveBeenCalledTimes(1);
      const [ratio, focus] = zoom.onPinch.mock.calls[0];
      expect(ratio).toBeCloseTo(1.5); // 300 / 200
      // Midpoint (550,500) relative to element center (500,500).
      expect(focus.x).toBeCloseTo(50);
      expect(focus.y).toBeCloseTo(0);
    });

    it("pans with incremental deltas when panActive, not swipe", async () => {
      const swipe = makeSwipe();
      const zoom = makeZoom(true);
      mountZoom(swipe, zoom);

      await dispatch("pointerdown", 300, 300, 0, { pointerId: 1 });
      // First move past activation arms pan (no pan emitted yet).
      await dispatch("pointermove", 320, 330, 16, { pointerId: 1 });
      // Subsequent move emits the incremental delta.
      await dispatch("pointermove", 340, 360, 32, { pointerId: 1 });

      expect(zoom.onPan).toHaveBeenCalledTimes(1);
      expect(zoom.onPan).toHaveBeenCalledWith(20, 30);
      expect(swipe.onMove).not.toHaveBeenCalled();

      await dispatch("pointerup", 340, 360, 48, { pointerId: 1 });
      expect(zoom.onPanEnd).toHaveBeenCalledTimes(1);
      expect(swipe.onEnd).not.toHaveBeenCalled();
    });

    it("a second finger aborts an in-flight swipe and starts pinch", async () => {
      stubRect();
      const swipe = makeSwipe();
      const zoom = makeZoom();
      mountZoom(swipe, zoom);

      // Arm a swipe with one finger.
      await dispatch("pointerdown", 200, 300, 0, { pointerId: 1 });
      await dispatch("pointermove", 260, 300, 50, { pointerId: 1 });
      expect(swipe.onMove).toHaveBeenCalled();

      // Second finger lands → swipe aborts, pinch begins.
      await dispatch("pointerdown", 500, 300, 60, {
        pointerId: 2,
        isPrimary: false,
      });
      expect(swipe.onCancel).toHaveBeenCalledTimes(1);

      await dispatch("pointermove", 560, 300, 80, { pointerId: 2 });
      expect(zoom.onPinch).toHaveBeenCalled();
      // The aborted swipe must not also end.
      expect(swipe.onEnd).not.toHaveBeenCalled();
    });

    it("ends the pinch when a finger lifts", async () => {
      stubRect();
      const zoom = makeZoom();
      mountZoom(makeSwipe(), zoom);

      await dispatch("pointerdown", 400, 500, 0, { pointerId: 1 });
      await dispatch("pointerdown", 600, 500, 0, {
        pointerId: 2,
        isPrimary: false,
      });
      await dispatch("pointerup", 600, 500, 50, { pointerId: 2 });

      expect(zoom.onPinchEnd).toHaveBeenCalledTimes(1);
    });

    it("detects a double-tap and suppresses the single-tap action", async () => {
      vi.useFakeTimers();
      stubRect(900, 600);
      const zoom = makeZoom();
      mountZoom(makeSwipe(), zoom);

      // Two quick taps at the same point (center).
      await dispatch("pointerdown", 450, 300, 0);
      await dispatch("pointerup", 450, 300, 0);
      await dispatch("pointerdown", 450, 300, 100);
      await dispatch("pointerup", 450, 300, 100);

      expect(zoom.onDoubleTap).toHaveBeenCalledTimes(1);
      // The first tap's deferred action is cancelled.
      vi.advanceTimersByTime(400);
      expect(mockTap).not.toHaveBeenCalled();
    });

    it("defers a lone tap until the double-tap window passes", async () => {
      vi.useFakeTimers();
      stubRect(900, 600);
      const zoom = makeZoom();
      mountZoom(makeSwipe(), zoom);

      await dispatch("pointerdown", 450, 300, 0);
      await dispatch("pointerup", 450, 300, 0);
      expect(mockTap).not.toHaveBeenCalled(); // held to detect a double

      await act(async () => {
        vi.advanceTimersByTime(300);
      });
      expect(mockTap).toHaveBeenCalledTimes(1); // center tap → toolbar
      expect(zoom.onDoubleTap).not.toHaveBeenCalled();
    });

    it("a single tap only toggles the toolbar (never navigates) while zoomed", async () => {
      vi.useFakeTimers();
      stubRect(900, 600);
      const zoom = makeZoom(true); // panActive → zoomed
      mountZoom(makeSwipe(), zoom);

      // Tap in the right (next) zone.
      await dispatch("pointerdown", 800, 300, 0);
      await dispatch("pointerup", 800, 300, 0);
      await act(async () => {
        vi.advanceTimersByTime(300);
      });

      expect(mockTap).toHaveBeenCalledTimes(1);
      expect(mockNextPage).not.toHaveBeenCalled();
    });
  });

  describe("swipe (live drag) plumbing", () => {
    const makeSwipe = () => ({
      enabled: true,
      onStart: vi.fn(() => true),
      onMove: vi.fn(),
      onEnd: vi.fn(),
      onCancel: vi.fn(),
    });

    const mountSwipe = (swipe: ReturnType<typeof makeSwipe>) => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
          swipe,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });
    };

    it("arms on a horizontal drag and reports move + release velocity", async () => {
      const swipe = makeSwipe();
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 300, 0);
      await dispatch("pointermove", 230, 305, 50);
      await dispatch("pointermove", 260, 305, 100);
      await dispatch("pointerup", 260, 305, 100);

      expect(swipe.onStart).toHaveBeenCalledTimes(1);
      expect(swipe.onMove).toHaveBeenCalled();
      // Final move offset is +60 horizontally.
      expect(swipe.onMove).toHaveBeenLastCalledWith(60, 5);
      expect(swipe.onEnd).toHaveBeenCalledTimes(1);
      const [dragPx, , velocity] = swipe.onEnd.mock.calls[0];
      expect(dragPx).toBe(60);
      // (260-230)/(100-50) = 0.6 px/ms.
      expect(velocity).toBeCloseTo(0.6);

      // A drag must not also trigger tap/zone navigation.
      expect(mockTap).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("does not arm when onStart vetoes (pannable/zoomed)", async () => {
      const swipe = makeSwipe();
      swipe.onStart.mockReturnValue(false);
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 300, 0);
      await dispatch("pointermove", 260, 300, 50);
      await dispatch("pointerup", 260, 300, 100);

      expect(swipe.onStart).toHaveBeenCalledTimes(1);
      expect(swipe.onMove).not.toHaveBeenCalled();
      expect(swipe.onEnd).not.toHaveBeenCalled();
      expect(mockTap).not.toHaveBeenCalled();
    });

    it("never arms on a vertical-dominant drag (native scroll)", async () => {
      const swipe = makeSwipe();
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 100, 0);
      await dispatch("pointermove", 210, 200, 50);
      await dispatch("pointerup", 210, 200, 100);

      expect(swipe.onStart).not.toHaveBeenCalled();
      expect(swipe.onEnd).not.toHaveBeenCalled();
    });

    it("treats a sub-threshold press as a tap, not a swipe", async () => {
      const swipe = makeSwipe();
      vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
        left: 0,
        top: 0,
        right: 900,
        bottom: 600,
        width: 900,
        height: 600,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
      mountSwipe(swipe);

      await dispatch("pointerdown", 450, 300, 0);
      await dispatch("pointermove", 452, 301, 50);
      await dispatch("pointerup", 452, 301, 100);

      expect(swipe.onStart).not.toHaveBeenCalled();
      expect(swipe.onEnd).not.toHaveBeenCalled();
      expect(mockTap).toHaveBeenCalledTimes(1);
    });

    it("captures the pointer while a drag is armed and releases it on end", async () => {
      const swipe = makeSwipe();
      const setCapture = vi.fn();
      const releaseCapture = vi.fn();
      (element as unknown as Record<string, unknown>).setPointerCapture =
        setCapture;
      (element as unknown as Record<string, unknown>).releasePointerCapture =
        releaseCapture;
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 300, 0);
      await dispatch("pointermove", 260, 300, 50);
      expect(setCapture).toHaveBeenCalledWith(1);

      await dispatch("pointerup", 260, 300, 100);
      expect(releaseCapture).toHaveBeenCalledWith(1);
    });

    it("calls onCancel when an armed drag is cancelled", async () => {
      const swipe = makeSwipe();
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 300, 0);
      await dispatch("pointermove", 260, 300, 50);
      await dispatch("pointercancel", 260, 300, 60);

      expect(swipe.onCancel).toHaveBeenCalledTimes(1);
      expect(swipe.onEnd).not.toHaveBeenCalled();
    });

    it("is inert when the swipe block is disabled", async () => {
      const swipe = makeSwipe();
      swipe.enabled = false;
      mountSwipe(swipe);

      await dispatch("pointerdown", 200, 300, 0);
      await dispatch("pointermove", 260, 300, 50);
      await dispatch("pointerup", 260, 300, 100);

      expect(swipe.onStart).not.toHaveBeenCalled();
      expect(swipe.onMove).not.toHaveBeenCalled();
      expect(swipe.onEnd).not.toHaveBeenCalled();
    });
  });

  describe("click-only navigation (no swipe)", () => {
    it("ignores horizontal drags / swipes", async () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(200, 100, 100, 100);
      await simulateGesture(100, 100, 200, 100);

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
      expect(mockTap).not.toHaveBeenCalled();
    });

    it("ignores vertical drags / swipes", async () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(100, 100, 100, 250);

      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
      expect(mockTap).not.toHaveBeenCalled();
    });
  });

  describe("zone-aware tap dispatch", () => {
    // jsdom doesn't compute layout, so we stub getBoundingClientRect to make
    // the element 900x600 anchored at (0,0).
    const stubRect = (w = 900, h = 600) => {
      vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
        left: 0,
        top: 0,
        right: w,
        bottom: h,
        width: w,
        height: h,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
    };

    it("calls onPrevPage for a tap in the left third (LTR)", async () => {
      stubRect();
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(100, 300, 100, 300);

      expect(mockPrevPage).toHaveBeenCalledTimes(1);
      expect(mockTap).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
    });

    it("calls onTap for a tap in the middle third (LTR)", async () => {
      stubRect();
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(450, 300, 450, 300);

      expect(mockTap).toHaveBeenCalledTimes(1);
      expect(mockPrevPage).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
    });

    it("calls onNextPage for a tap in the right third (LTR)", async () => {
      stubRect();
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(800, 300, 800, 300);

      expect(mockNextPage).toHaveBeenCalledTimes(1);
      expect(mockTap).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });

    it("flips left/right zones in RTL", async () => {
      stubRect();
      useReaderStore.setState({ readingDirectionOverride: "rtl" });

      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(100, 300, 100, 300);
      expect(mockNextPage).toHaveBeenCalledTimes(1);

      await simulateGesture(800, 300, 800, 300);
      expect(mockPrevPage).toHaveBeenCalledTimes(1);
    });

    it("uses vertical thirds in TTB mode", async () => {
      stubRect();
      useReaderStore.setState({ readingDirectionOverride: "ttb" });

      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      // Top third → prev.
      await simulateGesture(450, 50, 450, 50);
      expect(mockPrevPage).toHaveBeenCalledTimes(1);

      // Middle third → toolbar toggle.
      await simulateGesture(450, 300, 450, 300);
      expect(mockTap).toHaveBeenCalledTimes(1);

      // Bottom third → next.
      await simulateGesture(450, 550, 450, 550);
      expect(mockNextPage).toHaveBeenCalledTimes(1);
    });

    it("treats every tap as a center tap when tapZones is false", async () => {
      stubRect();
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
          tapZones: false,
        }),
      );
      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(100, 300, 100, 300);
      await simulateGesture(800, 300, 800, 300);

      expect(mockTap).toHaveBeenCalledTimes(2);
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("mouse input", () => {
    it("treats a mouse click as a tap", async () => {
      vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
        left: 0,
        top: 0,
        right: 900,
        bottom: 600,
        width: 900,
        height: 600,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });

      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(450, 300, 451, 301, { pointerType: "mouse" });

      expect(mockTap).toHaveBeenCalledTimes(1);
    });

    it("ignores non-primary mouse buttons (right-click drag)", async () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(300, 200, 300, 200, {
        pointerType: "mouse",
        button: 2,
      });

      expect(mockTap).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("disabled state", () => {
    it("does not respond when disabled", async () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: false,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await simulateGesture(450, 300, 450, 300);

      expect(mockTap).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("pointer cancel", () => {
    it("clears gesture state so a follow-up pointerup is ignored", async () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onNextPage: mockNextPage,
          onPrevPage: mockPrevPage,
          onTap: mockTap,
        }),
      );

      act(() => {
        result.current.touchRef(element);
      });

      await act(async () => {
        element.dispatchEvent(
          createPointerEvent("pointerdown", 200, 100, { timeStamp: 0 }),
        );
      });
      await act(async () => {
        element.dispatchEvent(
          createPointerEvent("pointercancel", 200, 100, { timeStamp: 50 }),
        );
      });
      await act(async () => {
        element.dispatchEvent(
          createPointerEvent("pointerup", 200, 100, { timeStamp: 100 }),
        );
      });

      expect(mockTap).not.toHaveBeenCalled();
      expect(mockNextPage).not.toHaveBeenCalled();
      expect(mockPrevPage).not.toHaveBeenCalled();
    });
  });

  describe("ref management", () => {
    it("cleans up listeners when ref changes", () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onTap: mockTap,
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

      // Old element should no longer have listeners; tap on it must not fire.
      act(() => {
        element.dispatchEvent(createPointerEvent("pointerdown", 200, 200));
        element.dispatchEvent(createPointerEvent("pointerup", 200, 200));
      });

      expect(mockTap).not.toHaveBeenCalled();

      document.body.removeChild(element2);
    });

    it("handles null ref without throwing", () => {
      const { result } = renderHook(() =>
        useTouchNav({
          enabled: true,
          onTap: mockTap,
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
      vi.spyOn(element, "getBoundingClientRect").mockReturnValue({
        left: 0,
        top: 0,
        right: 900,
        bottom: 600,
        width: 900,
        height: 600,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });

      const storeNextPage = vi.spyOn(useReaderStore.getState(), "nextPage");

      useReaderStore.setState({
        currentPage: 1,
        totalPages: 10,
      });

      const { result } = renderHook(() => useTouchNav({ enabled: true }));

      act(() => {
        result.current.touchRef(element);
      });

      // Tap in the right third → next page.
      await simulateGesture(800, 300, 800, 300);

      expect(storeNextPage).toHaveBeenCalled();
    });
  });
});
