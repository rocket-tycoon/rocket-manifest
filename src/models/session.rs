use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::feature::FeatureState;
use super::task::CreateTaskInput;

/// An active work session on a leaf feature.
///
/// Sessions are **ephemeral**—they exist only during active work. When a session
/// completes, its tasks are summarized into a [`FeatureHistory`] entry and deleted.
/// Only one session can be active on a feature at a time.
///
/// Sessions can only be created on **leaf features** (features with no children).
/// This enforces work at the appropriate level of granularity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub feature_id: Uuid,
    /// High-level objective for this work session.
    pub goal: String,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// The status of a work session.
///
/// - `Active`: Work is in progress
/// - `Completed`: Session finished successfully, history entry created
/// - `Failed`: Session ended without successful completion
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Completed,
    Failed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Input for creating a new session.
///
/// Sessions are created with an initial set of tasks. The feature must be a
/// leaf node (no children) and must not have an active session already.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionInput {
    pub feature_id: Uuid,
    /// High-level objective for this work session.
    pub goal: String,
    /// Initial tasks to create with the session.
    pub tasks: Vec<CreateTaskInput>,
}

/// Response when creating a session, includes the session and its tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session: Session,
    pub tasks: Vec<super::Task>,
}

/// Detailed session status including feature context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusResponse {
    pub session: Session,
    pub feature: SessionFeatureSummary,
    pub tasks: Vec<super::Task>,
}

/// Minimal feature info included in session status responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFeatureSummary {
    pub id: Uuid,
    pub title: String,
}

/// Input for completing a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteSessionInput {
    /// Summary of work done, becomes the history entry description.
    pub summary: String,
    /// Optionally update the feature's state (e.g., to `Implemented`).
    /// If not provided, the feature state is not changed.
    #[serde(default)]
    pub feature_state: Option<FeatureState>,
}

/// Result of completing a session.
///
/// Contains the updated session (now with `Completed` status) and the
/// newly created history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCompletionResult {
    pub session: Session,
    pub history_entry: super::FeatureHistory,
}
