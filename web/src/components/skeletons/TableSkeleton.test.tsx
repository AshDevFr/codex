import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import { CardListSkeleton, TableSkeleton } from "./TableSkeleton";

function setMatchMedia(matches: (query: string) => boolean) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: matches(query),
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

const forceMobile = () =>
  setMatchMedia((query) => query.includes("max-width: 30.0625em"));
const forceDesktop = () => setMatchMedia(() => false);

beforeEach(() => {
  forceDesktop();
});

describe("TableSkeleton", () => {
  it("renders a desktop table with the requested row + column count", () => {
    renderWithProviders(<TableSkeleton rows={3} columns={5} />);
    const wrapper = screen.getByTestId("table-skeleton");
    const rows = wrapper.querySelectorAll("tbody tr");
    expect(rows.length).toBe(3);
    // One <th> per requested column.
    const headers = wrapper.querySelectorAll("thead th");
    expect(headers.length).toBe(5);
  });

  it("switches to a stacked card layout below the mobile breakpoint", () => {
    forceMobile();
    renderWithProviders(<TableSkeleton rows={4} columns={3} />);
    const wrapper = screen.getByTestId("table-skeleton");
    // Mobile layout has no <table>.
    expect(wrapper.querySelector("table")).toBeNull();
    // One card per row.
    const cards = wrapper.querySelectorAll(".mantine-Card-root");
    expect(cards.length).toBe(4);
  });

  it("uses column labels when provided so desktop headers read like the real table", () => {
    renderWithProviders(
      <TableSkeleton rows={1} columnLabels={["Name", "Last Login", "Role"]} />,
    );
    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByText("Role")).toBeInTheDocument();
  });
});

describe("CardListSkeleton", () => {
  it("renders the requested number of cards", () => {
    renderWithProviders(<CardListSkeleton count={4} />);
    const wrapper = screen.getByTestId("card-list-skeleton");
    expect(wrapper.querySelectorAll(".mantine-Card-root").length).toBe(4);
  });
});
