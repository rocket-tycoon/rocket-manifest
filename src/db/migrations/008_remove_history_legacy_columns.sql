-- Make legacy columns nullable since all data now lives in the details JSON column
-- SQLite doesn't support ALTER COLUMN, so we recreate the table

-- Create new table without NOT NULL constraints on legacy columns
CREATE TABLE feature_history_new (
    id TEXT PRIMARY KEY,
    feature_id TEXT REFERENCES features(id) ON DELETE CASCADE,
    session_id TEXT,
    summary TEXT NOT NULL,
    files_changed JSON,
    author TEXT,
    details JSON,
    created_at TEXT NOT NULL
);

-- Copy data from old table
INSERT INTO feature_history_new SELECT * FROM feature_history;

-- Drop old table and rename new one
DROP TABLE feature_history;
ALTER TABLE feature_history_new RENAME TO feature_history;
