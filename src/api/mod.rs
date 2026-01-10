mod handlers;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::db::Database;

pub fn create_router(db: Database) -> Router {
    let api = Router::new()
        // Projects
        .route("/projects", get(handlers::list_projects))
        .route("/projects", post(handlers::create_project))
        .route("/projects/{id}", get(handlers::get_project))
        .route("/projects/{id}", put(handlers::update_project))
        .route("/projects/{id}", delete(handlers::delete_project))
        .route("/projects/{id}/directories", get(handlers::list_project_directories))
        .route("/projects/{id}/directories", post(handlers::add_project_directory))
        .route("/projects/{id}/features", get(handlers::list_project_features))
        .route("/projects/{id}/features", post(handlers::create_feature))
        .route("/projects/{id}/features/roots", get(handlers::list_root_features))
        .route("/projects/{id}/features/tree", get(handlers::get_feature_tree))
        // Directories (for delete by directory id)
        .route("/directories/{id}", delete(handlers::remove_project_directory))
        // Features (by feature id)
        .route("/features", get(handlers::list_features))
        .route("/features/{id}", get(handlers::get_feature))
        .route("/features/{id}", put(handlers::update_feature))
        .route("/features/{id}", delete(handlers::delete_feature))
        .route("/features/{id}/children", get(handlers::list_children))
        .route("/features/{id}/history", get(handlers::get_feature_history))
        // Sessions
        .route("/sessions", post(handlers::create_session))
        .route("/sessions/{id}", get(handlers::get_session))
        .route("/sessions/{id}/status", get(handlers::get_session_status))
        .route("/sessions/{id}/complete", post(handlers::complete_session))
        // Tasks
        .route("/tasks/{id}", get(handlers::get_task))
        .route("/tasks/{id}", put(handlers::update_task))
        // Health
        .route("/health", get(handlers::health));

    Router::new()
        .nest("/api/v1", api)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(db)
}
