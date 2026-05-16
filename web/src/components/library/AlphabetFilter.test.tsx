import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { type AlphabetCounts, AlphabetFilter } from "./AlphabetFilter";

function forceMobileViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: query.includes("max-width"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

function resetViewport() {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

describe("AlphabetFilter", () => {
  it("renders all letters including ALL and #", () => {
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

    expect(screen.getByRole("button", { name: "ALL" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "#" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "A" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Z" })).toBeInTheDocument();
  });

  it("highlights ALL when selected is null", () => {
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

    const allButton = screen.getByRole("button", { name: "ALL" });
    expect(allButton).toHaveAttribute("data-selected");
  });

  it("highlights selected letter", () => {
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected="B" onSelect={onSelect} />);

    const bButton = screen.getByRole("button", { name: "B" });
    expect(bButton).toHaveAttribute("data-selected");

    const allButton = screen.getByRole("button", { name: "ALL" });
    expect(allButton).not.toHaveAttribute("data-selected");
  });

  it("calls onSelect with letter when clicking a letter", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

    await user.click(screen.getByRole("button", { name: "C" }));
    expect(onSelect).toHaveBeenCalledWith("C");
  });

  it("calls onSelect with null when clicking ALL", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected="C" onSelect={onSelect} />);

    await user.click(screen.getByRole("button", { name: "ALL" }));
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("calls onSelect with null when clicking already selected letter (toggle off)", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected="D" onSelect={onSelect} />);

    await user.click(screen.getByRole("button", { name: "D" }));
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("calls onSelect with # for number filter", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

    await user.click(screen.getByRole("button", { name: "#" }));
    expect(onSelect).toHaveBeenCalledWith("#");
  });

  it("disables letters with no count when counts are provided", () => {
    const onSelect = vi.fn();
    const counts: AlphabetCounts = new Map([
      ["a", 5],
      ["b", 10],
      // "c" has no count, should be disabled
    ]);

    renderWithProviders(
      <AlphabetFilter
        selected={null}
        onSelect={onSelect}
        counts={counts}
        totalCount={15}
      />,
    );

    // C button should be disabled (no count)
    const cButton = screen.getByRole("button", { name: "C" });
    expect(cButton).toBeDisabled();

    // A button should be enabled (has count)
    const aButton = screen.getByRole("button", { name: "A" });
    expect(aButton).not.toBeDisabled();
  });

  it("aggregates numeric counts for # button", () => {
    const onSelect = vi.fn();
    const counts: AlphabetCounts = new Map([
      ["1", 3],
      ["2", 2],
      ["a", 5],
    ]);

    renderWithProviders(
      <AlphabetFilter
        selected={null}
        onSelect={onSelect}
        counts={counts}
        totalCount={10}
      />,
    );

    // # button should be enabled (sum of numeric counts = 5)
    const hashButton = screen.getByRole("button", { name: "#" });
    expect(hashButton).not.toBeDisabled();
  });
});

describe("AlphabetFilter - mobile", () => {
  beforeEach(() => {
    forceMobileViewport();
  });

  afterEach(() => {
    resetViewport();
  });

  it("renders a Select picker instead of the letter strip", () => {
    renderWithProviders(<AlphabetFilter selected={null} onSelect={vi.fn()} />);

    expect(
      screen.getByRole("textbox", { name: "Jump to letter" }),
    ).toBeInTheDocument();
    // No A-Z buttons should be present below xs
    expect(screen.queryByRole("button", { name: "A" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Z" })).not.toBeInTheDocument();
  });

  it("shows the selected letter in the picker value", () => {
    renderWithProviders(<AlphabetFilter selected="M" onSelect={vi.fn()} />);

    const input = screen.getByRole("textbox", {
      name: "Jump to letter",
    }) as HTMLInputElement;
    expect(input.value).toBe("M");
  });

  it("shows All series when no letter is selected", () => {
    renderWithProviders(<AlphabetFilter selected={null} onSelect={vi.fn()} />);

    const input = screen.getByRole("textbox", {
      name: "Jump to letter",
    }) as HTMLInputElement;
    expect(input.value).toBe("All series");
  });

  it("calls onSelect with the chosen letter when picking from the dropdown", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected={null} onSelect={onSelect} />);

    const input = screen.getByRole("textbox", { name: "Jump to letter" });
    await user.click(input);
    // Mantine 8 Combobox renders the dropdown into a portal whose container
    // keeps `display: none` until the open transition fires; testing-library's
    // default queries filter that out. Querying with `hidden: true` finds
    // options regardless of the portal's display state.
    const gOption = await screen.findByRole("option", {
      name: "G",
      hidden: true,
    });
    await user.click(gOption);

    expect(onSelect).toHaveBeenCalledWith("G");
  });

  it("calls onSelect with null when picking All series", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    renderWithProviders(<AlphabetFilter selected="C" onSelect={onSelect} />);

    const input = screen.getByRole("textbox", { name: "Jump to letter" });
    await user.click(input);
    const allOption = await screen.findByRole("option", {
      name: "All series",
      hidden: true,
    });
    await user.click(allOption);

    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("shows counts in option labels when provided", async () => {
    const user = userEvent.setup();
    const counts: AlphabetCounts = new Map([
      ["a", 5],
      ["b", 10],
    ]);
    renderWithProviders(
      <AlphabetFilter
        selected={null}
        onSelect={vi.fn()}
        counts={counts}
        totalCount={15}
      />,
    );

    const input = screen.getByRole("textbox", { name: "Jump to letter" });
    await user.click(input);

    expect(
      await screen.findByRole("option", {
        name: "All series (15)",
        hidden: true,
      }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "A (5)", hidden: true }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("option", { name: "B (10)", hidden: true }),
    ).toBeInTheDocument();
  });

  it("disables letters with no count in the dropdown", async () => {
    const user = userEvent.setup();
    const counts: AlphabetCounts = new Map([["a", 5]]);
    renderWithProviders(
      <AlphabetFilter
        selected={null}
        onSelect={vi.fn()}
        counts={counts}
        totalCount={5}
      />,
    );

    const input = screen.getByRole("textbox", { name: "Jump to letter" });
    await user.click(input);

    const cOption = await screen.findByRole("option", {
      name: "C",
      hidden: true,
    });
    expect(cOption).toHaveAttribute("data-combobox-disabled", "true");
  });
});
