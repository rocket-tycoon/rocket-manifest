-- Add priority field for ordering features within a parent.
-- Lower values appear first. Default 0 for existing features.

ALTER TABLE features ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;

-- Index for efficient ordering when listing children
CREATE INDEX idx_features_parent_priority ON features(parent_id, priority);
