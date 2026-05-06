import { describe, expect, it } from "vitest";
import { elapsedSince, formatElapsed } from "./duration";

describe("formatElapsed", () => {
  it("renders sub-second values as 0s", () => {
    expect(formatElapsed(400)).toBe("0s");
    expect(formatElapsed(0)).toBe("0s");
  });

  it("renders seconds-only durations", () => {
    expect(formatElapsed(12_000)).toBe("12s");
    expect(formatElapsed(59_000)).toBe("59s");
  });

  it("renders minute + second durations", () => {
    expect(formatElapsed(60_000)).toBe("1m 0s");
    expect(formatElapsed(84_000)).toBe("1m 24s");
    expect(formatElapsed(59 * 60 * 1000 + 30_000)).toBe("59m 30s");
  });

  it("renders hour + minute durations and drops seconds past the hour mark", () => {
    expect(formatElapsed(3_600_000)).toBe("1h 0m");
    expect(formatElapsed(7_500_000)).toBe("2h 5m");
  });

  it("clamps invalid input to 0s", () => {
    expect(formatElapsed(-5)).toBe("0s");
    expect(formatElapsed(Number.NaN)).toBe("0s");
    expect(formatElapsed(Number.POSITIVE_INFINITY)).toBe("0s");
  });
});

describe("elapsedSince", () => {
  it("returns 0 for missing or invalid timestamps", () => {
    expect(elapsedSince(null)).toBe(0);
    expect(elapsedSince(undefined)).toBe(0);
    expect(elapsedSince("not-a-date")).toBe(0);
  });

  it("computes elapsed milliseconds against the supplied now", () => {
    const startedAt = "2026-05-04T12:00:00.000Z";
    const now = Date.parse("2026-05-04T12:00:30.000Z");
    expect(elapsedSince(startedAt, now)).toBe(30_000);
  });

  it("clamps future startedAt values to 0", () => {
    const startedAt = "2030-01-01T00:00:00.000Z";
    const now = Date.parse("2026-01-01T00:00:00.000Z");
    expect(elapsedSince(startedAt, now)).toBe(0);
  });
});
