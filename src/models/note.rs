use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A note documenting implementation details.
///
/// Notes can be attached to either a feature (permanent documentation) or
/// a task (ephemeral, deleted with the task when the session completes).
/// They capture decisions, gotchas, or context that might be useful later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationNote {
    pub id: Uuid,
    /// The feature this note is attached to. Mutually exclusive with `task_id`.
    pub feature_id: Option<Uuid>,
    /// The task this note is attached to. Mutually exclusive with `feature_id`.
    pub task_id: Option<Uuid>,
    /// The note content (markdown supported).
    pub content: String,
    /// Files that were changed as part of this implementation.
    pub files_changed: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Input for creating an implementation note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImplementationNoteInput {
    /// The note content (markdown supported).
    pub content: String,
    /// Files that were changed as part of this implementation.
    #[serde(default)]
    pub files_changed: Vec<String>,
}
