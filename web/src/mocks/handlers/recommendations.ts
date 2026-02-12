/**
 * Recommendations API mock handlers
 *
 * Provides mock data for:
 * - GET /api/v1/user/recommendations (list recommendations)
 * - POST /api/v1/user/recommendations/refresh (trigger refresh)
 * - POST /api/v1/user/recommendations/:external_id/dismiss (dismiss)
 */

import { delay, HttpResponse, http } from "msw";
import type { components } from "@/types/api.generated";
import { mockSeries } from "../data/store";

type RecommendationDto = components["schemas"]["RecommendationDto"];
type RecommendationsResponse = components["schemas"]["RecommendationsResponse"];

/** Generate a placeholder cover URL with the title baked in */
const cover = (title: string) =>
  `https://placehold.co/150x212/2a2a3a/eee?text=${encodeURIComponent(title)}`;

// Find some real series from the mock store to use as "in library" recommendations
const onePiece = mockSeries.find((s) => s.title === "One Piece");
const attackOnTitan = mockSeries.find((s) => s.title === "Attack on Titan");

let mockRecommendations: RecommendationDto[] = [
  {
    externalId: "anilist-21",
    externalUrl: "https://anilist.co/manga/30013",
    title: "One Piece",
    coverUrl: cover("One Piece"),
    summary:
      "Gol D. Roger, a man referred to as the King of the Pirates, is set to be executed by the World Government. But just before his death, he confirms the existence of a great treasure, One Piece.",
    genres: ["Action", "Adventure", "Comedy", "Fantasy"],
    score: 0.97,
    reason: "Because you rated Berserk and Vagabond highly",
    basedOn: ["Berserk", "Vagabond"],
    codexSeriesId: onePiece?.id ?? null,
    inLibrary: !!onePiece,
    inCodex: !!onePiece,
    status: "ongoing",
    totalBookCount: 110,
    rating: 88,
    popularity: 234000,
  },
  {
    externalId: "anilist-16498",
    externalUrl: "https://anilist.co/manga/53390",
    title: "Attack on Titan",
    coverUrl: cover("Attack on\nTitan"),
    summary:
      "In a world where humanity lives inside cities surrounded by enormous walls due to the Titans, gigantic humanoid creatures who devour humans seemingly without reason.",
    genres: ["Action", "Drama", "Fantasy", "Mystery"],
    score: 0.93,
    reason: "Popular with readers who enjoy dark fantasy epics",
    basedOn: ["Berserk", "Claymore"],
    codexSeriesId: attackOnTitan?.id ?? null,
    inLibrary: !!attackOnTitan,
    inCodex: !!attackOnTitan,
    status: "ended",
    totalBookCount: 34,
    rating: 84,
    popularity: 302000,
  },
  {
    externalId: "anilist-30002",
    externalUrl: "https://anilist.co/manga/30002",
    title: "Berserk",
    coverUrl: cover("Berserk"),
    summary:
      "Guts, a former mercenary now known as the Black Swordsman, is out for revenge. After a tumultuous childhood, he finally finds someone he respects and believes he can trust.",
    genres: ["Action", "Adventure", "Drama", "Fantasy", "Horror"],
    score: 0.91,
    reason: "Highly rated dark fantasy with masterful artwork",
    basedOn: ["Vagabond", "Vinland Saga"],
    inLibrary: false,
    inCodex: false,
    status: "hiatus",
    totalBookCount: 42,
    rating: 93,
    popularity: 185000,
  },
  {
    externalId: "anilist-36531",
    externalUrl: "https://anilist.co/manga/36531",
    title: "Vinland Saga",
    coverUrl: cover("Vinland\nSaga"),
    summary:
      "Thorfinn is son to one of the Vikings' greatest warriors, but when his father is killed in battle by the mercenary leader Askeladd, he swears to have his revenge.",
    genres: ["Action", "Adventure", "Drama"],
    score: 0.89,
    reason: "Fans of historical action manga love this series",
    basedOn: ["Berserk", "Vagabond", "Kingdom"],
    inLibrary: false,
    inCodex: false,
    status: "ended",
    totalBookCount: 27,
    rating: 89,
    popularity: 95000,
  },
  {
    externalId: "anilist-30656",
    externalUrl: "https://anilist.co/manga/30656",
    title: "Vagabond",
    coverUrl: cover("Vagabond"),
    summary:
      "Growing up in the late 1500s Sengoku era Japan, Shinmen Takezo is shunned by the local villagers as a devil child due to his wild and violent nature.",
    genres: ["Action", "Adventure", "Drama"],
    score: 0.87,
    reason: "A beautifully drawn historical masterpiece",
    basedOn: ["Berserk", "Blade of the Immortal"],
    inLibrary: false,
    inCodex: false,
    status: "hiatus",
    totalBookCount: 37,
    rating: 90,
    popularity: 112000,
  },
  {
    externalId: "anilist-30026",
    externalUrl: "https://anilist.co/manga/30026",
    title: "Fullmetal Alchemist",
    coverUrl: cover("Fullmetal\nAlchemist"),
    summary:
      "Two brothers search for the Philosopher's Stone after an attempt to use alchemy to bring their mother back from the dead, which cost one of them his body and the other a leg and an arm.",
    genres: ["Action", "Adventure", "Comedy", "Drama", "Fantasy"],
    score: 0.85,
    reason: "A perfectly crafted adventure with deep themes",
    basedOn: ["Hunter x Hunter", "My Hero Academia"],
    codexSeriesId:
      mockSeries.find((s) => s.title === "Fullmetal Alchemist")?.id ?? null,
    inLibrary: !!mockSeries.find((s) => s.title === "Fullmetal Alchemist"),
    inCodex: !!mockSeries.find((s) => s.title === "Fullmetal Alchemist"),
    status: "ended",
    totalBookCount: 27,
    rating: 88,
    popularity: 198000,
  },
  {
    externalId: "anilist-85486",
    externalUrl: "https://anilist.co/manga/85486",
    title: "Chainsaw Man",
    coverUrl: cover("Chainsaw\nMan"),
    summary:
      "Denji is a teenage boy living with a Chainsaw Devil named Pochita. Due to the debt his father left behind, he has been living a rock-bottom life.",
    genres: ["Action", "Comedy", "Drama", "Horror", "Supernatural"],
    score: 0.82,
    reason: "A wildly creative and unpredictable action manga",
    basedOn: ["Jujutsu Kaisen", "Fire Punch"],
    codexSeriesId:
      mockSeries.find((s) => s.title === "Chainsaw Man")?.id ?? null,
    inLibrary: !!mockSeries.find((s) => s.title === "Chainsaw Man"),
    inCodex: !!mockSeries.find((s) => s.title === "Chainsaw Man"),
    status: "ongoing",
    totalBookCount: 18,
    rating: 85,
    popularity: 267000,
  },
  {
    externalId: "anilist-44347",
    externalUrl: "https://anilist.co/manga/44347",
    title: "Kingdom",
    coverUrl: cover("Kingdom"),
    summary:
      "In the Warring States period of ancient China, Li Xin and Piao are war orphans who dream of one day becoming Great Generals of the Heavens.",
    genres: ["Action", "Drama"],
    score: 0.78,
    reason: "Epic historical warfare at its finest",
    basedOn: ["Vinland Saga", "Vagabond"],
    inLibrary: false,
    inCodex: false,
    status: "ongoing",
    totalBookCount: 72,
    rating: 86,
    popularity: 48000,
  },
];

let isRefreshing = false;

export const recommendationsHandlers = [
  // GET /api/v1/user/recommendations
  http.get("/api/v1/user/recommendations", async () => {
    await delay(200);

    const response: RecommendationsResponse = {
      recommendations: mockRecommendations,
      pluginId: "00000000-0000-0000-0000-000000000001",
      pluginName: "AniList",
      generatedAt: new Date(Date.now() - 3600000).toISOString(),
      cached: true,
      taskStatus: isRefreshing ? "running" : null,
      taskId: isRefreshing ? "00000000-0000-0000-0000-000000000099" : null,
    };

    return HttpResponse.json(response);
  }),

  // POST /api/v1/user/recommendations/refresh
  http.post("/api/v1/user/recommendations/refresh", async () => {
    await delay(200);

    if (isRefreshing) {
      return HttpResponse.json(
        { error: "Recommendation refresh already in progress" },
        { status: 409 },
      );
    }

    isRefreshing = true;
    // Auto-clear after 3 seconds to simulate task completion
    setTimeout(() => {
      isRefreshing = false;
    }, 3000);

    return HttpResponse.json({
      taskId: "00000000-0000-0000-0000-000000000099",
      message: "Recommendation refresh started",
    });
  }),

  // POST /api/v1/user/recommendations/:external_id/dismiss
  http.post(
    "/api/v1/user/recommendations/:external_id/dismiss",
    async ({ params }) => {
      await delay(150);

      const externalId = params.external_id as string;
      mockRecommendations = mockRecommendations.filter(
        (r) => r.externalId !== externalId,
      );

      return HttpResponse.json({ dismissed: true });
    },
  ),
];
