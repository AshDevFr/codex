import { screen, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { renderWithProviders } from "@/test/utils";
import { Recommendations } from "./Recommendations";

// =============================================================================
// MSW Setup
// =============================================================================

const server = setupServer(
  http.get("/api/v1/settings/branding", () => {
    return HttpResponse.json({ applicationName: "Codex" });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

// =============================================================================
// Tests
// =============================================================================

describe("Recommendations", () => {
  it("renders page title", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(screen.getByText("Recommendations")).toBeInTheDocument();
    });
  });

  it("shows loading state", () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return new Promise(() => {}); // Never resolves
      }),
    );

    renderWithProviders(<Recommendations />);
    expect(screen.getByText("Loading recommendations...")).toBeInTheDocument();
  });

  it("shows empty state when no recommendations", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(screen.getByText("No recommendations yet")).toBeInTheDocument();
    });
  });

  it("shows no-plugin message on 404", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json(
          { error: "No recommendation plugin enabled" },
          { status: 404 },
        );
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(
        screen.getByText("No recommendation plugin enabled"),
      ).toBeInTheDocument();
    });
  });

  it("renders recommendations when available", async () => {
    server.use(
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
          generatedAt: "2026-02-06T12:00:00Z",
          cached: false,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(screen.getByText("Vinland Saga")).toBeInTheDocument();
      expect(screen.getByText("Monster")).toBeInTheDocument();
    });
  });

  it("shows plugin name", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(screen.getByText(/Powered by AniList Recs/)).toBeInTheDocument();
    });
  });

  it("shows cached indicator", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: true,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(screen.getByText("(cached)")).toBeInTheDocument();
    });
  });

  it("has a Refresh button", async () => {
    server.use(
      http.get("*/user/recommendations", () => {
        return HttpResponse.json({
          recommendations: [],
          pluginId: "plugin-1",
          pluginName: "AniList Recs",
          cached: false,
        });
      }),
    );

    renderWithProviders(<Recommendations />);
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /Refresh/ }),
      ).toBeInTheDocument();
    });
  });
});
