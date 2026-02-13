import { describe, expect, it } from "vitest";
import { renderWithProviders, screen, userEvent } from "@/test/utils";
import { AlternateTitles } from "./AlternateTitles";

const makeTitles = (labels: string[]) =>
  labels.map((label, i) => ({
    id: `title-${i}`,
    label,
    title: `${label} Title`,
    seriesId: "series-1",
    createdAt: "2024-01-01T00:00:00Z",
    updatedAt: "2024-01-01T00:00:00Z",
  }));

describe("AlternateTitles", () => {
  it("should render nothing when titles array is empty", () => {
    renderWithProviders(<AlternateTitles titles={[]} compact />);
    expect(screen.queryByText(/Title/)).not.toBeInTheDocument();
    expect(screen.queryByText(/more/)).not.toBeInTheDocument();
  });

  it("should render all titles in non-compact mode", () => {
    const titles = makeTitles(["en", "native", "romaji", "Korean"]);
    renderWithProviders(<AlternateTitles titles={titles} />);

    expect(screen.getByText("en Title")).toBeInTheDocument();
    expect(screen.getByText("Korean Title")).toBeInTheDocument();
  });

  it("should show priority labels (en, native, romaji) by default in compact mode", () => {
    const titles = makeTitles(["en", "native", "romaji", "Korean", "French"]);
    renderWithProviders(<AlternateTitles titles={titles} compact />);

    // Priority titles should be visible
    expect(screen.getByText("en Title")).toBeInTheDocument();
    expect(screen.getByText("native Title")).toBeInTheDocument();
    expect(screen.getByText("romaji Title")).toBeInTheDocument();

    // Non-priority titles should be hidden
    expect(screen.queryByText("Korean Title")).not.toBeInTheDocument();
    expect(screen.queryByText("French Title")).not.toBeInTheDocument();

    // Should show "+2 more"
    expect(screen.getByText("+2 more")).toBeInTheDocument();
  });

  it("should be case-insensitive for priority label matching", () => {
    const titles = makeTitles(["English", "Native", "Romaji", "Korean"]);
    renderWithProviders(<AlternateTitles titles={titles} compact />);

    expect(screen.getByText("English Title")).toBeInTheDocument();
    expect(screen.getByText("Native Title")).toBeInTheDocument();
    expect(screen.getByText("Romaji Title")).toBeInTheDocument();
    expect(screen.queryByText("Korean Title")).not.toBeInTheDocument();
    expect(screen.getByText("+1 more")).toBeInTheDocument();
  });

  it("should not show '+X more' when all titles are priority titles", () => {
    const titles = makeTitles(["en", "native"]);
    renderWithProviders(<AlternateTitles titles={titles} compact />);

    expect(screen.getByText("en Title")).toBeInTheDocument();
    expect(screen.getByText("native Title")).toBeInTheDocument();
    expect(screen.queryByText(/more/)).not.toBeInTheDocument();
  });

  it("should expand to show all titles when clicking '+X more'", async () => {
    const user = userEvent.setup();
    const titles = makeTitles(["en", "native", "Korean", "French"]);
    renderWithProviders(<AlternateTitles titles={titles} compact />);

    // Initially collapsed
    expect(screen.queryByText("Korean Title")).not.toBeInTheDocument();
    expect(screen.getByText("+2 more")).toBeInTheDocument();

    // Click to expand
    await user.click(screen.getByText("+2 more"));

    // All titles should now be visible
    expect(screen.getByText("en Title")).toBeInTheDocument();
    expect(screen.getByText("native Title")).toBeInTheDocument();
    expect(screen.getByText("Korean Title")).toBeInTheDocument();
    expect(screen.getByText("French Title")).toBeInTheDocument();

    // "+2 more" gone, "Show less" present
    expect(screen.queryByText("+2 more")).not.toBeInTheDocument();
    expect(screen.getByText("Show less")).toBeInTheDocument();
  });

  it("should collapse back when clicking 'Show less'", async () => {
    const user = userEvent.setup();
    const titles = makeTitles(["en", "Korean"]);
    renderWithProviders(<AlternateTitles titles={titles} compact />);

    // Expand
    await user.click(screen.getByText("+1 more"));
    expect(screen.getByText("Korean Title")).toBeInTheDocument();

    // Collapse
    await user.click(screen.getByText("Show less"));
    expect(screen.queryByText("Korean Title")).not.toBeInTheDocument();
    expect(screen.getByText("+1 more")).toBeInTheDocument();
  });
});
