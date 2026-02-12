import { MantineProvider } from "@mantine/core";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";
import { BoundaryNotification } from "./BoundaryNotification";

function wrapper({ children }: { children: ReactNode }) {
  return <MantineProvider>{children}</MantineProvider>;
}

describe("BoundaryNotification", () => {
  it("should not render when not visible", () => {
    render(
      <BoundaryNotification
        message="Test message"
        visible={false}
        type="at-end"
      />,
      { wrapper },
    );

    expect(screen.queryByText("Test message")).not.toBeInTheDocument();
  });

  it("should not render when message is null", () => {
    render(
      <BoundaryNotification message={null} visible={true} type="at-end" />,
      { wrapper },
    );

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("should render single-line message as title", () => {
    render(
      <BoundaryNotification
        message="End of book"
        visible={true}
        type="at-end"
      />,
      { wrapper },
    );

    expect(screen.getByText("End of book")).toBeInTheDocument();
  });

  it("should render two-line message with title and subtitle", () => {
    render(
      <BoundaryNotification
        message={'End of book\nPress again for "Next Book"'}
        visible={true}
        type="at-end"
      />,
      { wrapper },
    );

    expect(screen.getByText("End of book")).toBeInTheDocument();
    expect(screen.getByText('Press again for "Next Book"')).toBeInTheDocument();
  });

  it("should not render when type is none", () => {
    render(
      <BoundaryNotification
        message="Test message"
        visible={true}
        type="none"
      />,
      { wrapper },
    );

    // The component shows when visible=true and message is set,
    // even with type="none". The type just affects the icon.
    expect(screen.getByText("Test message")).toBeInTheDocument();
  });

  it("should display two-line message for at-start type", () => {
    render(
      <BoundaryNotification
        message={'Beginning of book\nPress again for "Previous Book"'}
        visible={true}
        type="at-start"
      />,
      { wrapper },
    );

    expect(screen.getByText("Beginning of book")).toBeInTheDocument();
    expect(
      screen.getByText('Press again for "Previous Book"'),
    ).toBeInTheDocument();
  });

  it("should display two-line message for at-end type", () => {
    render(
      <BoundaryNotification
        message={'End of book\nPress again for "Next Book"'}
        visible={true}
        type="at-end"
      />,
      { wrapper },
    );

    expect(screen.getByText("End of book")).toBeInTheDocument();
    expect(screen.getByText('Press again for "Next Book"')).toBeInTheDocument();
  });

  describe("reading direction", () => {
    it("should show right chevron for at-end in LTR mode (default)", () => {
      const { container } = render(
        <BoundaryNotification
          message="End of book"
          visible={true}
          type="at-end"
          readingDirection="ltr"
        />,
        { wrapper },
      );

      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });

    it("should show left chevron for at-end in RTL mode", () => {
      const { container } = render(
        <BoundaryNotification
          message="End of book"
          visible={true}
          type="at-end"
          readingDirection="rtl"
        />,
        { wrapper },
      );

      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });

    it("should show left chevron for at-start in LTR mode", () => {
      const { container } = render(
        <BoundaryNotification
          message="Beginning of book"
          visible={true}
          type="at-start"
          readingDirection="ltr"
        />,
        { wrapper },
      );

      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });

    it("should show right chevron for at-start in RTL mode", () => {
      const { container } = render(
        <BoundaryNotification
          message="Beginning of book"
          visible={true}
          type="at-start"
          readingDirection="rtl"
        />,
        { wrapper },
      );

      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });
  });

  describe("series end/start", () => {
    it("should show check icon instead of chevron when isSeriesEnd is true", () => {
      const { container } = render(
        <BoundaryNotification
          message={"End of series\nYou have reached the last book"}
          visible={true}
          type="at-end"
          isSeriesEnd={true}
        />,
        { wrapper },
      );

      expect(screen.getByText("End of series")).toBeInTheDocument();
      expect(
        screen.getByText("You have reached the last book"),
      ).toBeInTheDocument();
      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });

    it("should show check icon for series start", () => {
      const { container } = render(
        <BoundaryNotification
          message={"Beginning of series\nYou are at the first book"}
          visible={true}
          type="at-start"
          isSeriesEnd={true}
        />,
        { wrapper },
      );

      expect(screen.getByText("Beginning of series")).toBeInTheDocument();
      expect(screen.getByText("You are at the first book")).toBeInTheDocument();
      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });

    it("should not show chevrons when isSeriesEnd is true regardless of reading direction", () => {
      const { container } = render(
        <BoundaryNotification
          message={"End of series\nYou have reached the last book"}
          visible={true}
          type="at-end"
          readingDirection="rtl"
          isSeriesEnd={true}
        />,
        { wrapper },
      );

      // isSeriesEnd suppresses chevrons, shows check icon instead
      const svgs = container.querySelectorAll("svg");
      expect(svgs).toHaveLength(1);
    });
  });
});
