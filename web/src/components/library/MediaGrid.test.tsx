import { describe, expect, it, vi } from "vitest";
import { createSeries } from "@/mocks/data/factories";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { MediaGrid, type MediaGridItem } from "./MediaGrid";

// The grid's contract is layout + affordances; the card itself has its own
// suite. A stub keeps this file free of MediaCard's API mocks.
vi.mock("@/components/library/MediaCard", () => ({
  MediaCard: ({ data }: { data: { title?: string; name?: string } }) => (
    <div data-testid="media-card">{data.title ?? data.name}</div>
  ),
}));

function makeItems(titles: string[]): MediaGridItem[] {
  return titles.map((title, i) => ({
    id: `id-${i}`,
    type: "series",
    data: createSeries({ id: `id-${i}`, title }),
  }));
}

describe("MediaGrid", () => {
  it("renders a card per item in order", () => {
    renderWithProviders(<MediaGrid items={makeItems(["Alpha", "Bravo"])} />);

    const cards = screen.getAllByTestId("media-card");
    expect(cards).toHaveLength(2);
    expect(cards[0]).toHaveTextContent("Alpha");
    expect(cards[1]).toHaveTextContent("Bravo");
  });

  it("renders a skeleton for items whose payload is still loading", () => {
    const items: MediaGridItem[] = [
      { id: "pending", type: "series" },
      ...makeItems(["Alpha"]),
    ];
    renderWithProviders(<MediaGrid items={items} />);

    expect(screen.getAllByTestId("media-card")).toHaveLength(1);
  });

  it("shows no remove or reorder controls by default", () => {
    renderWithProviders(<MediaGrid items={makeItems(["Alpha"])} />);

    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Move up" })).toBeNull();
  });

  it("calls onRemove with the clicked item", async () => {
    const user = userEvent.setup();
    const onRemove = vi.fn();
    const items = makeItems(["Alpha", "Bravo"]);

    renderWithProviders(
      <MediaGrid items={items} onRemove={onRemove} removeLabel="Remove it" />,
    );

    const buttons = screen.getAllByRole("button", { name: "Remove it" });
    await user.click(buttons[1]);

    expect(onRemove).toHaveBeenCalledWith(items[1]);
  });

  it("reorders optimistically via the chevron fallback", async () => {
    const user = userEvent.setup();
    const onReorder = vi.fn();

    renderWithProviders(
      <MediaGrid
        items={makeItems(["Alpha", "Bravo", "Charlie"])}
        reorderable
        onReorder={onReorder}
      />,
    );

    // Move the first item down one slot.
    const downButtons = screen.getAllByRole("button", { name: "Move down" });
    await user.click(downButtons[0]);

    expect(onReorder).toHaveBeenCalledWith(["id-1", "id-0", "id-2"]);
    // Optimistic: the visible order flips before the server confirms.
    const cards = screen.getAllByTestId("media-card");
    expect(cards[0]).toHaveTextContent("Bravo");
    expect(cards[1]).toHaveTextContent("Alpha");
  });

  it("disables the edge chevrons", () => {
    renderWithProviders(
      <MediaGrid
        items={makeItems(["Alpha", "Bravo"])}
        reorderable
        onReorder={vi.fn()}
      />,
    );

    const upButtons = screen.getAllByRole("button", { name: "Move up" });
    const downButtons = screen.getAllByRole("button", { name: "Move down" });
    expect(upButtons[0]).toBeDisabled();
    expect(downButtons[downButtons.length - 1]).toBeDisabled();
  });
});
