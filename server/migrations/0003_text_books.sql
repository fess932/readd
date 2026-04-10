CREATE TABLE IF NOT EXISTS text_books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT NOT NULL,
    cover_path TEXT,
    file_path TEXT NOT NULL,
    file_size INTEGER,
    uploaded_by_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
