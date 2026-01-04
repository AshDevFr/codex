-- Initial database schema for Codex
--
-- This migration uses PostgreSQL syntax but is compatible with both:
-- - PostgreSQL: Uses native types (UUID, TIMESTAMPTZ, BOOLEAN, BIGINT)
-- - SQLite: Type affinity converts to compatible types (TEXT, TEXT, INTEGER, INTEGER)
--
-- SQLx handles automatic type conversion at runtime based on the connected database

-- ============================================================================
-- Libraries Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS libraries (
    id UUID PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    scanning_strategy TEXT NOT NULL,
    scanning_config TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    last_scanned_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_libraries_path ON libraries(path);

-- ============================================================================
-- Series Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS series (
    id UUID PRIMARY KEY NOT NULL,
    library_id UUID NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    sort_name TEXT,
    summary TEXT,
    publisher TEXT,
    year INTEGER,
    book_count INTEGER NOT NULL DEFAULT 0,
    user_rating REAL,
    external_rating REAL,
    external_rating_count INTEGER,
    external_rating_source TEXT,
    custom_metadata TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (library_id) REFERENCES libraries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_series_library_id ON series(library_id);
CREATE INDEX IF NOT EXISTS idx_series_normalized_name ON series(normalized_name);
CREATE INDEX IF NOT EXISTS idx_series_name ON series(name);

-- ============================================================================
-- Books Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS books (
    id UUID PRIMARY KEY NOT NULL,
    series_id UUID NOT NULL,
    title TEXT,
    number REAL,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    file_hash TEXT NOT NULL,
    format TEXT NOT NULL,
    page_count INTEGER NOT NULL,
    modified_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_books_series_id ON books(series_id);
CREATE INDEX IF NOT EXISTS idx_books_file_hash ON books(file_hash);
CREATE INDEX IF NOT EXISTS idx_books_file_path ON books(file_path);

-- ============================================================================
-- Book Metadata Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS book_metadata_records (
    id UUID PRIMARY KEY NOT NULL,
    book_id UUID NOT NULL UNIQUE,
    summary TEXT,
    writer TEXT,
    penciller TEXT,
    inker TEXT,
    colorist TEXT,
    letterer TEXT,
    cover_artist TEXT,
    editor TEXT,
    publisher TEXT,
    imprint TEXT,
    genre TEXT,
    web TEXT,
    language_iso TEXT,
    format_detail TEXT,
    black_and_white BOOLEAN,
    manga BOOLEAN,
    year INTEGER,
    month INTEGER,
    day INTEGER,
    volume INTEGER,
    count INTEGER,
    isbns TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

-- ============================================================================
-- Pages Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS pages (
    id UUID PRIMARY KEY NOT NULL,
    book_id UUID NOT NULL,
    page_number INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    format TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    file_size BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pages_book_id ON pages(book_id);
CREATE INDEX IF NOT EXISTS idx_pages_book_page ON pages(book_id, page_number);

-- ============================================================================
-- Users Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    last_login_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- ============================================================================
-- Read Progress Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS read_progress (
    id UUID PRIMARY KEY NOT NULL,
    user_id UUID NOT NULL,
    book_id UUID NOT NULL,
    current_page INTEGER NOT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    started_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    UNIQUE(user_id, book_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_read_progress_user_id ON read_progress(user_id);
CREATE INDEX IF NOT EXISTS idx_read_progress_book_id ON read_progress(book_id);

-- ============================================================================
-- Metadata Sources Table
-- ============================================================================

CREATE TABLE IF NOT EXISTS metadata_sources (
    id UUID PRIMARY KEY NOT NULL,
    series_id UUID NOT NULL,
    source_name TEXT NOT NULL,
    external_id TEXT NOT NULL,
    external_url TEXT,
    confidence REAL NOT NULL,
    metadata_json TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    UNIQUE(series_id, source_name, external_id),
    FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_metadata_sources_series_id ON metadata_sources(series_id);
CREATE INDEX IF NOT EXISTS idx_metadata_sources_source_name ON metadata_sources(source_name);
