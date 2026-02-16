import { screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { RecommendationsWidget } from "./RecommendationsWidget";

// =============================================================================
// MSW Setup
// =============================================================================

const server = setupServer();

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

// =============================================================================
// Helpers
// =============================================================================

/** MSW handler that returns an enabled, connected recommendation plugin. */
const pluginsWithRecommendation = http.get("*/user/plugins", () => {
  return HttpResponse.json({
    enabled: [
      {
        id: "plugin-1",
        pluginId: "plugin-1",
        connected: true,
        capabilities: { userRecommendationProvider: true, readSync: false },
      },
    ],
    available: [],
  });
});

/** MSW handler that returns no enabled plugins. */
const pluginsEmpty = http.get("*/user/plugins", () => {
  return HttpResponse.json({ enabled: [], available: [] });
});

// =============================================================================
// Tests
// =============================================================================

describe("RecommendationsWidget", () => {
  it("renders nothing when no recommendations", async () => {
    server.use(
      pluginsWithRecommendation,
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    const { container } = renderWithProviders(<RecommendationsWidget />);

    // Wait for query to resolve, then verify nothing rendered
    await waitFor(() => {
      expect(
        container.querySelector("[data-testid='recommendation-compact-card']"),
      ).not.toBeInTheDocument();
    });
    expect(screen.queryByText("Recommended For You")).not.toBeInTheDocument();
  });

  it("renders nothing when no plugin is enabled", async () => {
    server.use(pluginsEmpty);

    const { container } = renderWithProviders(<RecommendationsWidget />);

    // Wait a tick for the query to settle
    await new Promise((r) => setTimeout(r, 100));
    expect(
      container.querySelector("[data-testid='recommendation-compact-card']"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Recommended For You")).not.toBeInTheDocument();
  });

  it("renders carousel with recommendations", async () => {
    server.use(
      pluginsWithRecommendation,
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [
            {
              externalId: "1",
              title: "Vinland Saga",
              score: 0.95,
              reason: "Because you rated Berserk 10/10",
              inLibrary: false,
            },
            {
              externalId: "2",
              title: "Monster",
              score: 0.88,
              reason: "Based on your interest in thrillers",
              inLibrary: true,
            },
          ],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<RecommendationsWidget />);

    await waitFor(() => {
      expect(screen.getByText("Recommended For You")).toBeInTheDocument();
    });
    expect(screen.getByText("Vinland Saga")).toBeInTheDocument();
    expect(screen.getByText("Monster")).toBeInTheDocument();
  });

  it("shows plugin name as subtitle", async () => {
    server.use(
      pluginsWithRecommendation,
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [
            {
              externalId: "1",
              title: "Vinland Saga",
              score: 0.95,
              reason: "Great manga",
              inLibrary: false,
            },
          ],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<RecommendationsWidget />);

    await waitFor(() => {
      expect(screen.getByText("Powered by AniList Recs")).toBeInTheDocument();
    });
  });
});
