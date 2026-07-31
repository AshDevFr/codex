import { afterEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import { FilterBuilder } from "./FilterBuilder";

// Helper to drive `useMediaQuery` (which reads `window.matchMedia`) toward the
// mobile breakpoint used by the leaf editor's stacked layout.
function setViewportMatchesMobile(isMobile: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: isMobile && query.includes("768px"),
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

describe("FilterBuilder", () => {
  it("renders an empty state for a fresh builder", () => {
    renderWithProviders(
      <FilterBuilder
        condition={undefined}
        target="series"
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByText(/no filters yet/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /add filter/i }),
    ).toBeInTheDocument();
  });

  it("emits a new leaf when Add filter is clicked", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <FilterBuilder
        condition={undefined}
        target="series"
        onChange={onChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: /add filter/i }));
    expect(onChange).toHaveBeenCalledTimes(1);
    const next = onChange.mock.calls[0]![0] as SeriesCondition;
    expect(next).toHaveProperty("allOf");
    expect((next as { allOf: unknown[] }).allOf).toHaveLength(1);
  });

  it("toggles the root combinator between allOf and anyOf", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <FilterBuilder
        condition={{
          allOf: [
            { title: { operator: "contains", value: "punch" } },
            { title: { operator: "contains", value: "saitama" } },
          ],
        }}
        target="series"
        onChange={onChange}
      />,
    );

    await user.click(screen.getByText("Any of"));
    expect(onChange).toHaveBeenCalled();
    const next = onChange.mock.calls.at(-1)![0] as SeriesCondition;
    expect(next).toHaveProperty("anyOf");
  });

  it("removes a leaf when the trash button is clicked", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(
      <FilterBuilder
        condition={{
          allOf: [{ title: { operator: "contains", value: "punch" } }],
        }}
        target="series"
        onChange={onChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: /remove filter/i }));
    expect(onChange).toHaveBeenCalledWith(undefined);
  });

  it("renders an existing nested anyOf group", () => {
    renderWithProviders(
      <FilterBuilder
        condition={{
          allOf: [
            {
              anyOf: [
                { tag: { operator: "is", value: "manga" } },
                { tag: { operator: "is", value: "comic" } },
              ],
            },
          ],
        }}
        target="series"
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByText(/match any/i)).toBeInTheDocument();
  });

  describe("reordering", () => {
    it("renders a reorder grip for every sibling row", () => {
      renderWithProviders(
        <FilterBuilder
          condition={{
            allOf: [
              { title: { operator: "contains", value: "punch" } },
              { title: { operator: "contains", value: "saitama" } },
              { genre: { operator: "is", value: "action" } },
            ],
          }}
          target="series"
          onChange={vi.fn()}
        />,
      );

      expect(
        screen.getAllByRole("button", { name: /reorder filter/i }),
      ).toHaveLength(3);
    });

    it("labels a nested group's grip as a group", () => {
      renderWithProviders(
        <FilterBuilder
          condition={{
            allOf: [
              { title: { operator: "contains", value: "punch" } },
              {
                anyOf: [
                  { tag: { operator: "is", value: "manga" } },
                  { tag: { operator: "is", value: "comic" } },
                ],
              },
            ],
          }}
          target="series"
          onChange={vi.fn()}
        />,
      );

      expect(
        screen.getByRole("button", { name: /reorder group/i }),
      ).toBeInTheDocument();
      // The two leaves inside the group plus the one at the root.
      expect(
        screen.getAllByRole("button", { name: /reorder filter/i }),
      ).toHaveLength(3);
    });

    it("hides the grip when a group has a single row", () => {
      renderWithProviders(
        <FilterBuilder
          condition={{
            allOf: [{ title: { operator: "contains", value: "punch" } }],
          }}
          target="series"
          onChange={vi.fn()}
        />,
      );

      // Nothing to reorder, so the grip is hidden, which also takes it out of
      // the accessibility tree and the tab order.
      expect(
        screen.queryByRole("button", { name: /reorder filter/i }),
      ).not.toBeInTheDocument();

      // The gutter still occupies its column so a one-row group lines up with
      // multi-row ones. Queried by attribute because a `visibility: hidden`
      // element has no computed accessible name to match on.
      const grips = document.querySelectorAll<HTMLElement>(
        '[aria-label="Reorder filter"]',
      );
      expect(grips).toHaveLength(1);
      expect(grips[0].style.visibility).toBe("hidden");
    });
  });

  describe("responsive leaf layout", () => {
    afterEach(() => {
      // Reset to the desktop default so the override doesn't leak between tests.
      setViewportMatchesMobile(false);
    });

    it("stretches the value input to full width on mobile", () => {
      setViewportMatchesMobile(true);
      renderWithProviders(
        <FilterBuilder
          condition={{
            allOf: [{ title: { operator: "contains", value: "" } }],
          }}
          target="series"
          onChange={vi.fn()}
        />,
      );

      const root = screen
        .getByPlaceholderText("value")
        .closest(".mantine-TextInput-root") as HTMLElement;
      expect(root).not.toBeNull();
      // Mobile stacks the controls, so the value input owns the whole row
      // instead of being crushed to a few pixels beside the fixed-width selects.
      expect(root.style.width).toBe("100%");
    });

    it("keeps the value input inline (flex) on desktop", () => {
      setViewportMatchesMobile(false);
      renderWithProviders(
        <FilterBuilder
          condition={{
            allOf: [{ title: { operator: "contains", value: "" } }],
          }}
          target="series"
          onChange={vi.fn()}
        />,
      );

      const root = screen
        .getByPlaceholderText("value")
        .closest(".mantine-TextInput-root") as HTMLElement;
      expect(root).not.toBeNull();
      expect(root.style.width).toBe("");
      expect(root.style.flex).not.toBe("");
    });
  });
});
