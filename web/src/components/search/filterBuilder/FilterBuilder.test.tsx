import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import type { SeriesCondition } from "@/types/filters";
import { FilterBuilder } from "./FilterBuilder";

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
});
