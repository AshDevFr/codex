import { beforeEach, describe, expect, it } from "vitest";
import {
  sortForMetric,
  useReadingStatsPreferencesStore,
} from "./readingStatsPreferencesStore";

describe("useReadingStatsPreferencesStore", () => {
  beforeEach(() => {
    useReadingStatsPreferencesStore.setState({ metric: "time" });
  });

  /// Time is the richest measure when it exists, so it leads until the reader
  /// says otherwise.
  it("starts on time", () => {
    expect(useReadingStatsPreferencesStore.getState().metric).toBe("time");
  });

  it("remembers the chosen metric", () => {
    useReadingStatsPreferencesStore.getState().setMetric("booksFinished");

    expect(useReadingStatsPreferencesStore.getState().metric).toBe(
      "booksFinished",
    );
  });

  it("persists under its own storage key", () => {
    useReadingStatsPreferencesStore.getState().setMetric("pages");

    const stored = window.localStorage.getItem(
      "reading-stats-preferences-storage",
    );
    expect(stored).toContain("pages");
  });
});

/// The server applies the series limit, so a metric the UI draws must be
/// ranked by the matching key or the top rows are chosen by the wrong measure.
describe("sortForMetric", () => {
  it("maps each metric to the ranking key the API understands", () => {
    expect(sortForMetric("time")).toBe("time");
    expect(sortForMetric("pages")).toBe("pages");
    expect(sortForMetric("booksFinished")).toBe("completions");
  });
});
