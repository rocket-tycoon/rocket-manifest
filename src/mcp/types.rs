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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFeaturesRequest {
    #[schemars(description = "Optional project UUID to filter features by project")]
    pub project_id: Option<String>,
    #[schemars(
        description = "Optional state filter: 'proposed', 'specified', 'implemented', or 'deprecated'"
    )]
    pub state: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFeatureRequest {
    #[schemars(description = "The UUID of the feature to retrieve")]
    pub feature_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProjectContextRequest {
    #[schemars(
        description = "The directory path to look up (e.g., current working directory). Returns the project that contains this directory."
    )]
    pub directory_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateFeatureStateRequest {
    #[schemars(description = "The UUID of the feature to update")]
    pub feature_id: String,
    #[schemars(
        description = "The new state: 'proposed', 'specified', 'implemented', or 'deprecated'"
    )]
    pub state: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateProjectRequest {
    #[schemars(description = "The project name (e.g., 'RocketShip', 'MyApp')")]
    pub name: String,
    #[schemars(description = "Optional description of the project")]
    #[serde(default)]
    pub description: Option<String>,
    #[schemars(
        description = "Optional project-wide instructions for AI agents (coding guidelines, conventions)"
    )]
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddProjectDirectoryRequest {
    #[schemars(description = "The UUID of the project to add this directory to")]
    pub project_id: String,
    #[schemars(description = "Absolute path to the directory (e.g., '/Users/me/projects/myapp')")]
    pub path: String,
    #[schemars(description = "Optional git remote URL (e.g., 'git@github.com:org/repo.git')")]
    #[serde(default)]
    pub git_remote: Option<String>,
    #[schemars(
        description = "Whether this is the primary directory for the project. Defaults to false."
    )]
    #[serde(default)]
    pub is_primary: bool,
    #[schemars(
        description = "Optional directory-specific instructions (build commands, test commands)"
    )]
    #[serde(default)]
    pub instructions: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFeatureRequest {
    #[schemars(description = "The UUID of the project this feature belongs to")]
    pub project_id: String,
    #[schemars(description = "Optional parent feature UUID for hierarchical features")]
    #[serde(default)]
    pub parent_id: Option<String>,
    #[schemars(description = "Short title for the feature (e.g., 'User Authentication')")]
    pub title: String,
    #[schemars(
        description = "Optional user story in 'As a... I want... So that...' format"
    )]
    #[serde(default)]
    pub story: Option<String>,
    #[schemars(description = "Optional implementation details and technical notes")]
    #[serde(default)]
    pub details: Option<String>,
    #[schemars(
        description = "Initial state: 'proposed' (default), 'specified', 'implemented', or 'deprecated'"
    )]
    #[serde(default = "default_proposed")]
    pub state: String,
    #[schemars(
        description = "Priority for ordering within parent. Lower values appear first. Defaults to 0."
    )]
    #[serde(default)]
    pub priority: Option<i32>,
}

fn default_proposed() -> String {
    "proposed".to_string()
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
    /// Priority for ordering within parent. Lower values appear first.
    pub priority: i32,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FeatureListResponse {
    pub features: Vec<FeatureInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectContextResponse {
    pub project: ProjectInfo,
    pub directory: DirectoryInfo,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Project-wide instructions for AI agents (coding guidelines, conventions).
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DirectoryInfo {
    pub id: String,
    pub path: String,
    pub git_remote: Option<String>,
    pub is_primary: bool,
    /// Directory-specific instructions (build commands, test commands).
    pub instructions: Option<String>,
}
