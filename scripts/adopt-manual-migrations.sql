-- One-off repair for databases created before sqlx tracked migrations.
--
-- Symptom on startup:
--   "while executing migration 2: duplicate column name: finished_at"
--   or "migration 2 was previously applied but has been modified"
--
-- Columns of migrations 2 and 5 were once added by hand, so the schema has them
-- but sqlx does not know. This records each as applied, only where its column exists.
-- Safe to run more than once. Stop the server first:
--
--   sqlite3 path/to/readd.db < scripts/adopt-manual-migrations.sql

INSERT OR REPLACE INTO _sqlx_migrations (version, description, success, checksum, execution_time)
SELECT 2, 'finished at', 1, x'73d051b5533b37f717a29f53161b4f3a00ebad612e66267ea614c7c2675fe027f73d60428e13cbfe3ec6610330376847', -1
WHERE EXISTS (SELECT 1 FROM pragma_table_info('user_library') WHERE name = 'finished_at');

INSERT OR REPLACE INTO _sqlx_migrations (version, description, success, checksum, execution_time)
SELECT 5, 'tts chunk meta', 1, x'a939bb0c9a2a466753a65ec001e211d68896314b5d577499562576eff173734231462cf4285df87dd8a5e50fabaa1be7', -1
WHERE EXISTS (SELECT 1 FROM pragma_table_info('tts_chunks') WHERE name = 'duration_sec');

SELECT version, description FROM _sqlx_migrations ORDER BY version;
