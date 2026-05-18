import { LazyMotion } from "motion/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import domAnimation from "@/lib/motion/domAnimation";
import { fireEvent, renderWithProviders, screen } from "@/test/utils";
import { FilterBottomSheet } from "./FilterBottomSheet";

// Motion components need LazyMotion in the tree. The shared test wrapper
// doesn't provide one (the production tree gets it from MotionProvider in
// main.tsx), so we wrap the SUT explicitly here.
function renderSheet(ui: ReactNode) {
  return renderWithProviders(
    <LazyMotion features={domAnimation}>{ui}</LazyMotion>,
  );
}

describe("FilterBottomSheet", () => {
  it("does not render any sheet content when closed", () => {
    renderSheet(
      <FilterBottomSheet
        opened={false}
        onClose={vi.fn()}
        title="Filters"
        footer={<button type="button">Apply</button>}
      >
        <p>body</p>
      </FilterBottomSheet>,
    );

    expect(screen.queryByText("body")).not.toBeInTheDocument();
    expect(screen.queryByTestId("filter-bottom-sheet-handle")).toBeNull();
  });

  it("mounts the sheet with title, body, footer, and drag handle when opened", () => {
    renderSheet(
      <FilterBottomSheet
        opened
        onClose={vi.fn()}
        title="Filters"
        footer={<button type="button">Apply</button>}
      >
        <p>body</p>
      </FilterBottomSheet>,
    );

    expect(screen.getByText("Filters")).toBeInTheDocument();
    expect(screen.getByText("body")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
    // The drag handle is a native button so it claims focus without
    // explicit tabindex / role wiring.
    const handle = screen.getByTestId("filter-bottom-sheet-handle");
    expect(handle.tagName).toBe("BUTTON");
    expect(handle).toHaveAccessibleName(
      "Drag to resize, swipe down to dismiss",
    );
  });

  it("invokes onClose when the user taps the overlay", () => {
    const onClose = vi.fn();
    renderSheet(
      <FilterBottomSheet opened onClose={onClose} title="Filters" footer={null}>
        <p>body</p>
      </FilterBottomSheet>,
    );

    // The overlay is the sibling of the sheet inside the Portal; reach
    // for it via the `aria-hidden` attribute and click.
    const overlay = document.querySelector(
      '[aria-hidden="true"]',
    ) as HTMLElement | null;
    expect(overlay).not.toBeNull();
    if (!overlay) return;
    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("dismisses on Escape", () => {
    const onClose = vi.fn();
    renderSheet(
      <FilterBottomSheet opened onClose={onClose} title="Filters" footer={null}>
        <p>body</p>
      </FilterBottomSheet>,
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("toggles peek↔full when the handle is clicked", () => {
    renderSheet(
      <FilterBottomSheet opened onClose={vi.fn()} title="Filters" footer={null}>
        <p>body</p>
      </FilterBottomSheet>,
    );

    const handle = screen.getByTestId("filter-bottom-sheet-handle");
    // The sheet starts at the peek snap point.
    const sheet = handle.closest("[data-snap]");
    expect(sheet).toHaveAttribute("data-snap", "peek");

    fireEvent.click(handle);
    expect(sheet).toHaveAttribute("data-snap", "full");

    fireEvent.click(handle);
    expect(sheet).toHaveAttribute("data-snap", "peek");
  });
});
