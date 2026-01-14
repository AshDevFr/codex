/**
 * Mock data factories using Faker.js
 *
 * These factories generate realistic mock data for the API responses.
 * They use the auto-generated types from the OpenAPI schema.
 */

import { faker } from "@faker-js/faker";
import type { components } from "@/types/api.generated";

// Re-export types for convenience
export type UserDto = components["schemas"]["UserDto"];
export type LibraryDto = components["schemas"]["LibraryDto"];
export type SeriesDto = components["schemas"]["SeriesDto"];
export type BookDto = components["schemas"]["BookDto"];

// Extended mock types with additional fields for UI convenience
export type MockLibrary = LibraryDto;
export type MockSeries = SeriesDto & { libraryName?: string };
// MockBook includes libraryId for mock filtering (books are associated with libraries via series)
export type MockBook = BookDto & { libraryId?: string };
export type ReadProgressResponse =
	components["schemas"]["ReadProgressResponse"];
export type MetricsDto = components["schemas"]["MetricsDto"];
export type LibraryMetricsDto = components["schemas"]["LibraryMetricsDto"];
export type TaskMetricsResponse = components["schemas"]["TaskMetricsResponse"];
export type TaskMetricsSummaryDto =
	components["schemas"]["TaskMetricsSummaryDto"];
export type TaskTypeMetricsDto = components["schemas"]["TaskTypeMetricsDto"];
export type QueueHealthMetricsDto =
	components["schemas"]["QueueHealthMetricsDto"];
export type TaskResponse = components["schemas"]["TaskResponse"];
export type TaskStats = components["schemas"]["TaskStats"];
export type SettingDto = components["schemas"]["SettingDto"];
export type SettingHistoryDto = components["schemas"]["SettingHistoryDto"];
export type DuplicateGroup = components["schemas"]["DuplicateGroup"];
export type PaginatedResponse<T> = {
	data: T[];
	page: number;
	pageSize: number;
	total: number;
	totalPages: number;
};

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
 * User factory - matches UserDto schema
 */
export const createUser = (overrides: Partial<UserDto> = {}): UserDto => ({
	id: faker.string.uuid(),
	username: faker.internet.username(),
	email: faker.internet.email(),
	isAdmin: false,
	isActive: true,
	lastLoginAt: faker.date.recent().toISOString(),
	createdAt: faker.date.past().toISOString(),
	updatedAt: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Library factory - matches LibraryDto schema
 */
export const createLibrary = (
	overrides: Partial<LibraryDto> = {},
): LibraryDto => {
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
		defaultReadingDirection: "ltr",
		seriesStrategy: "series_volume",
		bookStrategy: "smart",
		numberStrategy: "smart",
		...overrides,
	};
};

/**
 * Series factory - matches SeriesDto schema with optional mock extensions
 */
export const createSeries = (
	overrides: Partial<MockSeries> = {},
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
		libraryId: overrides.libraryId || faker.string.uuid(),
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

/**
 * Book factory - matches BookDto schema
 * Note: libraryId is an extension for mock filtering (books are associated with libraries via series)
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
	const title = overrides.title || `${seriesName} #${number}`;
	const formats = ["cbz", "cbr", "pdf", "epub"];

	return {
		id: faker.string.uuid(),
		seriesId: overrides.seriesId || faker.string.uuid(),
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
		readingDirection: "ltr",
		deleted: false,
		...overrides,
	};
};

/**
 * Read progress factory - matches ReadProgressResponse schema
 */
export const createReadProgress = (
	overrides: Partial<ReadProgressResponse> = {},
): ReadProgressResponse => ({
	id: faker.string.uuid(),
	user_id: faker.string.uuid(),
	book_id: faker.string.uuid(),
	current_page: faker.number.int({ min: 1, max: 30 }),
	completed: false,
	completed_at: null,
	started_at: faker.date.past().toISOString(),
	updated_at: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Setting factory - matches SettingDto schema
 */
export const createSetting = (
	overrides: Partial<SettingDto> = {},
): SettingDto => ({
	id: faker.string.uuid(),
	key:
		overrides.key ||
		faker.helpers.arrayElement([
			"server.name",
			"server.port",
			"auth.registration_enabled",
			"scanning.default_interval",
		]),
	value: overrides.value || faker.word.sample(),
	default_value: overrides.default_value || faker.word.sample(),
	description: faker.lorem.sentence(),
	category:
		overrides.category ||
		faker.helpers.arrayElement(["server", "auth", "scanning"]),
	value_type: "string",
	is_sensitive: false,
	updated_at: faker.date.recent().toISOString(),
	updated_by: faker.string.uuid(),
	version: faker.number.int({ min: 1, max: 10 }),
	...overrides,
});

/**
 * Setting history factory - matches SettingHistoryDto schema
 */
export const createSettingHistory = (
	overrides: Partial<SettingHistoryDto> = {},
): SettingHistoryDto => ({
	id: faker.string.uuid(),
	setting_id: faker.string.uuid(),
	key: overrides.key || "server.name",
	old_value: "Old Value",
	new_value: "New Value",
	changed_at: faker.date.recent().toISOString(),
	changed_by: faker.string.uuid(),
	change_reason: faker.lorem.sentence(),
	ip_address: faker.internet.ip(),
	...overrides,
});

/**
 * Task factory - matches TaskDto schema
 */
export const createTask = (
	overrides: Partial<TaskResponse> = {},
): TaskResponse => {
	const statuses: TaskResponse["status"][] = [
		"pending",
		"processing",
		"completed",
		"failed",
	];
	return {
		id: faker.string.uuid(),
		task_type:
			overrides.task_type ||
			faker.helpers.arrayElement([
				"scan_library",
				"generate_thumbnails",
				"analyze_metadata",
			]),
		status: overrides.status || faker.helpers.arrayElement(statuses),
		priority: faker.number.int({ min: 0, max: 10 }),
		attempts: faker.number.int({ min: 0, max: 3 }),
		max_attempts: 3,
		created_at: faker.date.past().toISOString(),
		scheduled_for: faker.date.recent().toISOString(),
		started_at: faker.date.recent().toISOString(),
		completed_at: null,
		last_error: null,
		library_id: faker.string.uuid(),
		book_id: null,
		series_id: null,
		locked_by: null,
		locked_until: null,
		params: null,
		result: null,
		...overrides,
	};
};

/**
 * Task stats factory - matches TaskStats schema
 */
export const createTaskStats = (
	overrides: Partial<TaskStats> = {},
): TaskStats => ({
	pending: faker.number.int({ min: 0, max: 50 }),
	processing: faker.number.int({ min: 0, max: 10 }),
	completed: faker.number.int({ min: 100, max: 5000 }),
	failed: faker.number.int({ min: 0, max: 20 }),
	stale: faker.number.int({ min: 0, max: 5 }),
	total: faker.number.int({ min: 100, max: 5100 }),
	by_type: {},
	...overrides,
});

/**
 * Inventory metrics factory - matches MetricsDto schema
 */
export const createInventoryMetrics = (
	overrides: Partial<MetricsDto> = {},
): MetricsDto => ({
	library_count: faker.number.int({ min: 1, max: 10 }),
	series_count: faker.number.int({ min: 10, max: 500 }),
	book_count: faker.number.int({ min: 100, max: 10000 }),
	total_book_size: faker.number.int({
		min: 1_000_000_000,
		max: 100_000_000_000,
	}),
	user_count: faker.number.int({ min: 1, max: 50 }),
	database_size: faker.number.int({ min: 10_000_000, max: 500_000_000 }),
	page_count: faker.number.int({ min: 10000, max: 500000 }),
	libraries: [],
	...overrides,
});

/**
 * Library metrics factory - matches LibraryMetricsDto schema
 */
export const createLibraryMetrics = (
	overrides: Partial<LibraryMetricsDto> = {},
): LibraryMetricsDto => ({
	id: faker.string.uuid(),
	name: faker.helpers.arrayElement(["Comics", "Manga", "Ebooks"]),
	series_count: faker.number.int({ min: 5, max: 100 }),
	book_count: faker.number.int({ min: 50, max: 2000 }),
	total_size: faker.number.int({ min: 500_000_000, max: 50_000_000_000 }),
	...overrides,
});

/**
 * Task metrics factory - matches TaskMetricsResponse schema
 */
export const createTaskMetrics = (
	overrides: Partial<TaskMetricsResponse> = {},
): TaskMetricsResponse => ({
	updated_at: faker.date.recent().toISOString(),
	retention: "30",
	summary: {
		total_executed: faker.number.int({ min: 100, max: 10000 }),
		total_succeeded: faker.number.int({ min: 90, max: 9000 }),
		total_failed: faker.number.int({ min: 0, max: 100 }),
		avg_duration_ms: faker.number.float({ min: 100, max: 5000 }),
		avg_queue_wait_ms: faker.number.float({ min: 10, max: 500 }),
		tasks_per_minute: faker.number.float({ min: 0.5, max: 20 }),
	},
	by_type: [],
	queue: {
		pending_count: faker.number.int({ min: 0, max: 50 }),
		processing_count: faker.number.int({ min: 0, max: 5 }),
		stale_count: 0,
		oldest_pending_age_ms: null,
	},
	...overrides,
});

/**
 * Task type metrics factory - matches TaskTypeMetricsDto schema
 */
export const createTaskTypeMetrics = (
	overrides: Partial<TaskTypeMetricsDto> = {},
): TaskTypeMetricsDto => ({
	task_type: overrides.task_type || "scan_library",
	executed: faker.number.int({ min: 10, max: 1000 }),
	succeeded: faker.number.int({ min: 9, max: 950 }),
	failed: faker.number.int({ min: 0, max: 50 }),
	retried: faker.number.int({ min: 0, max: 20 }),
	avg_duration_ms: faker.number.float({ min: 500, max: 10000 }),
	min_duration_ms: faker.number.int({ min: 100, max: 500 }),
	max_duration_ms: faker.number.int({ min: 10000, max: 60000 }),
	p50_duration_ms: faker.number.int({ min: 1000, max: 3000 }),
	p95_duration_ms: faker.number.int({ min: 5000, max: 15000 }),
	avg_queue_wait_ms: faker.number.float({ min: 10, max: 200 }),
	items_processed: faker.number.int({ min: 100, max: 50000 }),
	bytes_processed: faker.number.int({ min: 100_000_000, max: 10_000_000_000 }),
	throughput_per_sec: faker.number.float({ min: 1, max: 100 }),
	error_rate_pct: faker.number.float({ min: 0, max: 10 }),
	last_error: null,
	last_error_at: null,
	...overrides,
});

/**
 * Duplicate group factory - matches DuplicateGroup schema
 */
export const createDuplicateGroup = (
	overrides: Partial<DuplicateGroup> = {},
): DuplicateGroup => ({
	id: faker.string.uuid(),
	file_hash: faker.string.alphanumeric(64),
	duplicate_count: faker.number.int({ min: 2, max: 5 }),
	book_ids: [faker.string.uuid(), faker.string.uuid()],
	created_at: faker.date.past().toISOString(),
	updated_at: faker.date.recent().toISOString(),
	...overrides,
});

/**
 * Paginated response factory
 * Matches the server's PaginatedResponse format
 */
export const createPaginatedResponse = <T>(
	data: T[],
	options: { page?: number; pageSize?: number; total?: number } = {},
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

/**
 * Create a list of items
 */
export const createList = <T>(
	factory: (index: number) => T,
	count: number,
): T[] => Array.from({ length: count }, (_, i) => factory(i));
