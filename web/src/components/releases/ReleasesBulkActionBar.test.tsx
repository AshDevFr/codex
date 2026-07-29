import { describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { RELEASE_ACTION_DESCRIPTIONS } from "./actionDescriptions";
import { ReleasesBulkActionBar } from "./ReleasesBulkActionBar";

function setup(
  overrides: Partial<Parameters<typeof ReleasesBulkActionBar>[0]>,
) {
  const props = {
    count: 3,
    isPending: false,
    onAction: vi.fn(),
    onClear: vi.fn(),
    ...overrides,
  };
  renderWithProviders(<ReleasesBulkActionBar {...props} />);
  return props;
}

describe("ReleasesBulkActionBar", () => {
  it("dispatches the matching bulk action for each button", async () => {
    const user = userEvent.setup();
    const props = setup({});

    await user.click(screen.getByRole("button", { name: "Mark acquired" }));
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    await user.click(screen.getByRole("button", { name: "Ignore" }));
    await user.click(screen.getByRole("button", { name: "Reset" }));

    expect(props.onAction.mock.calls.map(([action]) => action)).toEqual([
      "mark-acquired",
      "dismiss",
      "ignore",
      "reset",
    ]);
  });

  it("routes Delete through the caller's confirm handler, not onAction", async () => {
    const user = userEvent.setup();
    const onDeleteClick = vi.fn();
    const props = setup({ onDeleteClick });

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onDeleteClick).toHaveBeenCalledOnce();
    expect(props.onAction).not.toHaveBeenCalled();
  });

  it("hides Delete when no confirm handler is wired up", () => {
    setup({});
    expect(screen.queryByRole("button", { name: "Delete" })).toBeNull();
  });

  // The four state actions look interchangeable from their labels alone, so
  // the tooltip copy is the only thing telling a user what each one does to
  // the ledger row. Assert it actually surfaces.
  it.each([
    ["Mark acquired", RELEASE_ACTION_DESCRIPTIONS.markAcquired],
    ["Dismiss", RELEASE_ACTION_DESCRIPTIONS.dismiss],
    ["Ignore", RELEASE_ACTION_DESCRIPTIONS.ignore],
    ["Reset", RELEASE_ACTION_DESCRIPTIONS.reset],
  ])("explains %s on hover", async (name, description) => {
    const user = userEvent.setup();
    setup({});

    await user.hover(screen.getByRole("button", { name }));

    expect(await screen.findByText(description)).toBeInTheDocument();
  });

  it("warns that Delete is undone by the next poll", async () => {
    const user = userEvent.setup();
    setup({ onDeleteClick: vi.fn() });

    await user.hover(screen.getByRole("button", { name: "Delete" }));

    expect(
      await screen.findByText(RELEASE_ACTION_DESCRIPTIONS.delete),
    ).toBeInTheDocument();
  });

  // The hover tests above compare against the constants, so they'd still pass
  // if someone quietly dropped the permanence wording. These assert the claim
  // itself: a user reading the tooltip must learn that the action sticks
  // across future polls and that Reset is the way out.
  describe("permanence copy", () => {
    it.each([
      ["markAcquired", RELEASE_ACTION_DESCRIPTIONS.markAcquired],
      ["dismiss", RELEASE_ACTION_DESCRIPTIONS.dismiss],
      ["ignore", RELEASE_ACTION_DESCRIPTIONS.ignore],
    ])("%s says it is permanent and points at Reset", (_name, description) => {
      expect(description).toMatch(/permanent/i);
      expect(description).toMatch(/next poll/i);
      expect(description).toMatch(/Reset/);
    });

    it("delete disclaims permanence instead of claiming it", () => {
      const { delete: deleteCopy } = RELEASE_ACTION_DESCRIPTIONS;
      expect(deleteCopy).toMatch(/not permanent/i);
      expect(deleteCopy).toMatch(/returns on the next poll/i);
    });

    it("reset is described as the undo", () => {
      expect(RELEASE_ACTION_DESCRIPTIONS.reset).toMatch(/undo/i);
    });
  });

  it("clears the selection without touching any release", async () => {
    const user = userEvent.setup();
    const props = setup({});

    await user.click(screen.getByRole("button", { name: "Clear" }));

    expect(props.onClear).toHaveBeenCalledOnce();
    expect(props.onAction).not.toHaveBeenCalled();
  });
});
