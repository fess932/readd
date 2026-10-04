-- When the position was actually reached on the listener's device (unix ms).
-- A device that was offline may report it much later; the newest listening wins.
ALTER TABLE progress ADD COLUMN listened_at INTEGER NOT NULL DEFAULT 0;

UPDATE progress SET listened_at = CAST(strftime('%s', updated_at) AS INTEGER) * 1000;
