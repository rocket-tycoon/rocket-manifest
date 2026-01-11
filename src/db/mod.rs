mod schema;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::models::*;

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(path: PathBuf) -> Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Database path has no parent directory"))?;
        std::fs::create_dir_all(parent)?;
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_default() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("", "", "legion")
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        let db_path = dirs.data_dir().join("legion.db");
        Self::open(db_path)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().expect("database lock poisoned");
        schema::run_migrations(&conn)
    }

    // ============================================================
    // Project operations
    // ============================================================

    pub fn get_all_projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, description, instructions, created_at, updated_at
             FROM projects ORDER BY name",
        )?;

        let projects = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    name: row.get(1)?,
                    description: row.get(2)?,
                    instructions: row.get(3)?,
                    created_at: parse_datetime(row.get::<_, String>(4)?),
                    updated_at: parse_datetime(row.get::<_, String>(5)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(projects)
    }

    pub fn get_project(&self, id: Uuid) -> Result<Option<Project>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, description, instructions, created_at, updated_at
             FROM projects WHERE id = ?",
        )?;

        let mut rows = stmt.query([id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Project {
                id: parse_uuid(row.get::<_, String>(0)?),
                name: row.get(1)?,
                description: row.get(2)?,
                instructions: row.get(3)?,
                created_at: parse_datetime(row.get::<_, String>(4)?),
                updated_at: parse_datetime(row.get::<_, String>(5)?),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_project(&self, input: CreateProjectInput) -> Result<Project> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO projects (id, name, description, instructions, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            (
                id.to_string(),
                &input.name,
                &input.description,
                &input.instructions,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ),
        )?;

        Ok(Project {
            id,
            name: input.name,
            description: input.description,
            instructions: input.instructions,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_project(&self, id: Uuid, input: UpdateProjectInput) -> Result<Option<Project>> {
        let Some(existing) = self.get_project(id)? else {
            return Ok(None);
        };

        let conn = self.conn.lock().expect("database lock poisoned");
        let now = Utc::now();
        let name = input.name.unwrap_or(existing.name);
        let description = input.description.or(existing.description);
        let instructions = input.instructions.or(existing.instructions);

        conn.execute(
            "UPDATE projects SET name = ?, description = ?, instructions = ?, updated_at = ? WHERE id = ?",
            (
                &name,
                &description,
                &instructions,
                now.to_rfc3339(),
                id.to_string(),
            ),
        )?;

        Ok(Some(Project {
            id,
            name,
            description,
            instructions,
            created_at: existing.created_at,
            updated_at: now,
        }))
    }

    pub fn delete_project(&self, id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let rows = conn.execute("DELETE FROM projects WHERE id = ?", [id.to_string()])?;
        Ok(rows > 0)
    }

    // ============================================================
    // Project Directory operations
    // ============================================================

    pub fn get_project_directories(&self, project_id: Uuid) -> Result<Vec<ProjectDirectory>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, path, git_remote, is_primary, instructions, created_at
             FROM project_directories WHERE project_id = ? ORDER BY is_primary DESC, path",
        )?;

        let dirs = stmt
            .query_map([project_id.to_string()], |row| {
                Ok(ProjectDirectory {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    project_id: parse_uuid(row.get::<_, String>(1)?),
                    path: row.get(2)?,
                    git_remote: row.get(3)?,
                    is_primary: row.get::<_, i32>(4)? != 0,
                    instructions: row.get(5)?,
                    created_at: parse_datetime(row.get::<_, String>(6)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dirs)
    }

    pub fn add_project_directory(
        &self,
        project_id: Uuid,
        input: AddDirectoryInput,
    ) -> Result<ProjectDirectory> {
        self.get_project(project_id)?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

        let conn = self.conn.lock().expect("database lock poisoned");
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO project_directories (id, project_id, path, git_remote, is_primary, instructions, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                id.to_string(),
                project_id.to_string(),
                &input.path,
                &input.git_remote,
                if input.is_primary { 1 } else { 0 },
                &input.instructions,
                now.to_rfc3339(),
            ),
        )?;

        Ok(ProjectDirectory {
            id,
            project_id,
            path: input.path,
            git_remote: input.git_remote,
            is_primary: input.is_primary,
            instructions: input.instructions,
            created_at: now,
        })
    }

    pub fn remove_project_directory(&self, id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let rows = conn.execute(
            "DELETE FROM project_directories WHERE id = ?",
            [id.to_string()],
        )?;
        Ok(rows > 0)
    }

    pub fn get_project_with_directories(&self, id: Uuid) -> Result<Option<ProjectWithDirectories>> {
        let project = match self.get_project(id)? {
            Some(p) => p,
            None => return Ok(None),
        };

        let directories = self.get_project_directories(id)?;

        Ok(Some(ProjectWithDirectories {
            project,
            directories,
        }))
    }

    /// Find a project by a directory path.
    ///
    /// Returns the project and matching directory if the path matches exactly,
    /// or if the path is a subdirectory of a registered project directory.
    pub fn get_project_by_directory(&self, path: &str) -> Result<Option<ProjectWithDirectories>> {
        let conn = self.conn.lock().expect("database lock poisoned");

        // Get all directories ordered by path length (longest first for best match)
        let mut stmt = conn.prepare(
            "SELECT project_id, path FROM project_directories ORDER BY length(path) DESC",
        )?;

        let mut rows = stmt.query([])?;
        let mut found_project_id = None;

        while let Some(row) = rows.next()? {
            let dir_path: String = row.get(1)?;
            // Check exact match or subdirectory match
            if path == dir_path || path.starts_with(&format!("{}/", dir_path)) {
                found_project_id = Some(row.get::<_, String>(0)?);
                break;
            }
        }

        drop(rows);
        drop(stmt);
        drop(conn);

        match found_project_id {
            Some(id) => self.get_project_with_directories(parse_uuid(id)),
            None => Ok(None),
        }
    }

    // ============================================================
    // Feature operations
    // ============================================================

    pub fn get_all_features(&self) -> Result<Vec<Feature>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at
             FROM features ORDER BY priority, title",
        )?;

        let features = stmt
            .query_map([], |row| {
                Ok(Feature {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    project_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    title: row.get(3)?,
                    story: row.get(4)?,
                    details: row.get(5)?,
                    state: FeatureState::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(FeatureState::Proposed),
                    priority: row.get(7)?,
                    created_at: parse_datetime(row.get::<_, String>(8)?),
                    updated_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(features)
    }

    pub fn get_features_by_project(&self, project_id: Uuid) -> Result<Vec<Feature>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at
             FROM features WHERE project_id = ? ORDER BY priority, title",
        )?;

        let features = stmt
            .query_map([project_id.to_string()], |row| {
                Ok(Feature {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    project_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    title: row.get(3)?,
                    story: row.get(4)?,
                    details: row.get(5)?,
                    state: FeatureState::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(FeatureState::Proposed),
                    priority: row.get(7)?,
                    created_at: parse_datetime(row.get::<_, String>(8)?),
                    updated_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(features)
    }

    pub fn get_feature(&self, id: Uuid) -> Result<Option<Feature>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at
             FROM features WHERE id = ?",
        )?;

        let mut rows = stmt.query([id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Feature {
                id: parse_uuid(row.get::<_, String>(0)?),
                project_id: parse_uuid(row.get::<_, String>(1)?),
                parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                title: row.get(3)?,
                story: row.get(4)?,
                details: row.get(5)?,
                state: FeatureState::from_str(&row.get::<_, String>(6)?)
                    .unwrap_or(FeatureState::Proposed),
                priority: row.get(7)?,
                created_at: parse_datetime(row.get::<_, String>(8)?),
                updated_at: parse_datetime(row.get::<_, String>(9)?),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_feature(&self, project_id: Uuid, input: CreateFeatureInput) -> Result<Feature> {
        // Verify project exists
        self.get_project(project_id)?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

        let conn = self.conn.lock().expect("database lock poisoned");
        let id = Uuid::new_v4();
        let now = Utc::now();
        let state = input.state.unwrap_or(FeatureState::Proposed);
        let priority = input.priority.unwrap_or(0);

        conn.execute(
            "INSERT INTO features (id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                id.to_string(),
                project_id.to_string(),
                input.parent_id.map(|u| u.to_string()),
                &input.title,
                &input.story,
                &input.details,
                state.as_str(),
                priority,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ),
        )?;

        Ok(Feature {
            id,
            project_id,
            parent_id: input.parent_id,
            title: input.title,
            story: input.story,
            details: input.details,
            state,
            priority,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_feature(&self, id: Uuid, input: UpdateFeatureInput) -> Result<Option<Feature>> {
        let Some(existing) = self.get_feature(id)? else {
            return Ok(None);
        };

        let conn = self.conn.lock().expect("database lock poisoned");
        let now = Utc::now();
        let title = input.title.unwrap_or(existing.title);
        let story = input.story.or(existing.story);
        let details = input.details.or(existing.details);
        let state = input.state.unwrap_or(existing.state);
        let parent_id = input.parent_id.or(existing.parent_id);
        let priority = input.priority.unwrap_or(existing.priority);

        conn.execute(
            "UPDATE features SET parent_id = ?, title = ?, story = ?, details = ?, state = ?, priority = ?, updated_at = ? WHERE id = ?",
            (
                parent_id.map(|u| u.to_string()),
                &title,
                &story,
                &details,
                state.as_str(),
                priority,
                now.to_rfc3339(),
                id.to_string(),
            ),
        )?;

        Ok(Some(Feature {
            id,
            project_id: existing.project_id,
            parent_id,
            title,
            story,
            details,
            state,
            priority,
            created_at: existing.created_at,
            updated_at: now,
        }))
    }

    pub fn delete_feature(&self, id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let rows = conn.execute("DELETE FROM features WHERE id = ?", [id.to_string()])?;
        Ok(rows > 0)
    }

    pub fn get_root_features(&self, project_id: Uuid) -> Result<Vec<Feature>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at
             FROM features WHERE project_id = ? AND parent_id IS NULL ORDER BY priority, title",
        )?;

        let features = stmt
            .query_map([project_id.to_string()], |row| {
                Ok(Feature {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    project_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: None,
                    title: row.get(3)?,
                    story: row.get(4)?,
                    details: row.get(5)?,
                    state: FeatureState::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(FeatureState::Proposed),
                    priority: row.get(7)?,
                    created_at: parse_datetime(row.get::<_, String>(8)?),
                    updated_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(features)
    }

    pub fn get_children(&self, parent_id: Uuid) -> Result<Vec<Feature>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, project_id, parent_id, title, story, details, state, priority, created_at, updated_at
             FROM features WHERE parent_id = ? ORDER BY priority, title",
        )?;

        let features = stmt
            .query_map([parent_id.to_string()], |row| {
                Ok(Feature {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    project_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    title: row.get(3)?,
                    story: row.get(4)?,
                    details: row.get(5)?,
                    state: FeatureState::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(FeatureState::Proposed),
                    priority: row.get(7)?,
                    created_at: parse_datetime(row.get::<_, String>(8)?),
                    updated_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(features)
    }

    pub fn is_leaf(&self, feature_id: Uuid) -> Result<bool> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM features WHERE parent_id = ?",
            [feature_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count == 0)
    }

    pub fn get_feature_tree(&self, project_id: Uuid) -> Result<Vec<FeatureTreeNode>> {
        let features = self.get_features_by_project(project_id)?;

        // Group features by parent_id
        let mut children_map: std::collections::HashMap<Option<Uuid>, Vec<Feature>> =
            std::collections::HashMap::new();
        for feature in features {
            children_map
                .entry(feature.parent_id)
                .or_default()
                .push(feature);
        }

        // Recursively build tree starting from roots (parent_id = None)
        fn build_subtree(
            parent_id: Option<Uuid>,
            children_map: &std::collections::HashMap<Option<Uuid>, Vec<Feature>>,
        ) -> Vec<FeatureTreeNode> {
            children_map
                .get(&parent_id)
                .map(|features| {
                    features
                        .iter()
                        .map(|f| FeatureTreeNode {
                            feature: f.clone(),
                            children: build_subtree(Some(f.id), children_map),
                        })
                        .collect()
                })
                .unwrap_or_default()
        }

        Ok(build_subtree(None, &children_map))
    }

    // ============================================================
    // Session operations
    // ============================================================

    pub fn get_session(&self, id: Uuid) -> Result<Option<Session>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, goal, status, created_at, completed_at
             FROM sessions WHERE id = ?",
        )?;

        let mut rows = stmt.query([id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Session {
                id: parse_uuid(row.get::<_, String>(0)?),
                feature_id: parse_uuid(row.get::<_, String>(1)?),
                goal: row.get(2)?,
                status: SessionStatus::from_str(&row.get::<_, String>(3)?)
                    .unwrap_or(SessionStatus::Active),
                created_at: parse_datetime(row.get::<_, String>(4)?),
                completed_at: row.get::<_, Option<String>>(5)?.map(parse_datetime),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn create_session(&self, input: CreateSessionInput) -> Result<SessionResponse> {
        self.get_feature(input.feature_id)?
            .ok_or_else(|| anyhow::anyhow!("Feature not found"))?;

        // Sessions can only be created on leaf features (no children)
        if !self.is_leaf(input.feature_id)? {
            anyhow::bail!("Sessions can only be created on leaf features");
        }

        let conn = self.conn.lock().expect("database lock poisoned");
        let session_id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO sessions (id, feature_id, goal, status, created_at)
             VALUES (?, ?, ?, 'active', ?)",
            (
                session_id.to_string(),
                input.feature_id.to_string(),
                &input.goal,
                now.to_rfc3339(),
            ),
        )?;

        let session = Session {
            id: session_id,
            feature_id: input.feature_id,
            goal: input.goal,
            status: SessionStatus::Active,
            created_at: now,
            completed_at: None,
        };

        // Create tasks
        let mut tasks = Vec::new();
        for task_input in input.tasks {
            let task_id = Uuid::new_v4();

            conn.execute(
                "INSERT INTO tasks (id, session_id, parent_id, title, scope, status, agent_type, created_at)
                 VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)",
                (
                    task_id.to_string(),
                    session_id.to_string(),
                    task_input.parent_id.map(|u| u.to_string()),
                    &task_input.title,
                    &task_input.scope,
                    task_input.agent_type.as_str(),
                    now.to_rfc3339(),
                ),
            )?;

            tasks.push(Task {
                id: task_id,
                session_id,
                parent_id: task_input.parent_id,
                title: task_input.title,
                scope: task_input.scope,
                status: TaskStatus::Pending,
                agent_type: task_input.agent_type,
                worktree_path: None,
                branch: None,
                created_at: now,
            });
        }

        Ok(SessionResponse { session, tasks })
    }

    pub fn get_session_status(&self, id: Uuid) -> Result<Option<SessionStatusResponse>> {
        let session = match self.get_session(id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        let feature = self
            .get_feature(session.feature_id)?
            .ok_or_else(|| anyhow::anyhow!("Feature not found"))?;

        let tasks = self.get_tasks_by_session(id)?;

        Ok(Some(SessionStatusResponse {
            session,
            feature: SessionFeatureSummary {
                id: feature.id,
                title: feature.title,
            },
            tasks,
        }))
    }

    pub fn complete_session(
        &self,
        id: Uuid,
        input: CompleteSessionInput,
    ) -> Result<Option<SessionCompletionResult>> {
        let session = match self.get_session(id)? {
            Some(s) => s,
            None => return Ok(None),
        };

        if session.status != SessionStatus::Active {
            anyhow::bail!("Session is not active");
        }

        // Create history entry with structured details
        let history_entry = self.create_history_entry(CreateHistoryInput {
            feature_id: session.feature_id,
            session_id: Some(id),
            details: HistoryDetails {
                summary: input.summary,
                author: input.author,
                files_changed: input.files_changed,
                commits: input.commits,
            },
        })?;

        // Delete tasks
        let conn = self.conn.lock().expect("database lock poisoned");
        conn.execute("DELETE FROM tasks WHERE session_id = ?", [id.to_string()])?;

        // Update session status
        let now = Utc::now();
        conn.execute(
            "UPDATE sessions SET status = 'completed', completed_at = ? WHERE id = ?",
            (now.to_rfc3339(), id.to_string()),
        )?;

        // Update feature state if provided
        if let Some(state) = input.feature_state {
            conn.execute(
                "UPDATE features SET state = ?, updated_at = ? WHERE id = ?",
                (
                    state.as_str(),
                    now.to_rfc3339(),
                    session.feature_id.to_string(),
                ),
            )?;
        }

        let completed_session = Session {
            id: session.id,
            feature_id: session.feature_id,
            goal: session.goal,
            status: SessionStatus::Completed,
            created_at: session.created_at,
            completed_at: Some(now),
        };

        Ok(Some(SessionCompletionResult {
            session: completed_session,
            history_entry,
        }))
    }

    // ============================================================
    // Task operations
    // ============================================================

    pub fn get_task(&self, id: Uuid) -> Result<Option<Task>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, parent_id, title, scope, status, agent_type, worktree_path, branch, created_at
             FROM tasks WHERE id = ?"
        )?;

        let mut rows = stmt.query([id.to_string()])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Task {
                id: parse_uuid(row.get::<_, String>(0)?),
                session_id: parse_uuid(row.get::<_, String>(1)?),
                parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                title: row.get(3)?,
                scope: row.get(4)?,
                status: TaskStatus::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(TaskStatus::Pending),
                agent_type: AgentType::from_str(&row.get::<_, String>(6)?)
                    .unwrap_or(AgentType::Claude),
                worktree_path: row.get(7)?,
                branch: row.get(8)?,
                created_at: parse_datetime(row.get::<_, String>(9)?),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_tasks_by_session(&self, session_id: Uuid) -> Result<Vec<Task>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, parent_id, title, scope, status, agent_type, worktree_path, branch, created_at
             FROM tasks WHERE session_id = ? ORDER BY created_at"
        )?;

        let tasks = stmt
            .query_map([session_id.to_string()], |row| {
                Ok(Task {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    session_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    title: row.get(3)?,
                    scope: row.get(4)?,
                    status: TaskStatus::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or(TaskStatus::Pending),
                    agent_type: AgentType::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(AgentType::Claude),
                    worktree_path: row.get(7)?,
                    branch: row.get(8)?,
                    created_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn get_task_children(&self, parent_id: Uuid) -> Result<Vec<Task>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, session_id, parent_id, title, scope, status, agent_type, worktree_path, branch, created_at
             FROM tasks WHERE parent_id = ? ORDER BY created_at"
        )?;

        let tasks = stmt
            .query_map([parent_id.to_string()], |row| {
                Ok(Task {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    session_id: parse_uuid(row.get::<_, String>(1)?),
                    parent_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    title: row.get(3)?,
                    scope: row.get(4)?,
                    status: TaskStatus::from_str(&row.get::<_, String>(5)?)
                        .unwrap_or(TaskStatus::Pending),
                    agent_type: AgentType::from_str(&row.get::<_, String>(6)?)
                        .unwrap_or(AgentType::Claude),
                    worktree_path: row.get(7)?,
                    branch: row.get(8)?,
                    created_at: parse_datetime(row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn create_task(&self, session_id: Uuid, input: CreateTaskInput) -> Result<Task> {
        // Verify session exists and is active
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

        if session.status != SessionStatus::Active {
            anyhow::bail!("Cannot add tasks to a completed session");
        }

        let conn = self.conn.lock().expect("database lock poisoned");
        let id = Uuid::new_v4();
        let now = Utc::now();

        conn.execute(
            "INSERT INTO tasks (id, session_id, parent_id, title, scope, status, agent_type, created_at)
             VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)",
            (
                id.to_string(),
                session_id.to_string(),
                input.parent_id.map(|u| u.to_string()),
                &input.title,
                &input.scope,
                input.agent_type.as_str(),
                now.to_rfc3339(),
            ),
        )?;

        Ok(Task {
            id,
            session_id,
            parent_id: input.parent_id,
            title: input.title,
            scope: input.scope,
            status: TaskStatus::Pending,
            agent_type: input.agent_type,
            worktree_path: None,
            branch: None,
            created_at: now,
        })
    }

    pub fn update_task(&self, id: Uuid, input: UpdateTaskInput) -> Result<bool> {
        let conn = self.conn.lock().expect("database lock poisoned");

        let mut updates = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = input.status {
            updates.push("status = ?");
            params.push(Box::new(status.as_str().to_string()));
        }
        if let Some(worktree_path) = input.worktree_path {
            updates.push("worktree_path = ?");
            params.push(Box::new(worktree_path));
        }
        if let Some(branch) = input.branch {
            updates.push("branch = ?");
            params.push(Box::new(branch));
        }

        if updates.is_empty() {
            return Ok(false);
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE tasks SET {} WHERE id = ?", updates.join(", "));
        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = conn.execute(&sql, params_ref.as_slice())?;

        Ok(rows > 0)
    }

    // ============================================================
    // Feature History operations
    // ============================================================

    pub fn create_history_entry(&self, input: CreateHistoryInput) -> Result<FeatureHistory> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let id = Uuid::new_v4();
        let now = Utc::now();

        let details_json = serde_json::to_string(&input.details)?;

        // Write to both old columns (for backwards compat) and new details column
        conn.execute(
            "INSERT INTO feature_history (id, feature_id, session_id, summary, files_changed, author, details, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                id.to_string(),
                input.feature_id.to_string(),
                input.session_id.map(|u| u.to_string()),
                &input.details.summary,
                serde_json::to_string(&input.details.files_changed)?,
                &input.details.author,
                &details_json,
                now.to_rfc3339(),
            ),
        )?;

        Ok(FeatureHistory {
            id,
            feature_id: input.feature_id,
            session_id: input.session_id,
            details: input.details,
            created_at: now,
        })
    }

    pub fn get_feature_history(&self, feature_id: Uuid) -> Result<Vec<FeatureHistory>> {
        let conn = self.conn.lock().expect("database lock poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, feature_id, session_id, details, created_at
             FROM feature_history WHERE feature_id = ? ORDER BY created_at DESC",
        )?;

        let entries = stmt
            .query_map([feature_id.to_string()], |row| {
                let details_json: String = row.get(3)?;
                let details: HistoryDetails =
                    serde_json::from_str(&details_json).unwrap_or_default();

                Ok(FeatureHistory {
                    id: parse_uuid(row.get::<_, String>(0)?),
                    feature_id: parse_uuid(row.get::<_, String>(1)?),
                    session_id: row.get::<_, Option<String>>(2)?.map(parse_uuid),
                    details,
                    created_at: parse_datetime(row.get::<_, String>(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}

fn parse_uuid(s: String) -> Uuid {
    Uuid::parse_str(&s).unwrap_or_else(|_| Uuid::nil())
}

fn parse_datetime(s: String) -> chrono::DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
