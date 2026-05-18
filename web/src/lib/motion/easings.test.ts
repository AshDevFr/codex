import { describe, expect, it } from "vitest";
import { EASE_IN_OUT, EASE_OUT, SPRING_QUICK, SPRING_SOFT } from "./easings";

describe("motion easings", () => {
  it("exposes a 4-tuple cubic-bezier for EASE_OUT", () => {
    expect(EASE_OUT).toEqual([0.32, 0.72, 0, 1]);
  });

  it("exposes a 4-tuple cubic-bezier for EASE_IN_OUT", () => {
    expect(EASE_IN_OUT).toEqual([0.65, 0, 0.35, 1]);
  });

  it("exposes a soft spring config", () => {
    expect(SPRING_SOFT).toEqual({
      type: "spring",
      stiffness: 220,
      damping: 30,
    });
  });

  it("exposes a quick spring config", () => {
    expect(SPRING_QUICK).toEqual({
      type: "spring",
      stiffness: 360,
      damping: 32,
    });
  });
});
