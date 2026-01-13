use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::db::Database;
use crate::models::*;

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
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let features = db.get_all_features().map_err(internal_error)?;

    // Apply pagination
    let offset = query.offset.unwrap_or(0) as usize;
    let features: Vec<_> = features.into_iter().skip(offset).collect();
    let features: Vec<_> = match query.limit {
        Some(limit) => features.into_iter().take(limit as usize).collect(),
        None => features,
    };

    // Return summary or full details
    if query.include_details {
        Ok(Json(serde_json::to_value(features).unwrap()))
    } else {
        let summaries: Vec<FeatureSummary> = features.into_iter().map(Into::into).collect();
        Ok(Json(serde_json::to_value(summaries).unwrap()))
    }
}

pub async fn list_project_features(
    State(db): State<Database>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListFeaturesQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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

    // Return summary or full details
    if query.include_details {
        Ok(Json(serde_json::to_value(features).unwrap()))
    } else {
        let summaries: Vec<FeatureSummary> = features.into_iter().map(Into::into).collect();
        Ok(Json(serde_json::to_value(summaries).unwrap()))
    }
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
