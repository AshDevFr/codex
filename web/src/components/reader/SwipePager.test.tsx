import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReadingDirection } from "@/store/readerStore";
import { act, renderWithProviders } from "@/test/utils";
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
  Object.defineProperty(event, "pointerId", { value: 1 });
  Object.defineProperty(event, "pointerType", { value: "touch" });
  Object.defineProperty(event, "isPrimary", { value: true });
  Object.defineProperty(event, "timeStamp", { value: t });
  return event as unknown as PointerEvent;
};

interface Handlers {
  onNext: ReturnType<typeof vi.fn>;
  onPrev: ReturnType<typeof vi.fn>;
  onTap: ReturnType<typeof vi.fn>;
}

const renderPager = (
  opts: {
    enabled?: boolean;
    readingDirection?: ReadingDirection;
    prev?: boolean;
    next?: boolean;
  } = {},
): { root: HTMLElement; handlers: Handlers } => {
  const {
    enabled = true,
    readingDirection = "ltr",
    prev = true,
    next = true,
  } = opts;
  const handlers: Handlers = {
    onNext: vi.fn(),
    onPrev: vi.fn(),
    onTap: vi.fn(),
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
      enabled={enabled}
      duration={DURATION}
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
) => {
  await act(async () => {
    root.dispatchEvent(createPointerEvent(type, x, y, t));
  });
};

/** Drag horizontally from startX to endX at y=300, then release. */
const drag = async (root: HTMLElement, startX: number, endX: number) => {
  const midX = (startX + endX) / 2;
  await fire(root, "pointerdown", startX, 300, 0);
  await fire(root, "pointermove", midX, 300, 100);
  await fire(root, "pointermove", endX, 300, 200);
  await fire(root, "pointerup", endX, 300, 300);
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

  it("routes a center tap to onTap, not navigation", async () => {
    const { root, handlers } = renderPager();

    await fire(root, "pointerdown", 500, 300, 0);
    await fire(root, "pointerup", 500, 300, 0);

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
});
