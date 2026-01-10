use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An append-only log entry recording work done on a feature.
///
/// Feature history is like `git log` for a feature—it records what was done
/// during each implementation session. This is **not** version control for
/// the feature content itself (which is mutable); rather, it answers
/// "what work was done on this feature and when?"
///
/// History entries are typically created automatically when a session completes,
/// summarizing the tasks that were completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureHistory {
    pub id: Uuid,
    pub feature_id: Uuid,
    /// The session that generated this history entry, if any.
    pub session_id: Option<Uuid>,
    /// Summary of the work done.
    pub summary: String,
    /// Files that were changed during this work.
    pub files_changed: Vec<String>,
    /// Who did the work (agent type or human name).
    pub author: String,
    pub created_at: DateTime<Utc>,
}

/// Input for manually creating a history entry.
///
/// Typically history entries are created automatically when sessions complete,
/// but this input allows manual creation for migrations or special cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHistoryInput {
    pub feature_id: Uuid,
    pub session_id: Option<Uuid>,
    pub summary: String,
    pub files_changed: Vec<String>,
    pub author: String,
}
