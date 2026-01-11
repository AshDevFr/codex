/**
 * Mock data factories using Faker.js
 *
 * These factories generate realistic mock data for the API responses.
 * They use the OpenAPI schema examples as defaults and Faker for variations.
 */

import { faker } from "@faker-js/faker";

/**
 * Helper to generate consistent UUIDs for related entities.
 * Useful for creating reproducible mock data.
 */
export const seededUuid = (seed: string) => {
  faker.seed(seed.split("").reduce((a, b) => a + b.charCodeAt(0), 0));
  const uuid = faker.string.uuid();
  faker.seed(); // Reset seed
  return uuid;
};

/**
 * User factory
 */
export const createUser = (overrides: Partial<MockUser> = {}): MockUser => ({
  id: faker.string.uuid(),
  username: faker.internet.username(),
  email: faker.internet.email(),
  isAdmin: false,
  emailVerified: true,
  ...overrides,
});

export interface MockUser {
  id: string;
  username: string;
  email: string;
  isAdmin: boolean;
  emailVerified: boolean;
}

/**
 * Library factory
 */
export const createLibrary = (
  overrides: Partial<MockLibrary> = {}
): MockLibrary => {
  const name =
    overrides.name ||
    faker.helpers.arrayElement(["Comics", "Manga", "Ebooks", "Graphic Novels"]);
  return {
    id: faker.string.uuid(),
    name,
    path: `/media/${name.toLowerCase().replace(/\s+/g, "-")}`,
    description: faker.lorem.sentence(),
    isActive: true,
    scanningConfig: null,
    lastScannedAt: faker.date.recent().toISOString(),
    createdAt: faker.date.past().toISOString(),
    updatedAt: faker.date.recent().toISOString(),
    bookCount: faker.number.int({ min: 10, max: 5000 }),
    seriesCount: faker.number.int({ min: 5, max: 500 }),
    allowedFormats: ["CBZ", "CBR", "PDF", "EPUB"],
    excludedPatterns: ".DS_Store\nThumbs.db",
    ...overrides,
  };
};

export interface MockLibrary {
  id: string;
  name: string;
  path: string;
  description: string | null;
  isActive: boolean;
  scanningConfig: unknown | null;
  lastScannedAt: string | null;
  createdAt: string;
  updatedAt: string;
  bookCount: number | null;
  seriesCount: number | null;
  allowedFormats: string[] | null;
  excludedPatterns: string | null;
}

/**
 * Series factory
 */
export const createSeries = (
  overrides: Partial<MockSeries> = {}
): MockSeries => {
  const publishers = [
    "DC Comics",
    "Marvel",
    "Image Comics",
    "Dark Horse",
    "IDW Publishing",
    "Viz Media",
    "Kodansha",
  ];
  const name =
    overrides.name ||
    faker.helpers.arrayElement([
      "Batman: Year One",
      "Spider-Man",
      "Saga",
      "The Walking Dead",
      "One Piece",
      "Attack on Titan",
      "Sandman",
    ]);
  return {
    id: faker.string.uuid(),
    libraryId: faker.string.uuid(),
    name,
    sortName: name.toLowerCase().replace(/^the\s+/, ""),
    description: faker.lorem.paragraph(),
    publisher: faker.helpers.arrayElement(publishers),
    year: faker.number.int({ min: 1980, max: 2024 }),
    bookCount: faker.number.int({ min: 1, max: 100 }),
    path: `/media/comics/${name.replace(/[:\s]+/g, "-")}`,
    selectedCoverSource: "first_book",
    hasCustomCover: false,
    unreadCount: faker.number.int({ min: 0, max: 10 }),
    createdAt: faker.date.past().toISOString(),
    updatedAt: faker.date.recent().toISOString(),
    ...overrides,
  };
};

export interface MockSeries {
  id: string;
  libraryId: string;
  name: string;
  sortName: string | null;
  description: string | null;
  publisher: string | null;
  year: number | null;
  bookCount: number;
  path: string | null;
  selectedCoverSource: string | null;
  hasCustomCover: boolean | null;
  unreadCount: number | null;
  createdAt: string;
  updatedAt: string;
}

/**
 * Book factory
 */
export const createBook = (overrides: Partial<MockBook> = {}): MockBook => {
  const seriesName =
    overrides.seriesName ||
    faker.helpers.arrayElement([
      "Batman: Year One",
      "Spider-Man",
      "Saga",
      "The Walking Dead",
    ]);
  const number = overrides.number ?? faker.number.int({ min: 1, max: 50 });
  const title = `${seriesName} #${number}`;
  const formats = ["cbz", "cbr", "pdf", "epub"];

  return {
    id: faker.string.uuid(),
    seriesId: faker.string.uuid(),
    seriesName,
    title,
    sortTitle: title.toLowerCase(),
    filePath: `/media/comics/${seriesName.replace(/[:\s]+/g, "-")}/${title.replace(/[:\s#]+/g, "-")}.cbz`,
    fileFormat: faker.helpers.arrayElement(formats),
    fileSize: faker.number.int({ min: 10_000_000, max: 100_000_000 }),
    fileHash: faker.string.alphanumeric(40),
    pageCount: faker.number.int({ min: 20, max: 50 }),
    number,
    createdAt: faker.date.past().toISOString(),
    updatedAt: faker.date.recent().toISOString(),
    readProgress: null,
    ...overrides,
  };
};

export interface MockBook {
  id: string;
  seriesId: string;
  seriesName: string;
  title: string;
  sortTitle: string | null;
  filePath: string;
  fileFormat: string;
  fileSize: number;
  fileHash: string;
  pageCount: number;
  number: number | null;
  createdAt: string;
  updatedAt: string;
  readProgress: MockReadProgress | null;
}

/**
 * Read progress factory
 */
export const createReadProgress = (
  overrides: Partial<MockReadProgress> = {}
): MockReadProgress => ({
  id: faker.string.uuid(),
  userId: faker.string.uuid(),
  bookId: faker.string.uuid(),
  currentPage: faker.number.int({ min: 1, max: 30 }),
  totalPages: faker.number.int({ min: 30, max: 50 }),
  percentage: faker.number.float({ min: 0, max: 100, fractionDigits: 1 }),
  isCompleted: false,
  lastReadAt: faker.date.recent().toISOString(),
  createdAt: faker.date.past().toISOString(),
  updatedAt: faker.date.recent().toISOString(),
  ...overrides,
});

export interface MockReadProgress {
  id: string;
  userId: string;
  bookId: string;
  currentPage: number;
  totalPages: number;
  percentage: number;
  isCompleted: boolean;
  lastReadAt: string;
  createdAt: string;
  updatedAt: string;
}

/**
 * Paginated response factory
 * Matches the server's PaginatedResponse format
 */
export const createPaginatedResponse = <T>(
  data: T[],
  options: { page?: number; pageSize?: number; total?: number } = {}
): PaginatedResponse<T> => {
  const page = options.page ?? 0;
  const pageSize = options.pageSize ?? 20;
  const total = options.total ?? data.length;
  const totalPages = Math.ceil(total / pageSize);

  return {
    data,
    page,
    pageSize,
    total,
    totalPages,
  };
};

export interface PaginatedResponse<T> {
  data: T[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
}

/**
 * Create a list of items
 */
export const createList = <T>(
  factory: (index: number) => T,
  count: number
): T[] => Array.from({ length: count }, (_, i) => factory(i));
