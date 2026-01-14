use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::db::Database;
use crate::models::*;

// Import MCP types for bulk feature creation (re-exported from mcp module)
use crate::mcp::{PlanFeaturesResponse, ProposedFeature};

// ============================================================
// Error Handling
// ============================================================

/// Log an internal error and return a sanitized response to the client.
/// The full error is logged server-side for debugging, but clients only
/// see a generic message to avoid leaking internal details.
///
/// Some errors are validation errors that should be exposed to the client
/// (e.g., "Sessions can only be created on leaf features"). These are
/// returned as-is with a BAD_REQUEST status.
fn internal_error(e: impl std::fmt::Display) -> (StatusCode, String) {
    let msg = e.to_string();

    // Known validation errors that are safe to expose
    if msg.contains("leaf") || msg.contains("not active") || msg.contains("not found") {
        tracing::warn!("Validation error: {}", msg);
        return (StatusCode::BAD_REQUEST, msg);
    }

    tracing::error!("Internal error: {}", msg);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal server error".to_string(),
    )
}

// ============================================================
// Health
// ============================================================

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// ============================================================
// Projects
// ============================================================

pub async fn list_projects(
    State(db): State<Database>,
) -> Result<Json<Vec<Project>>, (StatusCode, String)> {
    db.get_all_projects().map(Json).map_err(internal_error)
}

pub async fn get_project(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectWithDirectories>, (StatusCode, String)> {
    db.get_project_with_directories(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))
}

pub async fn create_project(
    State(db): State<Database>,
    Json(input): Json<CreateProjectInput>,
) -> Result<(StatusCode, Json<Project>), (StatusCode, String)> {
    db.create_project(input)
        .map(|p| (StatusCode::CREATED, Json(p)))
        .map_err(internal_error)
}

pub async fn update_project(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProjectInput>,
) -> Result<Json<Project>, (StatusCode, String)> {
    db.update_project(id, input)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))
}

pub async fn delete_project(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if db.delete_project(id).map_err(internal_error)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Project not found".to_string()))
    }
}

// ============================================================
// Project Directories
// ============================================================

pub async fn list_project_directories(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectDirectory>>, (StatusCode, String)> {
    db.get_project_directories(project_id)
        .map(Json)
        .map_err(internal_error)
}

pub async fn add_project_directory(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<AddDirectoryInput>,
) -> Result<(StatusCode, Json<ProjectDirectory>), (StatusCode, String)> {
    db.add_project_directory(project_id, input)
        .map(|d| (StatusCode::CREATED, Json(d)))
        .map_err(internal_error)
}

pub async fn remove_project_directory(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if db.remove_project_directory(id).map_err(internal_error)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Directory not found".to_string()))
    }
}

// ============================================================
// Features
// ============================================================

pub async fn list_features(
    State(db): State<Database>,
    Query(query): Query<ListFeaturesQuery>,
) -> Result<Json<Vec<FeatureSummary>>, (StatusCode, String)> {
    let features = db.get_all_features().map_err(internal_error)?;

    // Apply pagination
    let offset = query.offset.unwrap_or(0) as usize;
    let features: Vec<_> = features.into_iter().skip(offset).collect();
    let features: Vec<_> = match query.limit {
        Some(limit) => features.into_iter().take(limit as usize).collect(),
        None => features,
    };

    // Always return summaries only - use get_feature for full details
    let summaries: Vec<FeatureSummary> = features.into_iter().map(Into::into).collect();
    Ok(Json(summaries))
}

pub async fn list_project_features(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListFeaturesQuery>,
) -> Result<Json<Vec<FeatureSummary>>, (StatusCode, String)> {
    let features = db
        .get_features_by_project(project_id)
        .map_err(internal_error)?;

    // Apply pagination
    let offset = query.offset.unwrap_or(0) as usize;
    let features: Vec<_> = features.into_iter().skip(offset).collect();
    let features: Vec<_> = match query.limit {
        Some(limit) => features.into_iter().take(limit as usize).collect(),
        None => features,
    };

    // Always return summaries only - use get_feature for full details
    let summaries: Vec<FeatureSummary> = features.into_iter().map(Into::into).collect();
    Ok(Json(summaries))
}

pub async fn list_root_features(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<Feature>>, (StatusCode, String)> {
    db.get_root_features(project_id)
        .map(Json)
        .map_err(internal_error)
}

pub async fn get_feature_tree(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<FeatureTreeNode>>, (StatusCode, String)> {
    db.get_feature_tree(project_id)
        .map(Json)
        .map_err(internal_error)
}

pub async fn list_children(
    State(db): State<Database>,
    Path(parent_id): Path<Uuid>,
) -> Result<Json<Vec<Feature>>, (StatusCode, String)> {
    db.get_children(parent_id).map(Json).map_err(internal_error)
}

pub async fn get_feature_history(
    State(db): State<Database>,
    Path(feature_id): Path<Uuid>,
) -> Result<Json<Vec<FeatureHistory>>, (StatusCode, String)> {
    db.get_feature_history(feature_id)
        .map(Json)
        .map_err(internal_error)
}

pub async fn list_feature_sessions(
    State(db): State<Database>,
    Path(feature_id): Path<Uuid>,
) -> Result<Json<Vec<Session>>, (StatusCode, String)> {
    // First verify feature exists
    db.get_feature(feature_id)
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Feature not found".to_string()))?;

    db.get_sessions_by_feature(feature_id)
        .map(Json)
        .map_err(internal_error)
}

pub async fn create_feature_session(
    State(db): State<Database>,
    Path(feature_id): Path<Uuid>,
    Json(input): Json<CreateFeatureSessionInput>,
) -> Result<(StatusCode, Json<SessionResponse>), (StatusCode, String)> {
    // First verify feature exists
    db.get_feature(feature_id)
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Feature not found".to_string()))?;

    // Convert to CreateSessionInput with feature_id from path
    let session_input = CreateSessionInput {
        feature_id,
        goal: input.goal,
        tasks: input.tasks,
    };

    db.create_session(session_input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(internal_error)
}

pub async fn get_feature(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<Feature>, (StatusCode, String)> {
    db.get_feature(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Feature not found".to_string()))
}

pub async fn get_feature_diff(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<FeatureDiff>, (StatusCode, String)> {
    db.get_feature_diff(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Feature not found".to_string()))
}

pub async fn create_feature(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<CreateFeatureInput>,
) -> Result<(StatusCode, Json<Feature>), (StatusCode, String)> {
    db.create_feature(project_id, input)
        .map(|f| (StatusCode::CREATED, Json(f)))
        .map_err(internal_error)
}

pub async fn update_feature(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateFeatureInput>,
) -> Result<Json<Feature>, (StatusCode, String)> {
    db.update_feature(id, input)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Feature not found".to_string()))
}

pub async fn delete_feature(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if db.delete_feature(id).map_err(internal_error)? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "Feature not found".to_string()))
    }
}

/// Query parameters for searching features.
#[derive(Debug, Deserialize)]
pub struct SearchFeaturesQuery {
    /// Search term to match against title and details.
    pub q: String,
    /// Optional project UUID to limit search to.
    pub project_id: Option<Uuid>,
    /// Maximum number of results to return. Defaults to 10.
    pub limit: Option<u32>,
}

/// Search features by title and details.
/// Returns summaries ranked by relevance.
pub async fn search_features(
    State(db): State<Database>,
    Query(query): Query<SearchFeaturesQuery>,
) -> Result<Json<Vec<FeatureSummary>>, (StatusCode, String)> {
    db.search_features(&query.q, query.project_id, query.limit)
        .map(Json)
        .map_err(internal_error)
}

// ============================================================
// Sessions
// ============================================================

pub async fn create_session(
    State(db): State<Database>,
    Json(input): Json<CreateSessionInput>,
) -> Result<(StatusCode, Json<SessionResponse>), (StatusCode, String)> {
    db.create_session(input)
        .map(|s| (StatusCode::CREATED, Json(s)))
        .map_err(internal_error)
}

pub async fn get_session(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<Session>, (StatusCode, String)> {
    db.get_session(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))
}

pub async fn get_session_status(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionStatusResponse>, (StatusCode, String)> {
    db.get_session_status(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))
}

pub async fn complete_session(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
    Json(input): Json<CompleteSessionInput>,
) -> Result<Json<SessionCompletionResult>, (StatusCode, String)> {
    db.complete_session(id, input)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))
}

// ============================================================
// Tasks
// ============================================================

pub async fn get_task(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, (StatusCode, String)> {
    db.get_task(id)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Task not found".to_string()))
}

pub async fn update_task(
    State(db): State<Database>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTaskInput>,
) -> Result<StatusCode, (StatusCode, String)> {
    if db.update_task(id, input).map_err(internal_error)? {
        Ok(StatusCode::OK)
    } else {
        Err((StatusCode::NOT_FOUND, "Task not found".to_string()))
    }
}

pub async fn create_session_task(
    State(db): State<Database>,
    Path(session_id): Path<Uuid>,
    Json(input): Json<CreateTaskInput>,
) -> Result<(StatusCode, Json<Task>), (StatusCode, String)> {
    // First verify session exists
    db.get_session(session_id)
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    db.create_task(session_id, input)
        .map(|t| (StatusCode::CREATED, Json(t)))
        .map_err(internal_error)
}

pub async fn list_session_tasks(
    State(db): State<Database>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Vec<Task>>, (StatusCode, String)> {
    // First verify session exists
    db.get_session(session_id)
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Session not found".to_string()))?;

    db.get_tasks_by_session(session_id)
        .map(Json)
        .map_err(internal_error)
}

// ============================================================
// Project by Directory (for MCP get_project_context)
// ============================================================

/// Query parameters for getting a project by directory path.
#[derive(Debug, Deserialize)]
pub struct GetProjectByDirectoryQuery {
    pub path: String,
}

/// Find a project by directory path.
///
/// Returns the project and matching directory if the path matches exactly,
/// or if the path is a subdirectory of a registered project directory.
pub async fn get_project_by_directory(
    State(db): State<Database>,
    Query(query): Query<GetProjectByDirectoryQuery>,
) -> Result<Json<ProjectWithDirectories>, (StatusCode, String)> {
    db.get_project_by_directory(&query.path)
        .map_err(internal_error)?
        .map(Json)
        .ok_or((
            StatusCode::NOT_FOUND,
            format!("No project found for directory: {}", query.path),
        ))
}

// ============================================================
// Bulk Feature Creation (for MCP plan_features)
// ============================================================

/// Input for bulk feature creation.
#[derive(Debug, Deserialize)]
pub struct BulkCreateFeaturesInput {
    /// The proposed feature tree.
    pub features: Vec<ProposedFeature>,
    /// If true, creates the features in the database. If false, returns preview only.
    #[serde(default)]
    pub confirm: bool,
}

/// Create multiple features at once with hierarchical structure.
///
/// When confirm=false (default), returns the proposed features without creating them.
/// When confirm=true, creates all features and returns their IDs.
pub async fn bulk_create_features(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
    Json(input): Json<BulkCreateFeaturesInput>,
) -> Result<Json<PlanFeaturesResponse>, (StatusCode, String)> {
    // Verify project exists
    db.get_project(project_id)
        .map_err(internal_error)?
        .ok_or((StatusCode::NOT_FOUND, "Project not found".to_string()))?;

    let mut created_ids = Vec::new();

    if input.confirm {
        // Create features recursively
        for feature in &input.features {
            create_feature_recursive(&db, project_id, None, feature, &mut created_ids)
                .map_err(internal_error)?;
        }
    }

    Ok(Json(PlanFeaturesResponse {
        proposed_features: input.features,
        created: input.confirm,
        created_feature_ids: created_ids,
    }))
}

/// Recursively create features from a ProposedFeature tree.
fn create_feature_recursive(
    db: &Database,
    project_id: Uuid,
    parent_id: Option<Uuid>,
    proposed: &ProposedFeature,
    created_ids: &mut Vec<String>,
) -> anyhow::Result<Uuid> {
    let feature = db.create_feature(
        project_id,
        CreateFeatureInput {
            parent_id,
            title: proposed.title.clone(),
            details: proposed.details.clone(),
            state: Some(FeatureState::Specified),
            priority: Some(proposed.priority),
        },
    )?;

    created_ids.push(feature.id.to_string());

    // Create children with this feature as parent
    for child in &proposed.children {
        create_feature_recursive(db, project_id, Some(feature.id), child, created_ids)?;
    }

    Ok(feature.id)
}
