import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReadingDirection } from "@/store/readerStore";
import { act, renderWithProviders, screen } from "@/test/utils";
import { SwipePager } from "./SwipePager";

/**
 * jsdom has no layout engine, so we stub the root element's bounding box to a
 * known viewport width and drive pointer events manually. The drag math lives in
 * unit-tested helpers (`swipeGesture`); these tests assert the wiring: a drag
 * past threshold commits, a short drag snaps back, taps still toggle, and the
 * disabled flag is inert.
 */

const VIEWPORT_W = 1000;
const VIEWPORT_H = 600;
const DURATION = 200;

type PointerKind = "touch" | "mouse" | "pen";

const createPointerEvent = (
  type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
  x: number,
  y: number,
  t: number,
  pointerId = 1,
): PointerEvent => {
  const event = new MouseEvent(type, {
    clientX: x,
    clientY: y,
    button: 0,
    bubbles: true,
    cancelable: true,
  }) as MouseEvent & {
    pointerId: number;
    pointerType: PointerKind;
    isPrimary: boolean;
  };
  Object.defineProperty(event, "pointerId", { value: pointerId });
  Object.defineProperty(event, "pointerType", { value: "touch" });
  Object.defineProperty(event, "isPrimary", { value: pointerId === 1 });
  Object.defineProperty(event, "timeStamp", { value: t });
  return event as unknown as PointerEvent;
};

interface Handlers {
  onNext: ReturnType<typeof vi.fn>;
  onPrev: ReturnType<typeof vi.fn>;
  onTap: ReturnType<typeof vi.fn>;
  onExit: ReturnType<typeof vi.fn>;
}

const renderPager = (
  opts: {
    enabled?: boolean;
    readingDirection?: ReadingDirection;
    prev?: boolean;
    next?: boolean;
    isContentPannable?: () => boolean;
  } = {},
): { root: HTMLElement; handlers: Handlers } => {
  const {
    enabled = true,
    readingDirection = "ltr",
    prev = true,
    next = true,
    isContentPannable,
  } = opts;
  const handlers: Handlers = {
    onNext: vi.fn(),
    onPrev: vi.fn(),
    onTap: vi.fn(),
    onExit: vi.fn(),
  };

  const { container } = renderWithProviders(
    <SwipePager
      current={<div>current</div>}
      prev={prev ? <div>prev</div> : null}
      next={next ? <div>next</div> : null}
      pageKey="5"
      readingDirection={readingDirection}
      onNext={handlers.onNext}
      onPrev={handlers.onPrev}
      onTap={handlers.onTap}
      onExit={handlers.onExit}
      enabled={enabled}
      duration={DURATION}
      isContentPannable={isContentPannable}
    />,
  );

  // Mantine injects a <style> as the first child; the pager root is the first div.
  const root = container.querySelector("div") as HTMLElement;
  vi.spyOn(root, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    right: VIEWPORT_W,
    bottom: VIEWPORT_H,
    width: VIEWPORT_W,
    height: VIEWPORT_H,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  return { root, handlers };
};

const fire = async (
  root: HTMLElement,
  type: "pointerdown" | "pointermove" | "pointerup" | "pointercancel",
  x: number,
  y: number,
  t: number,
  pointerId = 1,
) => {
  await act(async () => {
    root.dispatchEvent(createPointerEvent(type, x, y, t, pointerId));
  });
};

/** The element carrying the current page's zoom transform (wraps "current"). */
const zoomEl = (): HTMLElement =>
  screen.getByText("current").parentElement as HTMLElement;

/** Drag horizontally from startX to endX at y=300, then release. */
const drag = async (root: HTMLElement, startX: number, endX: number) => {
  const midX = (startX + endX) / 2;
  await fire(root, "pointerdown", startX, 300, 0);
  await fire(root, "pointermove", midX, 300, 100);
  await fire(root, "pointermove", endX, 300, 200);
  await fire(root, "pointerup", endX, 300, 300);
};

/** Drag vertically from startY to endY at x=500, then release. */
const dragVertical = async (
  root: HTMLElement,
  startY: number,
  endY: number,
) => {
  const midY = (startY + endY) / 2;
  await fire(root, "pointerdown", 500, startY, 0);
  await fire(root, "pointermove", 500, midY, 100);
  await fire(root, "pointermove", 500, endY, 200);
  await fire(root, "pointerup", 500, endY, 300);
};

describe("SwipePager", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("commits next on a far leftward drag (LTR)", async () => {
    const { root, handlers } = renderPager();

    await drag(root, 700, 200); // dx = -500

    expect(handlers.onNext).not.toHaveBeenCalled(); // not until the snap finishes
    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });
    expect(handlers.onNext).toHaveBeenCalledTimes(1);
    expect(handlers.onPrev).not.toHaveBeenCalled();
  });

  it("commits prev on a far rightward drag (LTR)", async () => {
    const { root, handlers } = renderPager();

    await drag(root, 300, 800); // dx = +500

    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });
    expect(handlers.onPrev).toHaveBeenCalledTimes(1);
    expect(handlers.onNext).not.toHaveBeenCalled();
  });

  it("snaps back without committing on a short slow drag", async () => {
    const { root, handlers } = renderPager();

    // dx = -40 over 200ms: below distance threshold and slow.
    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointermove", 480, 300, 200);
    await fire(root, "pointerup", 460, 300, 400);

    await act(async () => {
      vi.advanceTimersByTime(DURATION * 2);
    });
    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
  });

  it("resets the track to centered after a sub-threshold drag (snap back)", async () => {
    const { root } = renderPager();
    const track = (screen.getByText("current").parentElement as HTMLElement)
      .parentElement?.parentElement as HTMLElement;

    // Arm a drag (so the track is offset imperatively), then release short.
    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointermove", 470, 300, 100); // dx = -30, offsets the track
    await fire(root, "pointerup", 460, 300, 200); // dx = -40, stays

    // The release must drive the track back to the centered slot, not leave it
    // parked at the dragged offset.
    expect(track.style.transform).toBe("translateX(calc(-100% + 0px))");
  });

  it("resets the track to centered when a drag is cancelled", async () => {
    const { root } = renderPager();
    const track = (screen.getByText("current").parentElement as HTMLElement)
      .parentElement?.parentElement as HTMLElement;

    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointermove", 470, 300, 100); // offsets the track
    await fire(root, "pointercancel", 470, 300, 120);

    expect(track.style.transform).toBe("translateX(calc(-100% + 0px))");
  });

  it("flips polarity in RTL (leftward drag = prev)", async () => {
    const { root, handlers } = renderPager({ readingDirection: "rtl" });

    await drag(root, 700, 200); // leftward

    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });
    expect(handlers.onPrev).toHaveBeenCalledTimes(1);
    expect(handlers.onNext).not.toHaveBeenCalled();
  });

  it("stays at the edge when there is no next spread", async () => {
    const { root, handlers } = renderPager({ next: false });

    await drag(root, 700, 200); // leftward = next, but no next page

    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });
    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
  });

  it("exits the reader on a downward fling", async () => {
    const { root, handlers } = renderPager();

    // Drop from near the top to near the bottom (VIEWPORT_H = 600).
    await dragVertical(root, 80, 520);

    expect(handlers.onExit).toHaveBeenCalledTimes(1);
    // A downward fling must not turn the page or fire a tap.
    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
    expect(handlers.onTap).not.toHaveBeenCalled();
  });

  it("does not exit on a downward fling when the content is pannable", async () => {
    const { root, handlers } = renderPager({
      isContentPannable: () => true,
    });

    await dragVertical(root, 80, 520);

    expect(handlers.onExit).not.toHaveBeenCalled();
  });

  it("preserves the committed slide's DOM node so its image is not reloaded", () => {
    // The page just turned to (the old `next`) becomes the new `current`. Its
    // already-decoded <img> must move into the center slot, not be torn down and
    // re-created (which re-decodes and flashes the previous page on production).
    const shared = {
      readingDirection: "ltr" as ReadingDirection,
      onNext: vi.fn(),
      onPrev: vi.fn(),
      onTap: vi.fn(),
      enabled: true,
      duration: DURATION,
    };
    const { container, rerender } = renderWithProviders(
      <SwipePager
        {...shared}
        current={<div>A</div>}
        prev={null}
        next={<div data-testid="slide-B">B</div>}
        pageKey="A"
        nextKey="B"
      />,
    );
    const root = container.querySelector("div") as HTMLElement;
    expect(root).toBeTruthy();
    const bBefore = screen.getByTestId("slide-B");

    // Commit the turn: B is now current, A is prev, C is the new next.
    rerender(
      <SwipePager
        {...shared}
        current={<div data-testid="slide-B">B</div>}
        prev={<div>A</div>}
        next={<div>C</div>}
        pageKey="B"
        prevKey="A"
        nextKey="C"
      />,
    );

    // Same DOM element, moved to the center — not a re-created node.
    expect(screen.getByTestId("slide-B")).toBe(bBefore);
  });

  it("keeps the live drag offset across an unrelated re-render", async () => {
    // The track transform is written imperatively, not via React state. An
    // unrelated re-render mid-gesture (e.g. a neighbour image loading, a
    // preload/orientation update) must not re-apply a stale transform and clobber
    // the in-flight drag/snap — that was the cause of the commit-time black flash.
    const shared = {
      readingDirection: "ltr" as ReadingDirection,
      onNext: vi.fn(),
      onPrev: vi.fn(),
      onTap: vi.fn(),
      enabled: true,
      duration: DURATION,
    };
    const slides = {
      current: <div>current</div>,
      prev: <div>p</div>,
      next: <div>n</div>,
      pageKey: "5",
      prevKey: "4",
      nextKey: "6",
    };
    const { container, rerender } = renderWithProviders(
      <SwipePager {...shared} {...slides} />,
    );
    const root = container.querySelector("div") as HTMLElement;
    vi.spyOn(root, "getBoundingClientRect").mockReturnValue({
      left: 0,
      top: 0,
      right: VIEWPORT_W,
      bottom: VIEWPORT_H,
      width: VIEWPORT_W,
      height: VIEWPORT_H,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    const track = (screen.getByText("current").parentElement as HTMLElement)
      .parentElement?.parentElement as HTMLElement;

    // Arm a drag and offset the strip by -50px.
    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointermove", 450, 300, 50);
    expect(track.style.transform).toBe("translateX(calc(-100% + -50px))");

    // A re-render with unchanged navigation must leave the drag offset intact.
    rerender(<SwipePager {...shared} {...slides} />);
    expect(track.style.transform).toBe("translateX(calc(-100% + -50px))");
  });

  it("routes a center tap to onTap, not navigation", async () => {
    const { root, handlers } = renderPager();

    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointerup", 500, 300, 0);

    // Taps are held briefly to detect a double-tap (zoom); advance past it.
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(handlers.onTap).toHaveBeenCalledTimes(1);
    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
  });

  it("is inert when disabled", async () => {
    const { root, handlers } = renderPager({ enabled: false });

    await drag(root, 700, 200);
    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointerup", 500, 300, 0);

    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });
    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
    expect(handlers.onTap).not.toHaveBeenCalled();
  });

  // Pinch two fingers apart, centered on the viewport, to scale 2.5
  // (separation 200 → 500 at viewport center 500,300).
  const pinchOpen = async (root: HTMLElement) => {
    await fire(root, "pointerdown", 400, 300, 0, 1);
    await fire(root, "pointerdown", 600, 300, 0, 2);
    await fire(root, "pointermove", 900, 300, 50, 2);
  };

  it("pinches to zoom the current page only", async () => {
    const { root } = renderPager();

    await pinchOpen(root);

    expect(zoomEl().style.transform).toContain("scale(2.5)");
    // Neighbors are not wrapped in the zoom element, so they stay at fit.
    expect(screen.getByText("prev").parentElement?.style.transform).toBe("");
  });

  it("double-tap zooms the page in, then back out to fit", async () => {
    const { root } = renderPager();

    const doubleTap = async (x: number, y: number, base: number) => {
      await fire(root, "pointerdown", x, y, base, 1);
      await fire(root, "pointerup", x, y, base, 1);
      await fire(root, "pointerdown", x, y, base + 80, 1);
      await fire(root, "pointerup", x, y, base + 80, 1);
    };

    await doubleTap(500, 300, 0);
    expect(zoomEl().style.transform).toContain("scale(2.5)");

    await doubleTap(500, 300, 1000);
    expect(zoomEl().style.transform).toContain("scale(1)");
  });

  it("pans instead of turning the page while zoomed", async () => {
    const { root, handlers } = renderPager();

    await pinchOpen(root);
    await fire(root, "pointerup", 900, 300, 60, 2);
    await fire(root, "pointerup", 400, 300, 60, 1);

    const before = zoomEl().style.transform;

    // One-finger horizontal drag: pans (transform changes), does not navigate.
    await fire(root, "pointerdown", 500, 300, 100, 1);
    await fire(root, "pointermove", 450, 300, 120, 1);
    await fire(root, "pointermove", 420, 300, 140, 1);
    await fire(root, "pointerup", 420, 300, 160, 1);

    await act(async () => {
      vi.advanceTimersByTime(DURATION);
    });

    expect(handlers.onNext).not.toHaveBeenCalled();
    expect(handlers.onPrev).not.toHaveBeenCalled();
    expect(zoomEl().style.transform).not.toBe(before);
    expect(zoomEl().style.transform).toContain("scale(2.5)");
  });

  it("re-centers the strip instantly (no animated transition) when the committed spread changes", async () => {
    // Regression guard for the production-only swipe flash: after a turn commits
    // and the page advances (pageKey changes), the filmstrip must snap back to
    // its centered slot with the transition disabled, so the new page and the
    // re-center land in one frame instead of briefly revealing the old page.
    const props = {
      current: <div>current</div>,
      prev: <div>prev</div>,
      next: <div>next</div>,
      readingDirection: "ltr" as ReadingDirection,
      onNext: vi.fn(),
      onPrev: vi.fn(),
      onTap: vi.fn(),
      enabled: true,
      duration: DURATION,
    };
    const { container, rerender } = renderWithProviders(
      <SwipePager {...props} pageKey="5" />,
    );
    const root = container.querySelector("div") as HTMLElement;
    // The flex track wraps the three slots; reach it via the centered slot.
    const track = (screen.getByText("current").parentElement as HTMLElement)
      .parentElement?.parentElement as HTMLElement;

    rerender(<SwipePager {...props} pageKey="6" />);

    expect(track.style.transition).toBe("none");
    expect(track.style.transform).toBe("translateX(calc(-100% + 0px))");
    expect(root).toBeTruthy();
  });

  it("resets zoom when the page changes", async () => {
    const props = {
      current: <div>current</div>,
      prev: <div>prev</div>,
      next: <div>next</div>,
      readingDirection: "ltr" as ReadingDirection,
      onNext: vi.fn(),
      onPrev: vi.fn(),
      onTap: vi.fn(),
      enabled: true,
      duration: DURATION,
    };
    const { container, rerender } = renderWithProviders(
      <SwipePager {...props} pageKey="5" />,
    );
    const root = container.querySelector("div") as HTMLElement;
    vi.spyOn(root, "getBoundingClientRect").mockReturnValue({
      left: 0,
      top: 0,
      right: VIEWPORT_W,
      bottom: VIEWPORT_H,
      width: VIEWPORT_W,
      height: VIEWPORT_H,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });

    await pinchOpen(root);
    expect(zoomEl().style.transform).toContain("scale(2.5)");

    rerender(<SwipePager {...props} pageKey="6" />);
    expect(zoomEl().style.transform).toContain("scale(1)");
  });
});
