import { defineConfig } from "playwright/test";

const BASE_URL = process.env.BASE_URL || "http://localhost:5173";
const VIEWPORT_WIDTH = parseInt(process.env.VIEWPORT_WIDTH || "1920", 10);
const VIEWPORT_HEIGHT = parseInt(process.env.VIEWPORT_HEIGHT || "1080", 10);

export default defineConfig({
  testDir: "./scripts",
  timeout: 60000,
  expect: {
    timeout: 10000,
  },
  use: {
    baseURL: BASE_URL,
    viewport: { width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT },
    colorScheme: "dark",
    screenshot: "off",
    video: "off",
    trace: "off",
    headless: true,
  },
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
      },
    },
  ],
});

// Library type for configuration
export type LibraryType = "comics" | "manga" | "books";

export interface LibraryConfig {
  name: string;
  path: string;
  type: LibraryType;
  readingDirection: "ltr" | "rtl";
  formats: string[];
  seriesStrategy: string;
  excludedPatterns?: string;
  scanImmediately: boolean;
  cronSchedule?: string;
}

export const config = {
  baseUrl: BASE_URL,
  viewport: { width: VIEWPORT_WIDTH, height: VIEWPORT_HEIGHT },
  outputDir: "./output",
  admin: {
    username: process.env.ADMIN_USERNAME || "admin",
    email: process.env.ADMIN_EMAIL || "admin@example.com",
    password: process.env.ADMIN_PASSWORD || "SecurePass123!",
  },
  libraries: [
    {
      name: process.env.LIBRARY_1_NAME || "Comics",
      path: process.env.LIBRARY_1_PATH || "/libraries/comics",
      type: "comics" as LibraryType,
      readingDirection: "ltr" as const,
      formats: ["CBZ", "CBR"],
      seriesStrategy: "series_volume",
      scanImmediately: true,
    },
    {
      name: process.env.LIBRARY_2_NAME || "Manga",
      path: process.env.LIBRARY_2_PATH || "/libraries/manga",
      type: "manga" as LibraryType,
      readingDirection: "rtl" as const,
      formats: ["CBZ", "CBR"],
      seriesStrategy: "series_volume",
      excludedPatterns: "_to_filter",
      scanImmediately: true,
      cronSchedule: "0 0 * * *", // Daily at midnight
    },
    {
      name: process.env.LIBRARY_3_NAME || "Books",
      path: process.env.LIBRARY_3_PATH || "/libraries/books",
      type: "books" as LibraryType,
      readingDirection: "ltr" as const,
      formats: ["EPUB", "PDF"],
      seriesStrategy: "calibre_author",
      scanImmediately: true,
    },
  ] as LibraryConfig[],
};
