import { describe, expect, it } from "vitest";
import type { ActiveTask } from "@/types";
import { getTaskTarget } from "./tasks";

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
