-- Remove story field from features
-- User stories are now part of details field

-- Step 1: Migrate existing stories into details
UPDATE features
SET details = CASE
    WHEN story IS NOT NULL AND details IS NOT NULL THEN
        '## User Story' || char(10) || char(10) || story || char(10) || char(10) || '## Details' || char(10) || char(10) || details
    WHEN story IS NOT NULL AND details IS NULL THEN
        '## User Story' || char(10) || char(10) || story
    ELSE details
END
WHERE story IS NOT NULL;

-- Step 2: Create new table without story column
CREATE TABLE features_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES features_new(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    details TEXT,
    state TEXT NOT NULL DEFAULT 'proposed' CHECK (state IN ('proposed', 'specified', 'implemented', 'deprecated')),
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Step 3: Copy data (excluding story column)
INSERT INTO features_new (id, project_id, parent_id, title, details, state, priority, created_at, updated_at)
SELECT id, project_id, parent_id, title, details, state, priority, created_at, updated_at
FROM features;

-- Step 4: Drop old table and rename new
DROP TABLE features;
ALTER TABLE features_new RENAME TO features;

-- Step 5: Recreate indexes
CREATE INDEX idx_features_project ON features(project_id);
CREATE INDEX idx_features_parent ON features(parent_id);
