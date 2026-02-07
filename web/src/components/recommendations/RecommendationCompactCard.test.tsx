import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { RecommendationDto } from "@/api/recommendations";
import { renderWithProviders } from "@/test/utils";
import { RecommendationCompactCard } from "./RecommendationCompactCard";

const baseRec: RecommendationDto = {
  externalId: "1",
  title: "Vinland Saga",
  score: 0.95,
  reason: "Because you rated Berserk 10/10",
  inLibrary: false,
};

describe("RecommendationCompactCard", () => {
  it("renders title", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    expect(screen.getByText("Vinland Saga")).toBeInTheDocument();
  });

  it("renders score as percentage", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    expect(screen.getByText("95%")).toBeInTheDocument();
  });

  it("renders reason text", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    expect(
      screen.getByText("Because you rated Berserk 10/10"),
    ).toBeInTheDocument();
  });

  it("shows in-library badge when in library", () => {
    renderWithProviders(
      <RecommendationCompactCard
        recommendation={{ ...baseRec, inLibrary: true }}
      />,
    );
    expect(screen.getByText("Owned")).toBeInTheDocument();
  });

  it("does not show in-library badge when not in library", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    expect(screen.queryByText("Owned")).not.toBeInTheDocument();
  });

  it("renders cover image when coverUrl provided", () => {
    renderWithProviders(
      <RecommendationCompactCard
        recommendation={{
          ...baseRec,
          coverUrl: "https://example.com/cover.jpg",
        }}
      />,
    );
    expect(screen.getByAltText("Vinland Saga")).toBeInTheDocument();
  });

  it("renders no-cover placeholder when coverUrl is missing", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    expect(screen.getByText("No Cover")).toBeInTheDocument();
  });

  it("wraps in a link when externalUrl is provided", () => {
    renderWithProviders(
      <RecommendationCompactCard
        recommendation={{
          ...baseRec,
          externalUrl: "https://anilist.co/manga/1",
        }}
      />,
    );
    const link = screen.getByTestId("recommendation-compact-card");
    expect(link.tagName).toBe("A");
    expect(link).toHaveAttribute("href", "https://anilist.co/manga/1");
    expect(link).toHaveAttribute("target", "_blank");
  });

  it("renders as div when no externalUrl", () => {
    renderWithProviders(<RecommendationCompactCard recommendation={baseRec} />);
    const card = screen.getByTestId("recommendation-compact-card");
    expect(card.tagName).toBe("DIV");
  });

  it("rounds score correctly", () => {
    renderWithProviders(
      <RecommendationCompactCard
        recommendation={{ ...baseRec, score: 0.876 }}
      />,
    );
    expect(screen.getByText("88%")).toBeInTheDocument();
  });
});
