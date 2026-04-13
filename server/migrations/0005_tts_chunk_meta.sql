ALTER TABLE tts_chunks ADD COLUMN epub_chapter_idx INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tts_chunks ADD COLUMN duration_sec REAL;
