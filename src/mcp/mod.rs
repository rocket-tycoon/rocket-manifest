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
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                r#"RocketManifest manages feature implementation sessions and tasks.

AGENT WORKFLOW (when assigned a task_id):
1. Call get_task_context with your task_id to understand your assignment
2. Call start_task to signal you're beginning work
3. Implement the task scope - write code, run tests, verify
4. Call complete_task when done and verified

ORCHESTRATOR WORKFLOW (when managing a feature):
1. Call create_session on a leaf feature to start work
2. Call create_task to break down work into agent-sized units
3. Spawn agents with their task_ids
4. Call list_session_tasks to monitor progress
5. Call complete_session when all tasks are done (marks feature as 'implemented' by default)

IMPORTANT:
- Read feature details carefully before coding
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
