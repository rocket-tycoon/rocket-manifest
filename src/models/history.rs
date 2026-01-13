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
    /// Structured details about the work done.
    pub details: HistoryDetails,
    pub created_at: DateTime<Utc>,
}

/// Structured details about work done in a history entry.
///
/// Stored as JSON to allow schema evolution without migrations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryDetails {
    /// Summary of the work done.
    pub summary: String,
    /// Git commits created during this work.
    #[serde(default)]
    pub commits: Vec<CommitRef>,
}

/// A reference to a git commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    /// The commit SHA (short or full).
    pub sha: String,
    /// The commit message (first line).
    pub message: String,
    /// The commit author, if different from the session author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

/// Input for creating a history entry.
///
/// Typically history entries are created automatically when sessions complete,
/// but this input allows manual creation for migrations or special cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHistoryInput {
    pub feature_id: Uuid,
    pub session_id: Option<Uuid>,
    pub details: HistoryDetails,
}
