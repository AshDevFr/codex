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
	autoScanOnCreate: boolean;
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
}

export interface Series {
	id: string;
	library_id: string;
	name: string;
	sort_name?: string;
	publisher?: string;
	year?: number;
	description?: string;
	status?: string;
	cover_path?: string;
	created_at: string;
	updated_at: string;
	book_count?: number;
}

export interface Book {
	id: string;
	series_id: string;
	title: string;
	sort_title?: string;
	file_path: string;
	file_size: number;
	file_hash: string;
	page_count?: number;
	chapter_number?: string;
	release_date?: string;
	writer?: string;
	penciller?: string;
	inker?: string;
	colorist?: string;
	letterer?: string;
	cover_artist?: string;
	editor?: string;
	publisher?: string;
	imprint?: string;
	genre?: string;
	tags?: string[];
	description?: string;
	isbn?: string;
	age_rating?: string;
	file_last_modified: string;
	created_at: string;
	updated_at: string;
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
	items: T[];
	total: number;
	page: number;
	page_size: number;
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
}

export type ScanStatus =
	| "queued"
	| "running"
	| "completed"
	| "failed"
	| "cancelled";

export interface ScanProgress {
	library_id: string;
	status: ScanStatus;
	files_total: number;
	files_processed: number;
	series_found: number;
	books_found: number;
	errors: string[];
	started_at?: string;
	completed_at?: string;
	error_message?: string;
}
