//! HTTP client for the Manifest API.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Error types for the Manifest client.
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] ureq::Error),
    #[error("Failed to parse response: {0}")]
    Io(#[from] std::io::Error),
    #[error("Server returned error: {0}")]
    Server(String),
}

/// Feature state in the Manifest system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureState {
    Proposed,
    Specified,
    Implemented,
    Deprecated,
}

/// A feature in the Manifest system (matches FeatureTreeNode from API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: Uuid,
    pub project_id: Uuid,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    pub title: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub desired_details: Option<String>,
    pub state: FeatureState,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub children: Vec<Feature>,
}

/// A project in the Manifest system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// A project directory association.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDirectory {
    pub id: Uuid,
    pub project_id: Uuid,
    pub path: String,
    #[serde(default)]
    pub git_remote: Option<String>,
    pub is_primary: bool,
}

/// A project with its associated directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithDirectories {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub directories: Vec<ProjectDirectory>,
}

/// HTTP client for the Manifest API.
#[derive(Clone)]
pub struct ManifestClient {
    base_url: String,
}

impl ManifestClient {
    /// Create a new client with the given base URL.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Create a client pointing to localhost:17010.
    pub fn localhost() -> Self {
        Self::new("http://localhost:17010/api/v1")
    }

    /// Get the list of projects (blocking).
    pub fn get_projects(&self) -> Result<Vec<Project>, ClientError> {
        let url = format!("{}/projects", self.base_url);
        let response: Vec<Project> = ureq::get(&url).call()?.into_json()?;
        Ok(response)
    }

    /// Get the feature tree for a project (blocking).
    /// Returns the tree as a flat array of FeatureTreeNode (Feature with children).
    pub fn get_feature_tree(&self, project_id: &Uuid) -> Result<Vec<Feature>, ClientError> {
        let url = format!("{}/projects/{}/features/tree", self.base_url, project_id);
        // API returns array directly, not wrapped in an object
        let response: Vec<Feature> = ureq::get(&url).call()?.into_json()?;
        Ok(response)
    }

    /// Get a project by directory path (blocking).
    /// Returns the project associated with the given directory, or None if not found.
    pub fn get_project_by_directory(&self, path: &str) -> Result<Option<ProjectWithDirectories>, ClientError> {
        let url = format!("{}/projects/by-directory?path={}", self.base_url, urlencoding::encode(path));
        match ureq::get(&url).call() {
            Ok(response) => {
                let project: ProjectWithDirectories = response.into_json()?;
                Ok(Some(project))
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
