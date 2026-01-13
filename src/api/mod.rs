mod handlers;
mod middleware;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::db::Database;

pub use middleware::SecurityConfig;

/// Build CORS layer based on configuration
fn build_cors_layer(config: &SecurityConfig) -> CorsLayer {
    use axum::http::{header, Method};
    use tower_http::cors::AllowOrigin;

    if let Some(ref origins) = config.cors_origins {
        let origins: Vec<_> = origins.iter().filter_map(|s| s.parse().ok()).collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    } else {
        CorsLayer::permissive()
    }
}

pub fn create_router(db: Database) -> Router {
    create_router_with_config(db, SecurityConfig::from_env())
}

pub fn create_router_with_config(db: Database, config: SecurityConfig) -> Router {
    // Health endpoint (unauthenticated)
    let health_router = Router::new().route("/health", get(handlers::health));

    // Protected API routes
    let protected_api = Router::new()
        // Projects
        .route("/projects", get(handlers::list_projects))
        .route("/projects", post(handlers::create_project))
        .route("/projects/{id}", get(handlers::get_project))
        .route("/projects/{id}", put(handlers::update_project))
        .route("/projects/{id}", delete(handlers::delete_project))
        .route(
            "/projects/{id}/directories",
            get(handlers::list_project_directories),
        )
        .route(
            "/projects/{id}/directories",
            post(handlers::add_project_directory),
        )
        .route(
            "/projects/{id}/features",
            get(handlers::list_project_features),
        )
        .route("/projects/{id}/features", post(handlers::create_feature))
        .route(
            "/projects/{id}/features/roots",
            get(handlers::list_root_features),
        )
        .route(
            "/projects/{id}/features/tree",
            get(handlers::get_feature_tree),
        )
        // Directories (for delete by directory id)
        .route(
            "/directories/{id}",
            delete(handlers::remove_project_directory),
        )
        // Features (by feature id)
        .route("/features", get(handlers::list_features))
        .route("/features/{id}", get(handlers::get_feature))
        .route("/features/{id}", put(handlers::update_feature))
        .route("/features/{id}", delete(handlers::delete_feature))
        .route("/features/{id}/children", get(handlers::list_children))
        .route("/features/{id}/diff", get(handlers::get_feature_diff))
        .route("/features/{id}/history", get(handlers::get_feature_history))
        .route(
            "/features/{id}/sessions",
            get(handlers::list_feature_sessions).post(handlers::create_feature_session),
        )
        // Sessions
        .route("/sessions", post(handlers::create_session))
        .route("/sessions/{id}", get(handlers::get_session))
        .route("/sessions/{id}/status", get(handlers::get_session_status))
        .route("/sessions/{id}/complete", post(handlers::complete_session))
        .route(
            "/sessions/{id}/tasks",
            get(handlers::list_session_tasks).post(handlers::create_session_task),
        )
        // Tasks
        .route("/tasks/{id}", get(handlers::get_task))
        .route("/tasks/{id}", put(handlers::update_task));

    // Apply auth middleware to protected routes if API key is configured
    let protected_api = if config.api_key.is_some() {
        protected_api.layer(axum::middleware::from_fn_with_state(
            config.clone(),
            middleware::auth_middleware,
        ))
    } else {
        protected_api
    };

    // Apply rate limiting if configured
    let protected_api = if let Some(rate_limiter) = config.rate_limiter.clone() {
        protected_api.layer(axum::middleware::from_fn_with_state(
            rate_limiter,
            middleware::rate_limit_middleware,
        ))
    } else {
        protected_api
    };

    let cors_layer = build_cors_layer(&config);

    // Combine health (unauthenticated) with protected API
    let api = health_router.merge(protected_api);

    Router::new()
        .nest("/api/v1", api)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
        .with_state(db)
}
