-- Neko initial schema (SQLite)

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('admin', 'user')),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS libraries (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('anime', 'manga', 'book')),
    root_path  TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Anime ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS anime_series (
    id           TEXT PRIMARY KEY,
    library_id   TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    romaji_title TEXT,
    description  TEXT,
    cover_path   TEXT,
    status       TEXT NOT NULL DEFAULT 'unknown',
    year         INTEGER,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (library_id, title)
);

CREATE TABLE IF NOT EXISTS episodes (
    id             TEXT PRIMARY KEY,
    series_id      TEXT NOT NULL REFERENCES anime_series(id) ON DELETE CASCADE,
    season_number  INTEGER NOT NULL DEFAULT 1,
    episode_number REAL NOT NULL,
    title          TEXT,
    file_path      TEXT NOT NULL UNIQUE,
    duration_sec   INTEGER,
    size_bytes     INTEGER,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (series_id, season_number, episode_number)
);
CREATE INDEX IF NOT EXISTS idx_episodes_series ON episodes(series_id);

-- Manga ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS manga (
    id          TEXT PRIMARY KEY,
    library_id  TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    author      TEXT,
    description TEXT,
    cover_path  TEXT,
    status      TEXT NOT NULL DEFAULT 'unknown',
    year        INTEGER,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (library_id, title)
);

CREATE TABLE IF NOT EXISTS chapters (
    id             TEXT PRIMARY KEY,
    manga_id       TEXT NOT NULL REFERENCES manga(id) ON DELETE CASCADE,
    chapter_number REAL NOT NULL,
    title          TEXT,
    file_path      TEXT NOT NULL,
    page_count     INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (manga_id, chapter_number)
);
CREATE INDEX IF NOT EXISTS idx_chapters_manga ON chapters(manga_id);

-- Books ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS books (
    id          TEXT PRIMARY KEY,
    library_id  TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    author      TEXT,
    description TEXT,
    cover_path  TEXT,
    file_path   TEXT NOT NULL UNIQUE,
    format      TEXT,
    page_count  INTEGER,
    year        INTEGER,
    size_bytes  INTEGER,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_books_library ON books(library_id);

-- Watch / read progress -------------------------------------------------------
-- Exactly one of (episode_id, chapter_id, book_id) must be set.
CREATE TABLE IF NOT EXISTS progress (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_kind   TEXT NOT NULL CHECK (media_kind IN ('anime', 'manga', 'book')),
    episode_id   TEXT REFERENCES episodes(id) ON DELETE CASCADE,
    chapter_id   TEXT REFERENCES chapters(id) ON DELETE CASCADE,
    book_id      TEXT REFERENCES books(id) ON DELETE CASCADE,
    position_sec INTEGER NOT NULL DEFAULT 0,
    page         INTEGER NOT NULL DEFAULT 0,
    completed    INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (
        (episode_id IS NOT NULL AND chapter_id IS NULL AND book_id IS NULL AND media_kind = 'anime')
        OR (episode_id IS NULL AND chapter_id IS NOT NULL AND book_id IS NULL AND media_kind = 'manga')
        OR (episode_id IS NULL AND chapter_id IS NULL AND book_id IS NOT NULL AND media_kind = 'book')
    )
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_progress_episode ON progress(user_id, episode_id) WHERE episode_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_progress_chapter ON progress(user_id, chapter_id) WHERE chapter_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_progress_book ON progress(user_id, book_id) WHERE book_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_progress_user ON progress(user_id);

-- Library scan history --------------------------------------------------------
CREATE TABLE IF NOT EXISTS scan_events (
    id          TEXT PRIMARY KEY,
    library_id  TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    status      TEXT NOT NULL DEFAULT 'done',
    files_added INTEGER NOT NULL DEFAULT 0,
    message     TEXT,
    started_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    finished_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_scan_library ON scan_events(library_id);
