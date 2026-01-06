// API response types matching the Rust backend

export interface User {
  id: string;
  username: string;
  email: string;
  isAdmin: boolean;
  emailVerified: boolean;
}

export interface Library {
  id: string;
  name: string;
  path: string;
  scan_mode: 'DISABLED' | 'MANUAL' | 'AUTO';
  scan_interval_hours?: number;
  last_scan_at?: string;
  created_at: string;
  updated_at: string;
  book_count?: number;
  series_count?: number;
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
