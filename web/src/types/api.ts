// API response types matching the Rust backend

export interface User {
	id: string;
	username: string;
	email: string;
	isAdmin: boolean;
	emailVerified: boolean;
}

export interface ScanningConfig {
	cronSchedule?: string;
	scanMode: "normal" | "deep";
	enabled: boolean;
	scanOnStart: boolean;
	purgeDeletedOnScan: boolean;
}

export interface Library {
	id: string;
	name: string;
	path: string;
	description?: string;
	isActive: boolean;
	scanningConfig?: ScanningConfig;
	lastScannedAt?: string;
	createdAt: string;
	updatedAt: string;
	bookCount?: number;
	seriesCount?: number;
	allowedFormats?: string[];
	excludedPatterns?: string;
}

export interface Series {
	id: string;
	libraryId: string;
	name: string;
	sortName?: string;
	publisher?: string;
	year?: number;
	description?: string;
	status?: string;
	coverPath?: string;
	createdAt: string;
	updatedAt: string;
	bookCount?: number;
	path?: string;
	selectedCoverSource?: string;
	hasCustomCover?: boolean;
}

export interface Book {
	id: string;
	seriesId: string;
	seriesName: string;
	title: string;
	sortTitle?: string;
	filePath: string;
	fileFormat: string;
	fileSize: number;
	fileHash: string;
	pageCount: number;
	number?: number;
	createdAt: string;
	updatedAt: string;
}

export interface ReadProgress {
	user_id: string;
	book_id: string;
	current_page: number;
	completed: boolean;
	started_at: string;
	updated_at: string;
}

export interface LoginRequest {
	username: string;
	password: string;
}

export interface LoginResponse {
	accessToken: string;
	tokenType: string;
	expiresIn: number;
	user: User;
}

export interface RegisterRequest {
	username: string;
	email: string;
	password: string;
}

export interface RegisterResponse {
	accessToken?: string;
	tokenType?: string;
	expiresIn?: number;
	user: User;
	message?: string;
}

export interface ApiError {
	error: string;
	message?: string;
}

export interface PaginatedResponse<T> {
	data: T[];
	total: number;
	page: number;
	pageSize: number;
	totalPages: number;
}

export interface FileSystemEntry {
	name: string;
	path: string;
	is_directory: boolean;
	is_readable: boolean;
}

export interface BrowseResponse {
	current_path: string;
	parent_path: string | null;
	entries: FileSystemEntry[];
}

export interface CreateLibraryRequest {
	name: string;
	path: string;
	description?: string;
	scanningConfig?: ScanningConfig;
	scanImmediately?: boolean;
	allowedFormats?: string[];
	excludedPatterns?: string;
}

export type ScanStatus =
	| "pending"
	| "running"
	| "completed"
	| "failed"
	| "cancelled";

export interface ScanProgress {
	libraryId: string;
	status: ScanStatus;
	filesTotal: number;
	filesProcessed: number;
	seriesFound: number;
	booksFound: number;
	errors: string[];
	startedAt: string;
	completedAt?: string;
}

// Setup wizard types
export interface SetupStatusResponse {
	setupRequired: boolean;
	hasUsers: boolean;
}

export interface InitializeSetupRequest {
	username: string;
	email: string;
	password: string;
}

export interface InitializeSetupResponse {
	user: User;
	accessToken: string;
	tokenType: string;
	expiresIn: number;
	message: string;
}

export interface ConfigureSettingsRequest {
	settings: Record<string, string>;
	skipConfiguration: boolean;
}

export interface ConfigureSettingsResponse {
	message: string;
	settingsConfigured: number;
}
