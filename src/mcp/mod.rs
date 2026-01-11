//! MCP server for AI-assisted feature development.

mod types;

use std::str::FromStr;

pub use types::*;

use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use uuid::Uuid;

use crate::db::Database;
use crate::models::*;

#[derive(Clone)]
pub struct McpServer {
    db: Database,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }

    fn parse_uuid(s: &str) -> Result<Uuid, McpError> {
        Uuid::parse_str(s)
            .map_err(|e| McpError::invalid_params(format!("Invalid UUID: {}", e), None))
    }

    // ============================================================
    // Test helpers - expose tool logic for testing
    // ============================================================

    pub async fn test_get_task_context(
        &self,
        task_id: &str,
    ) -> Result<TaskContextResponse, McpError> {
        let task_id = Self::parse_uuid(task_id)?;

        let task = self
            .db
            .get_task(task_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Task not found", None))?;

        let session = self
            .db
            .get_session(task.session_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::internal_error("Session not found", None))?;

        let feature = self
            .db
            .get_feature(session.feature_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::internal_error("Feature not found", None))?;

        Ok(TaskContextResponse {
            task: TaskInfo {
                id: task.id.to_string(),
                title: task.title,
                scope: task.scope,
                status: task.status.as_str().to_string(),
                agent_type: task.agent_type.as_str().to_string(),
            },
            feature: FeatureInfo {
                id: feature.id.to_string(),
                title: feature.title,
                story: feature.story,
                details: feature.details,
                state: feature.state.as_str().to_string(),
                priority: feature.priority,
            },
            session_goal: session.goal,
        })
    }

    pub fn test_start_task(&self, task_id: &str) -> Result<(), McpError> {
        let task_id = Self::parse_uuid(task_id)?;
        let updated = self
            .db
            .update_task(
                task_id,
                UpdateTaskInput {
                    status: Some(TaskStatus::Running),
                    worktree_path: None,
                    branch: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if !updated {
            return Err(McpError::invalid_params("Task not found", None));
        }
        Ok(())
    }

    pub fn test_complete_task(&self, task_id: &str) -> Result<(), McpError> {
        let task_id = Self::parse_uuid(task_id)?;
        let updated = self
            .db
            .update_task(
                task_id,
                UpdateTaskInput {
                    status: Some(TaskStatus::Completed),
                    worktree_path: None,
                    branch: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if !updated {
            return Err(McpError::invalid_params("Task not found", None));
        }
        Ok(())
    }

    pub fn test_create_session(
        &self,
        feature_id: &str,
        goal: &str,
    ) -> Result<SessionInfo, McpError> {
        let feature_id = Self::parse_uuid(feature_id)?;
        let response = self
            .db
            .create_session(CreateSessionInput {
                feature_id,
                goal: goal.to_string(),
                tasks: vec![],
            })
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(SessionInfo {
            id: response.session.id.to_string(),
            feature_id: response.session.feature_id.to_string(),
            goal: response.session.goal,
            status: response.session.status.as_str().to_string(),
        })
    }

    pub fn test_create_task(
        &self,
        session_id: &str,
        title: &str,
        scope: &str,
        agent_type: &str,
    ) -> Result<TaskInfo, McpError> {
        let session_id = Self::parse_uuid(session_id)?;
        let agent_type = AgentType::from_str(agent_type).map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid agent_type '{}'. Must be: claude, gemini, or codex",
                    agent_type
                ),
                None,
            )
        })?;

        let task = self
            .db
            .create_task(
                session_id,
                CreateTaskInput {
                    parent_id: None,
                    title: title.to_string(),
                    scope: scope.to_string(),
                    agent_type,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(TaskInfo {
            id: task.id.to_string(),
            title: task.title,
            scope: task.scope,
            status: task.status.as_str().to_string(),
            agent_type: task.agent_type.as_str().to_string(),
        })
    }

    pub fn test_list_session_tasks(&self, session_id: &str) -> Result<TaskListResponse, McpError> {
        let session_id = Self::parse_uuid(session_id)?;
        let tasks = self
            .db
            .get_tasks_by_session(session_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(TaskListResponse {
            session_id: session_id.to_string(),
            tasks: tasks
                .into_iter()
                .map(|t| TaskInfo {
                    id: t.id.to_string(),
                    title: t.title,
                    scope: t.scope,
                    status: t.status.as_str().to_string(),
                    agent_type: t.agent_type.as_str().to_string(),
                })
                .collect(),
        })
    }

    pub fn test_complete_session(
        &self,
        session_id: &str,
        summary: &str,
        files_changed: Vec<String>,
        mark_implemented: bool,
    ) -> Result<CompleteSessionResponse, McpError> {
        let session_id = Self::parse_uuid(session_id)?;
        let feature_state = if mark_implemented {
            Some(FeatureState::Implemented)
        } else {
            None
        };

        let result = self
            .db
            .complete_session(
                session_id,
                CompleteSessionInput {
                    summary: summary.to_string(),
                    author: "session".to_string(),
                    files_changed,
                    commits: vec![],
                    feature_state,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Session not found", None))?;

        Ok(CompleteSessionResponse {
            session_id: result.session.id.to_string(),
            feature_id: result.session.feature_id.to_string(),
            feature_state: if mark_implemented {
                "implemented"
            } else {
                "unchanged"
            }
            .to_string(),
            history_entry_id: result.history_entry.id.to_string(),
        })
    }

    pub fn test_list_features(
        &self,
        project_id: Option<&str>,
        state: Option<&str>,
    ) -> Result<FeatureListResponse, McpError> {
        let features = match project_id {
            Some(pid) => {
                let project_id = Self::parse_uuid(pid)?;
                self.db
                    .get_features_by_project(project_id)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
            None => self
                .db
                .get_all_features()
                .map_err(|e| McpError::internal_error(e.to_string(), None))?,
        };

        let features: Vec<_> = match state {
            Some(state) => {
                let target_state = FeatureState::from_str(state).map_err(|_| {
                    McpError::invalid_params(format!("Invalid state '{}'", state), None)
                })?;
                features
                    .into_iter()
                    .filter(|f| f.state == target_state)
                    .collect()
            }
            None => features,
        };

        Ok(FeatureListResponse {
            features: features
                .into_iter()
                .map(|f| FeatureInfo {
                    id: f.id.to_string(),
                    title: f.title,
                    story: f.story,
                    details: f.details,
                    state: f.state.as_str().to_string(),
                    priority: f.priority,
                })
                .collect(),
        })
    }

    pub fn test_get_feature(&self, feature_id: &str) -> Result<FeatureInfo, McpError> {
        let feature_id = Self::parse_uuid(feature_id)?;
        let feature = self
            .db
            .get_feature(feature_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Feature not found", None))?;

        Ok(FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        })
    }

    pub fn test_get_project_context(
        &self,
        directory_path: &str,
    ) -> Result<ProjectContextResponse, McpError> {
        let project_with_dirs = self
            .db
            .get_project_by_directory(directory_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("No project found for directory: {}", directory_path),
                    None,
                )
            })?;

        let matching_dir = project_with_dirs
            .directories
            .iter()
            .find(|d| {
                directory_path == d.path || directory_path.starts_with(&format!("{}/", d.path))
            })
            .ok_or_else(|| McpError::internal_error("Directory match logic error", None))?;

        Ok(ProjectContextResponse {
            project: ProjectInfo {
                id: project_with_dirs.project.id.to_string(),
                name: project_with_dirs.project.name,
                description: project_with_dirs.project.description,
                instructions: project_with_dirs.project.instructions,
            },
            directory: DirectoryInfo {
                id: matching_dir.id.to_string(),
                path: matching_dir.path.clone(),
                git_remote: matching_dir.git_remote.clone(),
                is_primary: matching_dir.is_primary,
                instructions: matching_dir.instructions.clone(),
            },
        })
    }

    pub fn test_update_feature_state(
        &self,
        feature_id: &str,
        state: &str,
    ) -> Result<FeatureInfo, McpError> {
        let feature_id = Self::parse_uuid(feature_id)?;
        let new_state = FeatureState::from_str(state)
            .map_err(|_| McpError::invalid_params(format!("Invalid state '{}'", state), None))?;

        let feature = self
            .db
            .update_feature(
                feature_id,
                UpdateFeatureInput {
                    parent_id: None,
                    title: None,
                    story: None,
                    details: None,
                    state: Some(new_state),
                    priority: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Feature not found", None))?;

        Ok(FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        })
    }

    pub fn test_create_project(
        &self,
        name: &str,
        description: Option<&str>,
        instructions: Option<&str>,
    ) -> Result<ProjectInfo, McpError> {
        let project = self
            .db
            .create_project(CreateProjectInput {
                name: name.to_string(),
                description: description.map(|s| s.to_string()),
                instructions: instructions.map(|s| s.to_string()),
            })
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(ProjectInfo {
            id: project.id.to_string(),
            name: project.name,
            description: project.description,
            instructions: project.instructions,
        })
    }

    pub fn test_add_project_directory(
        &self,
        project_id: &str,
        path: &str,
        git_remote: Option<&str>,
        is_primary: bool,
        instructions: Option<&str>,
    ) -> Result<DirectoryInfo, McpError> {
        let project_id = Self::parse_uuid(project_id)?;

        let directory = self
            .db
            .add_project_directory(
                project_id,
                AddDirectoryInput {
                    path: path.to_string(),
                    git_remote: git_remote.map(|s| s.to_string()),
                    is_primary,
                    instructions: instructions.map(|s| s.to_string()),
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(DirectoryInfo {
            id: directory.id.to_string(),
            path: directory.path,
            git_remote: directory.git_remote,
            is_primary: directory.is_primary,
            instructions: directory.instructions,
        })
    }

    pub fn test_create_feature(
        &self,
        project_id: &str,
        parent_id: Option<&str>,
        title: &str,
        story: Option<&str>,
        details: Option<&str>,
        state: &str,
    ) -> Result<FeatureInfo, McpError> {
        let project_id = Self::parse_uuid(project_id)?;
        let parent_id = match parent_id {
            Some(pid) => Some(Self::parse_uuid(pid)?),
            None => None,
        };
        let state = FeatureState::from_str(state)
            .map_err(|_| McpError::invalid_params(format!("Invalid state '{}'", state), None))?;

        let feature = self
            .db
            .create_feature(
                project_id,
                CreateFeatureInput {
                    parent_id,
                    title: title.to_string(),
                    story: story.map(|s| s.to_string()),
                    details: details.map(|s| s.to_string()),
                    state: Some(state),
                    priority: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        })
    }
}

#[tool_router]
impl McpServer {
    // ============================================================
    // Agent Tools - Used by agents working on assigned tasks
    // ============================================================

    #[tool(
        description = "Retrieve your assigned task with full feature context. Call this FIRST when starting work. Returns: task details (id, title, scope, status), feature specification (title, story, details), and session goal. Use this information to understand what to implement before writing any code."
    )]
    async fn get_task_context(
        &self,
        params: Parameters<GetTaskContextRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let task_id = Self::parse_uuid(&req.task_id)?;

        let task = self
            .db
            .get_task(task_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Task not found", None))?;

        let session = self
            .db
            .get_session(task.session_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::internal_error("Session not found", None))?;

        let feature = self
            .db
            .get_feature(session.feature_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::internal_error("Feature not found", None))?;

        let context = TaskContextResponse {
            task: TaskInfo {
                id: task.id.to_string(),
                title: task.title,
                scope: task.scope,
                status: task.status.as_str().to_string(),
                agent_type: task.agent_type.as_str().to_string(),
            },
            feature: FeatureInfo {
                id: feature.id.to_string(),
                title: feature.title,
                story: feature.story,
                details: feature.details,
                state: feature.state.as_str().to_string(),
                priority: feature.priority,
            },
            session_goal: session.goal,
        };

        let json = serde_json::to_string_pretty(&context)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Signal that you are beginning work on a task. Call this AFTER get_task_context and BEFORE making any code changes. Sets task status to 'running' so the orchestrator knows work is in progress. Side effect: updates task.status to 'running'."
    )]
    async fn start_task(
        &self,
        params: Parameters<StartTaskRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let task_id = Self::parse_uuid(&req.task_id)?;

        let updated = self
            .db
            .update_task(
                task_id,
                UpdateTaskInput {
                    status: Some(TaskStatus::Running),
                    worktree_path: None,
                    branch: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if !updated {
            return Err(McpError::invalid_params("Task not found", None));
        }

        Ok(CallToolResult::success(vec![Content::text(
            "Task started - status set to 'running'",
        )]))
    }

    #[tool(
        description = "Signal that your task is finished. Call this ONLY when all work is done and verified. Before calling: ensure code compiles, tests pass, and implementation matches the task scope. After calling: your work is recorded and you should stop making changes. Side effect: updates task.status to 'completed'."
    )]
    async fn complete_task(
        &self,
        params: Parameters<CompleteTaskRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let task_id = Self::parse_uuid(&req.task_id)?;

        let updated = self
            .db
            .update_task(
                task_id,
                UpdateTaskInput {
                    status: Some(TaskStatus::Completed),
                    worktree_path: None,
                    branch: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if !updated {
            return Err(McpError::invalid_params("Task not found", None));
        }

        Ok(CallToolResult::success(vec![Content::text(
            "Task completed successfully",
        )]))
    }

    // ============================================================
    // Orchestrator Tools - Used to manage sessions and tasks
    // ============================================================

    #[tool(
        description = "Start a new implementation session on a feature. Only one active session per feature is allowed. Use this to begin work on a feature, then create tasks within the session for agents to execute. The goal should describe the overall objective. Constraint: feature must be a leaf (no children). Side effect: creates session with status 'active'."
    )]
    async fn create_session(
        &self,
        params: Parameters<CreateSessionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let feature_id = Self::parse_uuid(&req.feature_id)?;

        let response = self
            .db
            .create_session(CreateSessionInput {
                feature_id,
                goal: req.goal,
                tasks: vec![], // Create session without tasks, add them separately
            })
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = SessionInfo {
            id: response.session.id.to_string(),
            feature_id: response.session.feature_id.to_string(),
            goal: response.session.goal,
            status: response.session.status.as_str().to_string(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Create a new task within a session. Use this to break down feature work into discrete units that can be assigned to agents. Each task should be small enough for one agent to complete (1-3 story points). Include detailed scope so the agent knows exactly what to implement. Returns the created task with its ID for spawning an agent."
    )]
    async fn create_task(
        &self,
        params: Parameters<CreateTaskRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let session_id = Self::parse_uuid(&req.session_id)?;

        let agent_type = AgentType::from_str(&req.agent_type).map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid agent_type '{}'. Must be: claude, gemini, or codex",
                    req.agent_type
                ),
                None,
            )
        })?;

        let task = self
            .db
            .create_task(
                session_id,
                CreateTaskInput {
                    parent_id: None,
                    title: req.title,
                    scope: req.scope,
                    agent_type,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = TaskInfo {
            id: task.id.to_string(),
            title: task.title,
            scope: task.scope,
            status: task.status.as_str().to_string(),
            agent_type: task.agent_type.as_str().to_string(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "List all tasks in a session with their current status. Use this to monitor progress of parallel agent work. Returns array of tasks with: id, title, scope, status (pending/running/completed/failed), agent_type. Check status to know which tasks are done."
    )]
    async fn list_session_tasks(
        &self,
        params: Parameters<ListSessionTasksRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let session_id = Self::parse_uuid(&req.session_id)?;

        let tasks = self
            .db
            .get_tasks_by_session(session_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = TaskListResponse {
            session_id: session_id.to_string(),
            tasks: tasks
                .into_iter()
                .map(|t| TaskInfo {
                    id: t.id.to_string(),
                    title: t.title,
                    scope: t.scope,
                    status: t.status.as_str().to_string(),
                    agent_type: t.agent_type.as_str().to_string(),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Complete a session after all tasks are done. Call this when all tasks are completed to finalize the session. Creates a history entry summarizing the work and optionally marks the feature as 'implemented'. IMPORTANT: By default, this marks the feature as implemented. Set mark_implemented=false if the work is partial. Side effects: creates feature_history entry, deletes task records, updates session status to 'completed', optionally updates feature state to 'implemented'."
    )]
    async fn complete_session(
        &self,
        params: Parameters<CompleteSessionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let session_id = Self::parse_uuid(&req.session_id)?;

        // Determine feature state to set
        let feature_state = if req.mark_implemented {
            Some(FeatureState::Implemented)
        } else {
            None
        };

        // Convert MCP CommitRefInput to model CommitRef
        let commits = req
            .commits
            .into_iter()
            .map(|c| CommitRef {
                sha: c.sha,
                message: c.message,
                author: c.author,
            })
            .collect();

        let result = self
            .db
            .complete_session(
                session_id,
                CompleteSessionInput {
                    summary: req.summary,
                    author: "session".to_string(),
                    files_changed: req.files_changed,
                    commits,
                    feature_state,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Session not found", None))?;

        let response = CompleteSessionResponse {
            session_id: result.session.id.to_string(),
            feature_id: result.session.feature_id.to_string(),
            feature_state: if req.mark_implemented {
                "implemented"
            } else {
                "unchanged"
            }
            .to_string(),
            history_entry_id: result.history_entry.id.to_string(),
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // ============================================================
    // Discovery Tools - Browse features and projects
    // ============================================================

    #[tool(
        description = "List features, optionally filtered by project or state. Use this to discover what features exist and their current state. Returns features ordered by title. Filter by state to find features ready for work (e.g., state='specified')."
    )]
    async fn list_features(
        &self,
        params: Parameters<ListFeaturesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;

        // Get features (filtered by project if specified)
        let features = match req.project_id {
            Some(ref pid) => {
                let project_id = Self::parse_uuid(pid)?;
                self.db
                    .get_features_by_project(project_id)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
            }
            None => self
                .db
                .get_all_features()
                .map_err(|e| McpError::internal_error(e.to_string(), None))?,
        };

        // Filter by state if specified
        let features: Vec<_> = match req.state {
            Some(ref state) => {
                let target_state = FeatureState::from_str(state).map_err(|_| {
                    McpError::invalid_params(
                        format!(
                            "Invalid state '{}'. Must be: proposed, specified, implemented, or deprecated",
                            state
                        ),
                        None,
                    )
                })?;
                features
                    .into_iter()
                    .filter(|f| f.state == target_state)
                    .collect()
            }
            None => features,
        };

        let result = FeatureListResponse {
            features: features
                .into_iter()
                .map(|f| FeatureInfo {
                    id: f.id.to_string(),
                    title: f.title,
                    story: f.story,
                    details: f.details,
                    state: f.state.as_str().to_string(),
                    priority: f.priority,
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Get detailed information about a specific feature by ID. Returns the feature's title, story, implementation details, and current state. Use this before creating a session to understand what needs to be built."
    )]
    async fn get_feature(
        &self,
        params: Parameters<GetFeatureRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let feature_id = Self::parse_uuid(&req.feature_id)?;

        let feature = self
            .db
            .get_feature(feature_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Feature not found", None))?;

        let result = FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Get project context for a directory path. Given a directory (e.g., your current working directory), returns the associated project with its instructions and coding guidelines. Use this to understand project conventions before starting work."
    )]
    async fn get_project_context(
        &self,
        params: Parameters<GetProjectContextRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;

        let project_with_dirs = self
            .db
            .get_project_by_directory(&req.directory_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("No project found for directory: {}", req.directory_path),
                    None,
                )
            })?;

        // Find the matching directory
        let matching_dir = project_with_dirs
            .directories
            .iter()
            .find(|d| {
                req.directory_path == d.path
                    || req.directory_path.starts_with(&format!("{}/", d.path))
            })
            .ok_or_else(|| McpError::internal_error("Directory match logic error", None))?;

        let result = ProjectContextResponse {
            project: ProjectInfo {
                id: project_with_dirs.project.id.to_string(),
                name: project_with_dirs.project.name,
                description: project_with_dirs.project.description,
                instructions: project_with_dirs.project.instructions,
            },
            directory: DirectoryInfo {
                id: matching_dir.id.to_string(),
                path: matching_dir.path.clone(),
                git_remote: matching_dir.git_remote.clone(),
                is_primary: matching_dir.is_primary,
                instructions: matching_dir.instructions.clone(),
            },
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Update a feature's state. Use this to transition features through their lifecycle: proposed → specified → implemented → deprecated. Typically called by orchestrators after completing work or during planning."
    )]
    async fn update_feature_state(
        &self,
        params: Parameters<UpdateFeatureStateRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let feature_id = Self::parse_uuid(&req.feature_id)?;

        let new_state = FeatureState::from_str(&req.state).map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid state '{}'. Must be: proposed, specified, implemented, or deprecated",
                    req.state
                ),
                None,
            )
        })?;

        let feature = self
            .db
            .update_feature(
                feature_id,
                UpdateFeatureInput {
                    parent_id: None,
                    title: None,
                    story: None,
                    details: None,
                    state: Some(new_state),
                    priority: None,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| McpError::invalid_params("Feature not found", None))?;

        let result = FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    // ============================================================
    // Setup Tools - Create projects, directories, and features
    // ============================================================

    #[tool(
        description = "Create a new project. Projects are containers for features and can have multiple directories (e.g., monorepo subdirectories). Use this when starting work on a new codebase. After creating, use add_project_directory to associate directories."
    )]
    async fn create_project(
        &self,
        params: Parameters<CreateProjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;

        let project = self
            .db
            .create_project(CreateProjectInput {
                name: req.name,
                description: req.description,
                instructions: req.instructions,
            })
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = ProjectInfo {
            id: project.id.to_string(),
            name: project.name,
            description: project.description,
            instructions: project.instructions,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Associate a directory with a project. This enables get_project_context to find the project when given a directory path. Use after create_project. Mark one directory as is_primary=true for the main project location. Include instructions for directory-specific build/test commands."
    )]
    async fn add_project_directory(
        &self,
        params: Parameters<AddProjectDirectoryRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let project_id = Self::parse_uuid(&req.project_id)?;

        let directory = self
            .db
            .add_project_directory(
                project_id,
                AddDirectoryInput {
                    path: req.path,
                    git_remote: req.git_remote,
                    is_primary: req.is_primary,
                    instructions: req.instructions,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = DirectoryInfo {
            id: directory.id.to_string(),
            path: directory.path,
            git_remote: directory.git_remote,
            is_primary: directory.is_primary,
            instructions: directory.instructions,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(
        description = "Create a feature within a project. Features can be hierarchical (parent_id for nesting) or flat. Use 'proposed' state for ideas, 'specified' when ready for implementation. Include story (user story format) and details (technical notes) to guide implementation."
    )]
    async fn create_feature(
        &self,
        params: Parameters<CreateFeatureRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;
        let project_id = Self::parse_uuid(&req.project_id)?;
        let parent_id = match req.parent_id {
            Some(pid) => Some(Self::parse_uuid(&pid)?),
            None => None,
        };
        let state = FeatureState::from_str(&req.state).map_err(|_| {
            McpError::invalid_params(
                format!(
                    "Invalid state '{}'. Must be: proposed, specified, implemented, or deprecated",
                    req.state
                ),
                None,
            )
        })?;

        let feature = self
            .db
            .create_feature(
                project_id,
                CreateFeatureInput {
                    parent_id,
                    title: req.title,
                    story: req.story,
                    details: req.details,
                    state: Some(state),
                    priority: req.priority,
                },
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let result = FeatureInfo {
            id: feature.id.to_string(),
            title: feature.title,
            story: feature.story,
            details: feature.details,
            state: feature.state.as_str().to_string(),
            priority: feature.priority,
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: rmcp::model::Implementation {
                name: "rocket-manifest".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                icons: None,
                website_url: None,
            },
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some(
                r#"RocketManifest manages feature implementation sessions and tasks.

SETUP (one-time when starting a new project):
1. Call create_project with name, description, and coding instructions
2. Call add_project_directory to associate your codebase directory with the project
3. Call create_feature to define features to implement

FEATURE NAMING GUIDELINES:
- Name features by CAPABILITY, not by implementation phase or order
- BAD: "Phase 1: Core Routing", "Step 2: Add Validation"
- GOOD: "Router", "Request Validation", "OpenAPI Generation"
- Use the 'priority' field to indicate implementation order (lower = first)
- Features are LIVING DOCUMENTATION - they describe what the system does long-term
- Put implementation notes or phase info in 'details', not in the title

DISCOVERY (find what to work on):
- get_project_context: Given your CWD, find the project and its instructions
- list_features: Browse features, filter by project_id or state (proposed/specified/implemented/deprecated)
- get_feature: Get full details of a feature before starting work

AGENT WORKFLOW (when assigned a task_id):
1. Call get_task_context with your task_id to understand your assignment
2. Call start_task to signal you're beginning work
3. Implement the task scope - write code, run tests, verify
4. Call complete_task when done and verified

ORCHESTRATOR WORKFLOW (when managing a feature):
1. Call list_features with state='specified' to find work
2. Call get_feature to read the full specification
3. Call create_session on a leaf feature to start work
4. Call create_task to break down work into agent-sized units
5. Spawn agents with their task_ids
6. Call list_session_tasks to monitor progress
7. Call complete_session when all tasks are done
8. Call update_feature_state if needed (e.g., to 'deprecated')

IMPORTANT:
- Read feature story and details carefully before coding
- Only call complete_task when work is verified (tests pass, code compiles)
- Tasks should be small enough for one agent (1-3 story points)"#
                    .into(),
            ),
            ..Default::default()
        }
    }
}

pub async fn run_stdio_server(db: Database) -> anyhow::Result<()> {
    use tokio::io::{stdin, stdout};

    tracing::info!("Starting MCP server via stdio");

    let service = McpServer::new(db);
    let server = service.serve((stdin(), stdout())).await?;

    let quit_reason = server.waiting().await?;
    tracing::info!("MCP server stopped: {:?}", quit_reason);

    Ok(())
}
