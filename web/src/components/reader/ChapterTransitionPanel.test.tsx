import { MantineProvider } from "@mantine/core";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AdjacentBook } from "@/store/readerStore";
import {
  ChapterTransitionPanel,
  formatTransitionLabels,
} from "./ChapterTransitionPanel";

function wrapper({ children }: { children: ReactNode }) {
  return <MantineProvider>{children}</MantineProvider>;
}

const nextBook: AdjacentBook = {
  id: "book-2",
  title: "S01 Chapter 017",
  pageCount: 100,
  seriesName: "The Gamer",
  number: 17,
  volume: 1,
  chapter: 17,
};

describe("formatTransitionLabels", () => {
  it("formats series, Ch. number, and S{vol} Chapter {chap}", () => {
    expect(formatTransitionLabels(nextBook)).toEqual({
      series: "The Gamer",
      primary: "Ch. 17",
      secondary: "S01 Chapter 017",
    });
  });

  it("falls back to title when no number, and omits secondary without metadata", () => {
    expect(
      formatTransitionLabels({
        id: "b",
        title: "Standalone Title",
        pageCount: 10,
        seriesName: "Standalone Title",
        number: null,
        volume: null,
        chapter: null,
      }),
    ).toEqual({
      series: "Standalone Title",
      primary: "Standalone Title",
      secondary: null,
    });
  });

  it("uses title as secondary when only number is present", () => {
    const labels = formatTransitionLabels({
      ...nextBook,
      volume: null,
      chapter: null,
      title: "The Awakening",
    });
    expect(labels.primary).toBe("Ch. 17");
    expect(labels.secondary).toBe("The Awakening");
  });
});

describe("ChapterTransitionPanel", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the next-chapter heading, labels, and a Continue button", () => {
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={vi.fn()}
      />,
      { wrapper },
    );

    expect(screen.getByText("Next Chapter")).toBeInTheDocument();
    expect(screen.getByText("The Gamer")).toBeInTheDocument();
    expect(screen.getByText("Ch. 17")).toBeInTheDocument();
    expect(screen.getByText("S01 Chapter 017")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /continue reading/i }),
    ).toBeInTheDocument();
  });

  it("renders the previous-chapter heading", () => {
    render(
      <ChapterTransitionPanel
        direction="prev"
        book={nextBook}
        onContinue={vi.fn()}
      />,
      { wrapper },
    );
    expect(screen.getByText("Previous Chapter")).toBeInTheDocument();
  });

  it("points the next arrow right in LTR and left in RTL", () => {
    const { container, rerender } = render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={vi.fn()}
        readingDirection="ltr"
      />,
      { wrapper },
    );
    expect(
      container.querySelector(".tabler-icon-arrow-right"),
    ).toBeInTheDocument();

    rerender(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={vi.fn()}
        readingDirection="rtl"
      />,
    );
    expect(
      container.querySelector(".tabler-icon-arrow-left"),
    ).toBeInTheDocument();
    expect(
      container.querySelector(".tabler-icon-arrow-right"),
    ).not.toBeInTheDocument();
  });

  it("calls onContinue when the button is clicked", async () => {
    const user = userEvent.setup();
    const onContinue = vi.fn();
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={onContinue}
      />,
      { wrapper },
    );

    await user.click(screen.getByRole("button", { name: /continue reading/i }));
    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it("shows a series-end message and no button when next book is null", () => {
    render(
      <ChapterTransitionPanel
        direction="next"
        book={null}
        onContinue={vi.fn()}
      />,
      { wrapper },
    );
    expect(screen.getByText("End of series")).toBeInTheDocument();
    expect(
      screen.getByText("You've reached the last book."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /continue reading/i }),
    ).not.toBeInTheDocument();
  });

  it("shows a series-start message when prev book is null", () => {
    render(
      <ChapterTransitionPanel
        direction="prev"
        book={null}
        onContinue={vi.fn()}
      />,
      { wrapper },
    );
    expect(screen.getByText("Beginning of series")).toBeInTheDocument();
    expect(screen.getByText("You're at the first book.")).toBeInTheDocument();
  });

  it("does not show a countdown when autoAdvance is off", () => {
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={vi.fn()}
        autoAdvance={false}
      />,
      { wrapper },
    );
    expect(
      screen.queryByText(/Continuing to next chapter/),
    ).not.toBeInTheDocument();
  });

  it("does not show a countdown for the prev direction even with autoAdvance", () => {
    render(
      <ChapterTransitionPanel
        direction="prev"
        book={nextBook}
        onContinue={vi.fn()}
        autoAdvance
      />,
      { wrapper },
    );
    expect(screen.queryByText(/Continuing/)).not.toBeInTheDocument();
  });

  it("runs the countdown and auto-advances when autoAdvance is on", () => {
    vi.useFakeTimers();
    const onContinue = vi.fn();
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={onContinue}
        autoAdvance
        countdownSeconds={3}
      />,
      { wrapper },
    );

    expect(
      screen.getByText(/Continuing to next chapter in 3 seconds/),
    ).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it("cancels the countdown and reports the cancel", () => {
    vi.useFakeTimers();
    const onContinue = vi.fn();
    const onCancelAutoAdvance = vi.fn();
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={onContinue}
        autoAdvance
        countdownSeconds={3}
        onCancelAutoAdvance={onCancelAutoAdvance}
      />,
      { wrapper },
    );

    act(() => {
      screen.getByRole("button", { name: /cancel/i }).click();
    });

    expect(onCancelAutoAdvance).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByText(/Continuing to next chapter/),
    ).not.toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(onContinue).not.toHaveBeenCalled();
  });

  it("reserves the countdown space so the layout does not shift on cancel", () => {
    vi.useFakeTimers();
    render(
      <ChapterTransitionPanel
        direction="next"
        book={nextBook}
        onContinue={vi.fn()}
        autoAdvance
        countdownSeconds={3}
      />,
      { wrapper },
    );

    // The reserved slot exists while counting down...
    const slot = screen.getByTestId("auto-advance-slot");
    expect(slot).toBeInTheDocument();
    expect(
      screen.getByText(/Continuing to next chapter in 3 seconds/),
    ).toBeInTheDocument();

    act(() => {
      screen.getByRole("button", { name: /cancel/i }).click();
    });

    // ...and remains mounted after cancel (only its contents clear), so the
    // cover/button above it never reflow.
    expect(screen.getByTestId("auto-advance-slot")).toBe(slot);
    expect(
      screen.queryByText(/Continuing to next chapter/),
    ).not.toBeInTheDocument();

    vi.useRealTimers();
  });

  it("does not reserve countdown space on the prev panel", () => {
    render(
      <ChapterTransitionPanel
        direction="prev"
        book={nextBook}
        onContinue={vi.fn()}
        autoAdvance
      />,
      { wrapper },
    );
    expect(screen.queryByTestId("auto-advance-slot")).not.toBeInTheDocument();
  });
});
