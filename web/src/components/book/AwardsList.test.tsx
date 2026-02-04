import { describe, expect, it } from "vitest";
import { renderWithProviders, screen } from "@/test/utils";
import type { BookAward } from "@/types/book-metadata";
import { AwardsCount, AwardsList } from "./AwardsList";

describe("AwardsList", () => {
  const mockAwards: BookAward[] = [
    { name: "Hugo Award", year: 2020, category: "Best Novel", won: true },
    { name: "Nebula Award", year: 2020, category: "Best Novel", won: false },
    { name: "World Fantasy Award", year: 2019, won: true },
  ];

  it("renders nothing when awards is null", () => {
    renderWithProviders(<AwardsList awards={null} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when awards is undefined", () => {
    renderWithProviders(<AwardsList awards={undefined} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders nothing when awards array is empty", () => {
    renderWithProviders(<AwardsList awards={[]} />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("renders awards from array", () => {
    renderWithProviders(<AwardsList awards={mockAwards} />);
    expect(screen.getByText("Hugo Award (2020)")).toBeInTheDocument();
    expect(screen.getByText("Nebula Award (2020)")).toBeInTheDocument();
    expect(screen.getByText("World Fantasy Award (2019)")).toBeInTheDocument();
  });

  it("parses and renders awards from JSON string", () => {
    const json = JSON.stringify(mockAwards);
    renderWithProviders(<AwardsList awards={json} />);
    expect(screen.getByText("Hugo Award (2020)")).toBeInTheDocument();
  });

  it("filters to won awards only when wonOnly is true", () => {
    renderWithProviders(<AwardsList awards={mockAwards} wonOnly />);
    expect(screen.getByText("Hugo Award (2020)")).toBeInTheDocument();
    expect(screen.getByText("World Fantasy Award (2019)")).toBeInTheDocument();
    expect(screen.queryByText("Nebula Award (2020)")).not.toBeInTheDocument();
  });

  it("limits displayed awards when maxDisplay is set", () => {
    renderWithProviders(<AwardsList awards={mockAwards} maxDisplay={2} />);
    expect(screen.getByText("+1 more")).toBeInTheDocument();
  });

  it("sorts awards with won awards first", () => {
    const awards: BookAward[] = [
      { name: "Award A", won: false },
      { name: "Award B", won: true },
    ];
    renderWithProviders(<AwardsList awards={awards} />);
    const badges = screen
      .getAllByRole("generic")
      .filter((el) => el.classList.contains("mantine-Badge-root"));
    // Won awards should come first
    expect(badges[0]).toHaveTextContent("Award B");
  });

  it("renders award without year correctly", () => {
    const awards: BookAward[] = [{ name: "Some Award", won: true }];
    renderWithProviders(<AwardsList awards={awards} />);
    expect(screen.getByText("Some Award")).toBeInTheDocument();
  });

  it("handles invalid JSON gracefully", () => {
    renderWithProviders(<AwardsList awards="invalid json" />);
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });
});

describe("AwardsCount", () => {
  const mockAwards: BookAward[] = [
    { name: "Award 1", won: true },
    { name: "Award 2", won: true },
    { name: "Award 3", won: false },
  ];

  it("renders nothing when awards is null", () => {
    renderWithProviders(<AwardsCount awards={null} />);
    expect(screen.queryByText(/\d/)).not.toBeInTheDocument();
  });

  it("renders nothing when awards is empty", () => {
    renderWithProviders(<AwardsCount awards={[]} />);
    expect(screen.queryByText(/\d/)).not.toBeInTheDocument();
  });

  it("displays total count", () => {
    renderWithProviders(<AwardsCount awards={mockAwards} />);
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("parses JSON string input", () => {
    const json = JSON.stringify(mockAwards);
    renderWithProviders(<AwardsCount awards={json} />);
    expect(screen.getByText("3")).toBeInTheDocument();
  });
});
