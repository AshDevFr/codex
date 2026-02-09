import { describe, expect, it } from "vitest";
import { applyStaleness } from "./index.js";

// =============================================================================
// applyStaleness Tests
// =============================================================================

describe("applyStaleness", () => {
  // Helper: returns a timestamp N days ago from a fixed reference point
  const now = new Date("2026-02-08T12:00:00Z").getTime();
  const daysAgo = (days: number) => new Date(now - days * 24 * 60 * 60 * 1000).toISOString();

  describe("passthrough cases", () => {
    it("returns status unchanged when not reading", () => {
      expect(applyStaleness("completed", daysAgo(100), 30, 60, now)).toBe("completed");
      expect(applyStaleness("on_hold", daysAgo(100), 30, 60, now)).toBe("on_hold");
      expect(applyStaleness("dropped", daysAgo(100), 30, 60, now)).toBe("dropped");
      expect(applyStaleness("plan_to_read", daysAgo(100), 30, 60, now)).toBe("plan_to_read");
    });

    it("returns reading when both thresholds are 0 (disabled)", () => {
      expect(applyStaleness("reading", daysAgo(365), 0, 0, now)).toBe("reading");
    });

    it("returns reading when latestUpdatedAt is undefined", () => {
      expect(applyStaleness("reading", undefined, 30, 60, now)).toBe("reading");
    });

    it("returns reading when latestUpdatedAt is invalid", () => {
      expect(applyStaleness("reading", "not-a-date", 30, 60, now)).toBe("reading");
    });

    it("returns reading when activity is recent", () => {
      expect(applyStaleness("reading", daysAgo(5), 30, 60, now)).toBe("reading");
    });
  });

  describe("pause only (drop disabled)", () => {
    it("pauses after threshold", () => {
      expect(applyStaleness("reading", daysAgo(31), 30, 0, now)).toBe("on_hold");
    });

    it("pauses at exact threshold", () => {
      expect(applyStaleness("reading", daysAgo(30), 30, 0, now)).toBe("on_hold");
    });

    it("does not pause below threshold", () => {
      expect(applyStaleness("reading", daysAgo(29), 30, 0, now)).toBe("reading");
    });
  });

  describe("drop only (pause disabled)", () => {
    it("drops after threshold", () => {
      expect(applyStaleness("reading", daysAgo(61), 0, 60, now)).toBe("dropped");
    });

    it("drops at exact threshold", () => {
      expect(applyStaleness("reading", daysAgo(60), 0, 60, now)).toBe("dropped");
    });

    it("does not drop below threshold", () => {
      expect(applyStaleness("reading", daysAgo(59), 0, 60, now)).toBe("reading");
    });
  });

  describe("both pause and drop enabled", () => {
    it("pauses when inactive past pause but not drop threshold", () => {
      // pause=30, drop=60, inactive=45 → pause
      expect(applyStaleness("reading", daysAgo(45), 30, 60, now)).toBe("on_hold");
    });

    it("drops when inactive past both thresholds (drop takes priority)", () => {
      // pause=30, drop=60, inactive=90 → drop (stronger action)
      expect(applyStaleness("reading", daysAgo(90), 30, 60, now)).toBe("dropped");
    });

    it("drops at exact drop threshold even when pause threshold is also met", () => {
      expect(applyStaleness("reading", daysAgo(60), 30, 60, now)).toBe("dropped");
    });

    it("does nothing when active within both thresholds", () => {
      expect(applyStaleness("reading", daysAgo(10), 30, 60, now)).toBe("reading");
    });
  });

  describe("edge cases", () => {
    it("handles future latestUpdatedAt (0 days inactive)", () => {
      const future = new Date(now + 24 * 60 * 60 * 1000).toISOString();
      expect(applyStaleness("reading", future, 30, 60, now)).toBe("reading");
    });

    it("handles very old latestUpdatedAt", () => {
      expect(applyStaleness("reading", "2020-01-01T00:00:00Z", 30, 60, now)).toBe("dropped");
    });

    it("uses Date.now() when now parameter is omitted", () => {
      // Activity 1000 days ago with threshold of 1 day → should pause
      const veryOld = new Date(Date.now() - 1000 * 24 * 60 * 60 * 1000).toISOString();
      expect(applyStaleness("reading", veryOld, 1, 0)).toBe("on_hold");
    });
  });
});
