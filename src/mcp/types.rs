//! Request and response types for MCP tools.

use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================
// Request Types
// ============================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTaskContextRequest {
    #[schemars(description = "The UUID of the task assigned to you")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartTaskRequest {
    #[schemars(description = "The UUID of the task to start working on")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteTaskRequest {
    #[schemars(description = "The UUID of the task to mark as complete")]
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSessionRequest {
    #[schemars(
        description = "The UUID of the feature to start a session on (must be a leaf feature with no children)"
    )]
    pub feature_id: String,
    #[schemars(
        description = "The goal of this session - what will be accomplished when the session ends"
    )]
    pub goal: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTaskRequest {
    #[schemars(description = "The UUID of the session to create the task in")]
    pub session_id: String,
    #[schemars(description = "Short title describing what this task accomplishes")]
    pub title: String,
    #[schemars(
        description = "Detailed scope of work - be specific about what to implement, test, or verify"
    )]
    pub scope: String,
    #[schemars(
        description = "Which agent type should handle this task: 'claude', 'gemini', or 'codex'"
    )]
    pub agent_type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSessionTasksRequest {
    #[schemars(description = "The UUID of the session to list tasks for")]
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteSessionRequest {
    #[schemars(description = "The UUID of the session to complete")]
    pub session_id: String,
    #[schemars(
        description = "Summary of work done during this session - becomes the feature history entry"
    )]
    pub summary: String,
    #[schemars(description = "Files that were changed during this session")]
    #[serde(default)]
    pub files_changed: Vec<String>,
    #[schemars(description = "Git commits created during this session")]
    #[serde(default)]
    pub commits: Vec<CommitRefInput>,
    #[schemars(
        description = "Whether to mark the feature as 'implemented'. Defaults to true. Set to false if work is partial or feature needs more sessions."
    )]
    #[serde(default = "default_true")]
    pub mark_implemented: bool,
}

/// A reference to a git commit for MCP input.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommitRefInput {
    #[schemars(description = "The commit SHA (short or full)")]
    pub sha: String,
    #[schemars(description = "The commit message (first line)")]
    pub message: String,
    #[schemars(description = "The commit author, if different from the session author")]
    #[serde(default)]
    pub author: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============================================================
// Response Types
// ============================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskContextResponse {
    /// The task you are assigned to complete
    pub task: TaskInfo,
    /// The feature this task implements
    pub feature: FeatureInfo,
    /// The session goal describing the overall objective
    pub session_goal: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub status: String,
    pub agent_type: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FeatureInfo {
    pub id: String,
    pub title: String,
    pub story: Option<String>,
    pub details: Option<String>,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub id: String,
    pub feature_id: String,
    pub goal: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TaskListResponse {
    pub session_id: String,
    pub tasks: Vec<TaskInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CompleteSessionResponse {
    pub session_id: String,
    pub feature_id: String,
    pub feature_state: String,
    pub history_entry_id: String,
}
