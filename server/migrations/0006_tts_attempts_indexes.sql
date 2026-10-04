ALTER TABLE tts_chunks ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS chapters_book_id ON chapters (book_id);
CREATE INDEX IF NOT EXISTS tts_chunks_job_status ON tts_chunks (job_id, status);
