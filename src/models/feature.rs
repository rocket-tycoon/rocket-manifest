use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A living description of a system capability.
///
/// Unlike traditional issue trackers where items are "closed" and forgotten,
/// features are permanent documentation that evolves with the codebase.
/// Features form a hierarchical tree structure via `parent_id`, where any node
/// can have content, but only leaf nodes can have active sessions.
///
/// # Lifecycle
/// Features progress through states: Proposed → Specified → Implemented → (Living).
/// The "living" phase is implicit—implemented features remain active documentation
/// until deprecated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub story: Option<String>,
    pub details: Option<String>,
    pub state: FeatureState,
    /// Priority for ordering features within a parent. Lower values appear first.
    /// Use this to indicate implementation order without polluting feature titles.
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The lifecycle state of a feature.
///
/// - `Proposed`: Initial idea, not yet fully specified
/// - `Specified`: Requirements defined, ready for implementation
/// - `Implemented`: Built and deployed (enters "living" phase)
/// - `Deprecated`: No longer active, kept for historical reference
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    Proposed,
    Specified,
    Implemented,
    Deprecated,
}

impl FeatureState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Specified => "specified",
            Self::Implemented => "implemented",
            Self::Deprecated => "deprecated",
        }
    }
}

impl FromStr for FeatureState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "proposed" => Ok(Self::Proposed),
            "specified" => Ok(Self::Specified),
            "implemented" => Ok(Self::Implemented),
            "deprecated" => Ok(Self::Deprecated),
            _ => Err(()),
        }
    }
}

/// Input for creating a new feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFeatureInput {
    /// Parent feature ID for nesting. `None` creates a root feature.
    pub parent_id: Option<Uuid>,
    pub title: String,
    /// User story in "As a... I want... So that..." format.
    pub story: Option<String>,
    /// Technical details, constraints, or additional context.
    pub details: Option<String>,
    /// Initial state. Defaults to `Proposed` if not specified.
    pub state: Option<FeatureState>,
    /// Priority for ordering within parent. Lower values first. Defaults to 0.
    pub priority: Option<i32>,
}

/// Input for updating an existing feature. All fields are optional for partial updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFeatureInput {
    /// Move feature under a different parent.
    pub parent_id: Option<Uuid>,
    pub title: Option<String>,
    pub story: Option<String>,
    pub details: Option<String>,
    pub state: Option<FeatureState>,
    /// Update priority for ordering within parent.
    pub priority: Option<i32>,
}

/// A feature with its nested children, used for tree responses.
///
/// The `feature` fields are flattened into the JSON response, with an additional
/// `children` array containing nested `FeatureTreeNode` objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTreeNode {
    #[serde(flatten)]
    pub feature: Feature,
    pub children: Vec<FeatureTreeNode>,
}
