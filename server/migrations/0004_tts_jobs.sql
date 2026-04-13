CREATE TABLE IF NOT EXISTS tts_jobs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    text_book_id    INTEGER NOT NULL REFERENCES text_books(id),
    status          TEXT NOT NULL DEFAULT 'running',
    total_chunks    INTEGER NOT NULL DEFAULT 0,
    done_chunks     INTEGER NOT NULL DEFAULT 0,
    failed_chunks   INTEGER NOT NULL DEFAULT 0,
    audio_book_id   INTEGER REFERENCES books(id),
    audio_ext       TEXT NOT NULL DEFAULT 'mp3',
    error_msg       TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tts_chunks (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id    INTEGER NOT NULL REFERENCES tts_jobs(id),
    seq       INTEGER NOT NULL,
    text      TEXT NOT NULL,
    status    TEXT NOT NULL DEFAULT 'pending',
    file_path TEXT,
    error_msg TEXT,
    UNIQUE(job_id, seq)
);
