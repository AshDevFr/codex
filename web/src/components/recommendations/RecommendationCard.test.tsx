import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RecommendationDto } from "@/api/recommendations";
import { renderWithProviders, userEvent } from "@/test/utils";
import { RecommendationCard } from "./RecommendationCard";

// =============================================================================
// Test Data
// =============================================================================

const fullRecommendation: RecommendationDto = {
  externalId: "12345",
  externalUrl: "https://anilist.co/manga/12345",
  title: "Vinland Saga",
  coverUrl: "https://example.com/cover.jpg",
  summary: "A Viking epic about war, revenge, and peace.",
  genres: ["Action", "Historical", "Drama"],
  score: 0.95,
  reason: "Because you rated Berserk 10/10",
  basedOn: ["Berserk", "Vagabond"],
  inLibrary: false,
};

const minimalRecommendation: RecommendationDto = {
  externalId: "99",
  title: "Some Manga",
  score: 0.5,
  reason: "You might like it",
  inLibrary: false,
};

const inLibraryRecommendation: RecommendationDto = {
  externalId: "42",
  title: "Hunter x Hunter",
  score: 0.92,
  reason: "Because you loved One Piece and Naruto",
  basedOn: ["One Piece", "Naruto"],
  inLibrary: true,
};

// =============================================================================
// Tests
// =============================================================================

describe("RecommendationCard", () => {
  const defaultProps = {
    recommendation: fullRecommendation,
    onDismiss: vi.fn(),
  };

  it("renders recommendation title", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(screen.getByText("Vinland Saga")).toBeInTheDocument();
  });

  it("renders score as percentage", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(screen.getByText("95% match")).toBeInTheDocument();
  });

  it("renders reason text", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(
      screen.getByText("Because you rated Berserk 10/10"),
    ).toBeInTheDocument();
  });

  it("renders based on titles", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(screen.getByText("Based on: Berserk, Vagabond")).toBeInTheDocument();
  });

  it("renders summary", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(
      screen.getByText("A Viking epic about war, revenge, and peace."),
    ).toBeInTheDocument();
  });

  it("renders genre badges", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.getByText("Historical")).toBeInTheDocument();
    expect(screen.getByText("Drama")).toBeInTheDocument();
  });

  it("shows Not Interested button for non-library items", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    expect(screen.getByText("Not Interested")).toBeInTheDocument();
  });

  it("calls onDismiss when Not Interested is clicked", async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    renderWithProviders(
      <RecommendationCard
        recommendation={fullRecommendation}
        onDismiss={onDismiss}
      />,
    );

    await user.click(screen.getByText("Not Interested"));
    expect(onDismiss).toHaveBeenCalledWith("12345");
  });

  it("shows In Library badge for library items", () => {
    renderWithProviders(
      <RecommendationCard
        recommendation={inLibraryRecommendation}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText("In Library")).toBeInTheDocument();
  });

  it("hides Not Interested button for library items", () => {
    renderWithProviders(
      <RecommendationCard
        recommendation={inLibraryRecommendation}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.queryByText("Not Interested")).not.toBeInTheDocument();
  });

  it("renders minimal recommendation without errors", () => {
    renderWithProviders(
      <RecommendationCard
        recommendation={minimalRecommendation}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.getByText("Some Manga")).toBeInTheDocument();
    expect(screen.getByText("50% match")).toBeInTheDocument();
    expect(screen.getByText("You might like it")).toBeInTheDocument();
  });

  it("renders external link when URL is provided", () => {
    renderWithProviders(<RecommendationCard {...defaultProps} />);
    const link = screen.getByRole("link");
    expect(link).toHaveAttribute(
      "href",
      "https://anilist.co/manga/12345",
    );
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("does not render based on when empty", () => {
    renderWithProviders(
      <RecommendationCard
        recommendation={minimalRecommendation}
        onDismiss={vi.fn()}
      />,
    );
    expect(screen.queryByText(/Based on:/)).not.toBeInTheDocument();
  });
});
