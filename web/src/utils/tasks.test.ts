import { describe, expect, it } from "vitest";
import type { ActiveTask } from "@/types";
import { getTaskLabel, getTaskTarget } from "./tasks";

const baseTask: ActiveTask = {
  taskId: "00000000-0000-0000-0000-000000000000",
  taskType: "analyze_book",
  status: "running",
  startedAt: "2026-05-04T12:00:00.000Z",
};

describe("getTaskTarget", () => {
  it("prefers bookTitle over series and library", () => {
    expect(
      getTaskTarget({
        ...baseTask,
        bookTitle: "Naruto Vol. 12",
        seriesTitle: "Naruto",
        libraryName: "Manga Library",
      }),
    ).toBe("Naruto Vol. 12");
  });

  it("falls back to seriesTitle when book is absent", () => {
    expect(
      getTaskTarget({
        ...baseTask,
        seriesTitle: "Naruto",
        libraryName: "Manga Library",
      }),
    ).toBe("Naruto");
  });

  it("falls back to libraryName when neither book nor series is set", () => {
    expect(
      getTaskTarget({
        ...baseTask,
        libraryName: "Manga Library",
      }),
    ).toBe("Manga Library");
  });

  it("returns null when no target is set", () => {
    expect(getTaskTarget(baseTask)).toBeNull();
  });

  it("treats explicit nulls as missing", () => {
    expect(
      getTaskTarget({
        ...baseTask,
        bookTitle: null,
        seriesTitle: null,
        libraryName: null,
      }),
    ).toBeNull();
  });
});

describe("getTaskLabel", () => {
  it("prefers the live progress message over the resolved target", () => {
    expect(
      getTaskLabel({
        ...baseTask,
        progress: { current: 0, total: 1, message: "Analyzing i1.cbz" },
        bookTitle: "Naruto Vol. 12",
      }),
    ).toBe("Analyzing i1.cbz");
  });

  it("falls back to the resolved target when no message has arrived", () => {
    // A task seeded from the polling snapshot carries a title but no progress:
    // SSE is the only source of messages, and analyze tasks emit theirs once at
    // start, so a page opened mid-task never sees it.
    expect(
      getTaskLabel({
        ...baseTask,
        bookTitle: "Naruto Vol. 12",
      }),
    ).toBe("Naruto Vol. 12");
  });

  it("falls back to the target when the message is blank", () => {
    expect(
      getTaskLabel({
        ...baseTask,
        progress: { current: 0, total: 1, message: "   " },
        seriesTitle: "Naruto",
      }),
    ).toBe("Naruto");
  });

  it("falls back to the target when progress carries no message", () => {
    expect(
      getTaskLabel({
        ...baseTask,
        progress: { current: 3, total: 10 },
        libraryName: "Manga Library",
      }),
    ).toBe("Manga Library");
  });

  it("returns the generic placeholder when nothing is known", () => {
    expect(getTaskLabel(baseTask)).toBe("Processing...");
  });
});
