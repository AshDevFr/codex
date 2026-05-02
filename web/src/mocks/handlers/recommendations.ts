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
    totalVolumeCount: 110,
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
    totalVolumeCount: 34,
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
    inLibrary: true,
    inCodex: false,
    status: "hiatus",
    totalVolumeCount: 42,
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
    inLibrary: true,
    inCodex: false,
    status: "ended",
    totalVolumeCount: 27,
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
    totalVolumeCount: 37,
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
    codexSeriesId: "mock-fma-series-id",
    inLibrary: false,
    inCodex: true,
    status: "ended",
    totalVolumeCount: 27,
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
    codexSeriesId: "mock-csm-series-id",
    inLibrary: false,
    inCodex: true,
    status: "ongoing",
    totalVolumeCount: 18,
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
    totalVolumeCount: 72,
    rating: 86,
    popularity: 48000,
  },
  {
    externalId: "anilist-30011",
    externalUrl: "https://anilist.co/manga/30011",
    title: "Naruto",
    coverUrl: cover("Naruto"),
    summary:
      "Before Naruto's birth, a great demon fox had attacked the Hidden Leaf Village. A man known as the 4th Hokage sealed the demon inside the newly born Naruto, causing him to unknowingly grow up detested by his fellow villagers.",
    genres: ["Action", "Adventure", "Comedy"],
    score: 0.76,
    reason: "A classic shonen journey of growth and perseverance",
    basedOn: ["One Piece", "Bleach"],
    inLibrary: true,
    inCodex: false,
    status: "ended",
    totalVolumeCount: 72,
    rating: 79,
    popularity: 293000,
  },
  {
    externalId: "anilist-30012",
    externalUrl: "https://anilist.co/manga/30012",
    title: "Bleach",
    coverUrl: cover("Bleach"),
    summary:
      "Ichigo Kurosaki has always been able to see ghosts, but this ability doesn't change his life nearly as much as his close encounter with Rukia Kuchiki, a Soul Reaper and member of the mysterious Soul Society.",
    genres: ["Action", "Adventure", "Supernatural"],
    score: 0.74,
    reason: "Fast-paced supernatural battles with stylish art",
    basedOn: ["Naruto", "Yu Yu Hakusho"],
    inLibrary: false,
    inCodex: false,
    status: "ended",
    totalVolumeCount: 74,
    rating: 76,
    popularity: 187000,
  },
  {
    externalId: "anilist-30105",
    externalUrl: "https://anilist.co/manga/30105",
    title: "Hunter x Hunter",
    coverUrl: cover("Hunter x\nHunter"),
    summary:
      "Gon Freecss aspires to become a Hunter, an exceptional being capable of greatness. With his friends and his potential, he seeks for his father who left him when he was younger.",
    genres: ["Action", "Adventure", "Fantasy"],
    score: 0.73,
    reason: "Complex power systems and brilliant story arcs",
    basedOn: ["Yu Yu Hakusho", "One Piece"],
    codexSeriesId: "mock-hxh-series-id",
    inLibrary: false,
    inCodex: true,
    status: "hiatus",
    totalVolumeCount: 37,
    rating: 90,
    popularity: 215000,
  },
  {
    externalId: "anilist-30085",
    externalUrl: "https://anilist.co/manga/30085",
    title: "Slam Dunk",
    coverUrl: cover("Slam Dunk"),
    summary:
      "Hanamichi Sakuragi, a tall, red-haired delinquent, enrolls in Shohoku High School to impress a girl. He is recruited onto the basketball team and discovers he has a natural talent for the sport.",
    genres: ["Comedy", "Drama", "Sports"],
    score: 0.71,
    reason: "The greatest sports manga ever written",
    basedOn: ["Haikyuu!!", "Kuroko no Basket"],
    inLibrary: true,
    inCodex: false,
    status: "ended",
    totalVolumeCount: 31,
    rating: 88,
    popularity: 72000,
  },
  {
    externalId: "anilist-30001",
    externalUrl: "https://anilist.co/manga/30001",
    title: "Monster",
    coverUrl: cover("Monster"),
    summary:
      "Dr. Kenzo Tenma, an elite neurosurgeon, makes a life-changing decision when he chooses to save the life of a young boy over the mayor. His decision leads him down a dark path as the boy grows up to become a serial killer.",
    genres: ["Drama", "Mystery", "Psychological", "Thriller"],
    score: 0.69,
    reason: "A gripping psychological thriller by Naoki Urasawa",
    basedOn: ["20th Century Boys", "Pluto"],
    inLibrary: false,
    inCodex: false,
    status: "ended",
    totalVolumeCount: 18,
    rating: 92,
    popularity: 98000,
  },
  {
    externalId: "anilist-30025",
    externalUrl: "https://anilist.co/manga/30025",
    title: "Death Note",
    coverUrl: cover("Death Note"),
    summary:
      "Light Yagami is an ace student with great prospects — and he's bored out of his mind. But all that changes when he finds the Death Note, a notebook dropped by a rogue Shinigami death god.",
    genres: ["Mystery", "Psychological", "Supernatural", "Thriller"],
    score: 0.67,
    reason: "A cat-and-mouse intellectual battle like no other",
    basedOn: ["Monster", "Code Geass"],
    inLibrary: true,
    inCodex: true,
    codexSeriesId: "mock-dn-series-id",
    status: "ended",
    totalVolumeCount: 12,
    rating: 85,
    popularity: 310000,
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
