/**
 * Metadata API mock handlers (genres, tags, etc.)
 */

import { http, HttpResponse, delay } from "msw";

// Mock genres data
const mockGenres = [
  { id: "genre-1", name: "Action", series_count: 15 },
  { id: "genre-2", name: "Comedy", series_count: 12 },
  { id: "genre-3", name: "Drama", series_count: 10 },
  { id: "genre-4", name: "Fantasy", series_count: 8 },
  { id: "genre-5", name: "Horror", series_count: 5 },
  { id: "genre-6", name: "Romance", series_count: 7 },
  { id: "genre-7", name: "Sci-Fi", series_count: 6 },
  { id: "genre-8", name: "Slice of Life", series_count: 4 },
];

// Mock tags data
const mockTags = [
  { id: "tag-1", name: "Favorite", series_count: 10 },
  { id: "tag-2", name: "Reading", series_count: 8 },
  { id: "tag-3", name: "Complete", series_count: 15 },
  { id: "tag-4", name: "Dropped", series_count: 3 },
  { id: "tag-5", name: "To Read", series_count: 20 },
];

export const metadataHandlers = [
  // Get all genres
  http.get("/api/v1/genres", async () => {
    await delay(100);
    return HttpResponse.json({ genres: mockGenres });
  }),

  // Get all tags
  http.get("/api/v1/tags", async () => {
    await delay(100);
    return HttpResponse.json({ tags: mockTags });
  }),
];

// Export mock data for testing
export const getMockGenres = () => [...mockGenres];
export const getMockTags = () => [...mockTags];
