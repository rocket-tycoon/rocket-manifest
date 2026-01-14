-- Initial schema for Manifest
-- Creates all tables for feature documentation system

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE project_directories (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    git_remote TEXT,
    is_primary INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE features (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES features(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    story TEXT,
    details TEXT,
    state TEXT NOT NULL DEFAULT 'proposed' CHECK (state IN ('proposed', 'specified', 'implemented', 'deprecated')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE feature_history (
    id TEXT PRIMARY KEY,
    feature_id TEXT REFERENCES features(id) ON DELETE CASCADE,
    session_id TEXT,
    summary TEXT NOT NULL,
    files_changed JSON,
    author TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    feature_id TEXT REFERENCES features(id) ON DELETE CASCADE,
    goal TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'failed')),
    feature_version_before INTEGER,
    feature_version_after INTEGER,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    scope TEXT,
    status TEXT DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    agent_type TEXT CHECK (agent_type IN ('claude', 'gemini', 'codex')),
    worktree_path TEXT,
    branch TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE implementation_notes (
    id TEXT PRIMARY KEY,
    feature_id TEXT REFERENCES features(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    files_changed JSON,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_project_directories_project ON project_directories(project_id);
CREATE INDEX idx_features_project ON features(project_id);
CREATE INDEX idx_features_parent ON features(parent_id);
CREATE INDEX idx_sessions_feature ON sessions(feature_id);
CREATE INDEX idx_tasks_session ON tasks(session_id);
CREATE INDEX idx_tasks_parent ON tasks(parent_id);
CREATE INDEX idx_history_feature ON feature_history(feature_id);

-- Only one active session per feature at a time
CREATE UNIQUE INDEX idx_one_active_session
    ON sessions(feature_id) WHERE status = 'active';
