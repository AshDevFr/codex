import { render, screen } from "@testing-library/react";
import { m } from "motion/react";
import { describe, expect, it } from "vitest";
import { MotionProvider } from "./MotionProvider";

describe("MotionProvider", () => {
  it("renders children inside the LazyMotion tree", () => {
    render(
      <MotionProvider>
        <div data-testid="child">hello</div>
      </MotionProvider>,
    );

    expect(screen.getByTestId("child")).toHaveTextContent("hello");
  });

  it("permits slim <m.*> components to animate without throwing", () => {
    render(
      <MotionProvider>
        <m.div
          data-testid="animated"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0 }}
        >
          fade
        </m.div>
      </MotionProvider>,
    );

    expect(screen.getByTestId("animated")).toBeInTheDocument();
  });
});
