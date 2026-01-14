use axum::http::StatusCode;
use axum_test::TestServer;
use rocket_manifest::api::create_router;
use rocket_manifest::db::Database;
use rocket_manifest::models::*;

fn setup() -> TestServer {
    let db = Database::open_memory().expect("Failed to create database");
    db.migrate().expect("Failed to migrate");
    let app = create_router(db);
    TestServer::new(app).expect("Failed to create test server")
}

async fn create_test_project(server: &TestServer) -> Project {
    server
        .post("/api/v1/projects")
        .json(&CreateProjectInput {
            name: "Test Project".to_string(),
            description: None,
            instructions: None,
        })
        .await
        .json::<Project>()
}

mod feature_roots {
    use super::*;

    #[tokio::test]
    async fn returns_empty_list_when_no_features_exist() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .get(&format!("/api/v1/projects/{}/features/roots", project.id))
            .await;

        response.assert_status_ok();
        let features: Vec<Feature> = response.json();
        assert!(features.is_empty());
    }

    #[tokio::test]
    async fn returns_only_root_features() {
        let server = setup();
        let project = create_test_project(&server).await;

        // Create root feature
        let root = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Root".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        // Create child feature
        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Child".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        let response = server
            .get(&format!("/api/v1/projects/{}/features/roots", project.id))
            .await;

        response.assert_status_ok();
        let features: Vec<Feature> = response.json();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].title, "Root");
        assert!(features[0].parent_id.is_none());
    }
}

mod feature_children {
    use super::*;

    #[tokio::test]
    async fn returns_empty_list_when_feature_has_no_children() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Leaf".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .get(&format!("/api/v1/features/{}/children", feature.id))
            .await;

        response.assert_status_ok();
        let children: Vec<Feature> = response.json();
        assert!(children.is_empty());
    }

    #[tokio::test]
    async fn returns_direct_children_ordered_by_title() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Zebra".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Alpha".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        let response = server
            .get(&format!("/api/v1/features/{}/children", parent.id))
            .await;

        response.assert_status_ok();
        let children: Vec<Feature> = response.json();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].title, "Alpha");
        assert_eq!(children[1].title, "Zebra");
    }

    #[tokio::test]
    async fn does_not_return_grandchildren() {
        let server = setup();
        let project = create_test_project(&server).await;

        let root = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Root".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let child = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Child".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(child.id),
                title: "Grandchild".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        let response = server
            .get(&format!("/api/v1/features/{}/children", root.id))
            .await;

        response.assert_status_ok();
        let children: Vec<Feature> = response.json();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "Child");
    }
}

mod feature_hierarchy_create {
    use super::*;

    #[tokio::test]
    async fn creates_child_feature_with_parent_id() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Authentication".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Login".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let child: Feature = response.json();
        assert_eq!(child.parent_id, Some(parent.id));
        assert_eq!(child.title, "Login");
    }

    #[tokio::test]
    async fn creates_deeply_nested_features() {
        let server = setup();
        let project = create_test_project(&server).await;

        let level0 = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Authentication".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let level1 = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(level0.id),
                title: "OAuth".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let level2 = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(level1.id),
                title: "Google".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        assert_eq!(level2.parent_id, Some(level1.id));

        // Verify via GET
        let response = server.get(&format!("/api/v1/features/{}", level2.id)).await;
        let fetched: Feature = response.json();
        assert_eq!(fetched.parent_id, Some(level1.id));
    }
}

mod feature_cascade_delete {
    use super::*;

    #[tokio::test]
    async fn deletes_children_when_parent_is_deleted() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let child = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Child".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        // Delete parent
        server
            .delete(&format!("/api/v1/features/{}", parent.id))
            .await
            .assert_status(StatusCode::NO_CONTENT);

        // Child should be gone
        server
            .get(&format!("/api/v1/features/{}", child.id))
            .await
            .assert_status_not_found();
    }
}

mod feature_history {
    use super::*;

    #[tokio::test]
    async fn returns_empty_list_when_no_history() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "New Feature".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .get(&format!("/api/v1/features/{}/history", feature.id))
            .await;

        response.assert_status_ok();
        let history: Vec<FeatureHistory> = response.json();
        assert!(history.is_empty());
    }
}

mod session_leaf_validation {
    use super::*;

    #[tokio::test]
    async fn allows_session_creation_on_leaf_feature() {
        let server = setup();
        let project = create_test_project(&server).await;

        let leaf = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Leaf Feature".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: leaf.id,
                goal: "Implement feature".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status(StatusCode::CREATED);
    }

    #[tokio::test]
    async fn rejects_session_creation_on_non_leaf_feature() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Child".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await;

        let response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: parent.id,
                goal: "Implement feature".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status_bad_request();
        let body = response.text();
        assert!(body.contains("leaf"));
    }
}

mod session_completion {
    use super::*;

    #[tokio::test]
    async fn completes_session_and_returns_result() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let session = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Implement feature".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "Task".to_string(),
                    scope: "Scope".to_string(),
                    agent_type: AgentType::Claude,
                }],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .post(&format!("/api/v1/sessions/{}/complete", session.session.id))
            .json(&CompleteSessionInput {
                summary: "Feature implemented".to_string(),
                commits: vec![],
                feature_state: None,
            })
            .await;

        response.assert_status_ok();
        let result: SessionCompletionResult = response.json();
        assert_eq!(result.session.status, SessionStatus::Completed);
        assert!(result.session.completed_at.is_some());
        assert_eq!(result.history_entry.details.summary, "Feature implemented");
    }

    #[tokio::test]
    async fn creates_history_entry_on_completion() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                state: None,

                details: None,
                priority: None,
            })
            .await
            .json::<Feature>();

        let session = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![],
            })
            .await
            .json::<SessionResponse>();

        server
            .post(&format!("/api/v1/sessions/{}/complete", session.session.id))
            .json(&CompleteSessionInput {
                summary: "Work completed".to_string(),
                commits: vec![],
                feature_state: None,
            })
            .await;

        // Check history was created
        let response = server
            .get(&format!("/api/v1/features/{}/history", feature.id))
            .await;
        let history: Vec<FeatureHistory> = response.json();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].details.summary, "Work completed");
    }

    #[tokio::test]
    async fn returns_not_found_for_nonexistent_session() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .post(&format!("/api/v1/sessions/{}/complete", fake_id))
            .json(&CompleteSessionInput {
                summary: "Done".to_string(),
                commits: vec![],
                feature_state: None,
            })
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Health endpoint
// ============================================================

mod health {
    use super::*;

    #[tokio::test]
    async fn returns_ok() {
        let server = setup();

        let response = server.get("/api/v1/health").await;

        response.assert_status_ok();
    }
}

// ============================================================
// Project CRUD
// ============================================================

mod projects {
    use super::*;

    #[tokio::test]
    async fn list_returns_empty_when_no_projects() {
        let server = setup();

        let response = server.get("/api/v1/projects").await;

        response.assert_status_ok();
        let projects: Vec<Project> = response.json();
        assert!(projects.is_empty());
    }

    #[tokio::test]
    async fn list_returns_all_projects_ordered_by_name() {
        let server = setup();

        server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "Zebra Project".to_string(),
                description: None,
                instructions: None,
            })
            .await;

        server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "Alpha Project".to_string(),
                description: None,
                instructions: None,
            })
            .await;

        let response = server.get("/api/v1/projects").await;

        response.assert_status_ok();
        let projects: Vec<Project> = response.json();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "Alpha Project");
        assert_eq!(projects[1].name, "Zebra Project");
    }

    #[tokio::test]
    async fn create_returns_created_status() {
        let server = setup();

        let response = server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "New Project".to_string(),
                description: Some("A description".to_string()),
                instructions: Some("Build with cargo".to_string()),
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let project: Project = response.json();
        assert_eq!(project.name, "New Project");
        assert_eq!(project.description, Some("A description".to_string()));
        assert_eq!(project.instructions, Some("Build with cargo".to_string()));
    }

    #[tokio::test]
    async fn get_returns_project_by_id() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .get(&format!("/api/v1/projects/{}", project.id))
            .await;

        response.assert_status_ok();
        let fetched: Project = response.json();
        assert_eq!(fetched.id, project.id);
        assert_eq!(fetched.name, project.name);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_nonexistent_project() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server.get(&format!("/api/v1/projects/{}", fake_id)).await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn update_modifies_project() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .put(&format!("/api/v1/projects/{}", project.id))
            .json(&UpdateProjectInput {
                name: Some("Updated Name".to_string()),
                description: Some("New description".to_string()),
                instructions: None,
            })
            .await;

        response.assert_status_ok();
        let updated: Project = response.json();
        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[tokio::test]
    async fn update_returns_not_found_for_nonexistent_project() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .put(&format!("/api/v1/projects/{}", fake_id))
            .json(&UpdateProjectInput {
                name: Some("Name".to_string()),
                description: None,
                instructions: None,
            })
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn delete_removes_project() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .delete(&format!("/api/v1/projects/{}", project.id))
            .await;

        response.assert_status(StatusCode::NO_CONTENT);

        // Verify it's gone
        server
            .get(&format!("/api/v1/projects/{}", project.id))
            .await
            .assert_status_not_found();
    }

    #[tokio::test]
    async fn delete_returns_not_found_for_nonexistent_project() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .delete(&format!("/api/v1/projects/{}", fake_id))
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Project Directories
// ============================================================

mod project_directories {
    use super::*;

    #[tokio::test]
    async fn list_returns_empty_when_no_directories() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .get(&format!("/api/v1/projects/{}/directories", project.id))
            .await;

        response.assert_status_ok();
        let dirs: Vec<ProjectDirectory> = response.json();
        assert!(dirs.is_empty());
    }

    #[tokio::test]
    async fn add_creates_directory() {
        let server = setup();
        let project = create_test_project(&server).await;

        let response = server
            .post(&format!("/api/v1/projects/{}/directories", project.id))
            .json(&AddDirectoryInput {
                path: "/home/user/project".to_string(),
                git_remote: Some("git@github.com:user/repo.git".to_string()),
                is_primary: true,
                instructions: Some("Run npm test".to_string()),
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let dir: ProjectDirectory = response.json();
        assert_eq!(dir.path, "/home/user/project");
        assert!(dir.is_primary);
    }

    #[tokio::test]
    async fn delete_removes_directory() {
        let server = setup();
        let project = create_test_project(&server).await;

        let dir = server
            .post(&format!("/api/v1/projects/{}/directories", project.id))
            .json(&AddDirectoryInput {
                path: "/home/user/project".to_string(),
                git_remote: None,
                is_primary: false,
                instructions: None,
            })
            .await
            .json::<ProjectDirectory>();

        let response = server
            .delete(&format!("/api/v1/directories/{}", dir.id))
            .await;

        response.assert_status(StatusCode::NO_CONTENT);

        // Verify it's gone
        let list = server
            .get(&format!("/api/v1/projects/{}/directories", project.id))
            .await
            .json::<Vec<ProjectDirectory>>();
        assert!(list.is_empty());
    }
}

// ============================================================
// Feature CRUD
// ============================================================

mod features {
    use super::*;

    #[tokio::test]
    async fn list_returns_all_features() {
        let server = setup();
        let project = create_test_project(&server).await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature 1".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature 2".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await;

        // list_features always returns summaries - use get_feature for full details
        let response = server.get("/api/v1/features").await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert_eq!(features.len(), 2);
    }

    #[tokio::test]
    async fn get_returns_feature_by_id() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Test Feature".to_string(),
                details: Some("As a user... Implementation details".to_string()),
                state: Some(FeatureState::Specified),
                priority: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .get(&format!("/api/v1/features/{}", feature.id))
            .await;

        response.assert_status_ok();
        let fetched: Feature = response.json();
        assert_eq!(fetched.id, feature.id);
        assert_eq!(fetched.title, "Test Feature");
        assert_eq!(fetched.state, FeatureState::Specified);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_nonexistent_feature() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server.get(&format!("/api/v1/features/{}", fake_id)).await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn update_modifies_feature() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Original Title".to_string(),

                details: None,
                priority: None,
                state: Some(FeatureState::Proposed),
            })
            .await
            .json::<Feature>();

        let response = server
            .put(&format!("/api/v1/features/{}", feature.id))
            .json(&UpdateFeatureInput {
                parent_id: None,
                title: Some("Updated Title".to_string()),
                details: Some("New details".to_string()),
                desired_details: None,
                priority: None,
                state: Some(FeatureState::Implemented),
            })
            .await;

        response.assert_status_ok();
        let updated: Feature = response.json();
        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.details, Some("New details".to_string()));
        assert_eq!(updated.state, FeatureState::Implemented);
    }

    #[tokio::test]
    async fn update_returns_not_found_for_nonexistent_feature() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .put(&format!("/api/v1/features/{}", fake_id))
            .json(&UpdateFeatureInput {
                parent_id: None,
                title: Some("Title".to_string()),
                details: None,
                desired_details: None,
                priority: None,
                state: None,
            })
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Feature Search
// ============================================================

mod feature_search {
    use super::*;

    #[tokio::test]
    async fn returns_empty_list_when_no_matches() {
        let server = setup();
        let project = create_test_project(&server).await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "User Login".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        let response = server.get("/api/v1/features/search?q=nonexistent").await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert!(features.is_empty());
    }

    #[tokio::test]
    async fn finds_features_matching_title() {
        let server = setup();
        let project = create_test_project(&server).await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "User Authentication".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Payment Processing".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        let response = server.get("/api/v1/features/search?q=user").await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].title, "User Authentication");
    }

    #[tokio::test]
    async fn finds_features_matching_details() {
        let server = setup();
        let project = create_test_project(&server).await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "OAuth Integration".to_string(),
                details: Some("Implement Google OAuth using PKCE flow".to_string()),
                priority: None,
                state: None,
            })
            .await;

        let response = server.get("/api/v1/features/search?q=PKCE").await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].title, "OAuth Integration");
    }

    #[tokio::test]
    async fn respects_limit_parameter() {
        let server = setup();
        let project = create_test_project(&server).await;

        for i in 1..=5 {
            server
                .post(&format!("/api/v1/projects/{}/features", project.id))
                .json(&CreateFeatureInput {
                    parent_id: None,
                    title: format!("Feature {}", i),
                    details: None,
                    priority: None,
                    state: None,
                })
                .await;
        }

        let response = server
            .get("/api/v1/features/search?q=Feature&limit=2")
            .await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert_eq!(features.len(), 2);
    }

    #[tokio::test]
    async fn filters_by_project_id() {
        let server = setup();

        let project1 = server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "Project 1".to_string(),
                description: None,
                instructions: None,
            })
            .await
            .json::<Project>();

        let project2 = server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "Project 2".to_string(),
                description: None,
                instructions: None,
            })
            .await
            .json::<Project>();

        server
            .post(&format!("/api/v1/projects/{}/features", project1.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Auth in Project 1".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        server
            .post(&format!("/api/v1/projects/{}/features", project2.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Auth in Project 2".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        let response = server
            .get(&format!(
                "/api/v1/features/search?q=Auth&project_id={}",
                project1.id
            ))
            .await;

        response.assert_status_ok();
        let features: Vec<FeatureSummary> = response.json();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].title, "Auth in Project 1");
    }
}

// ============================================================
// Feature Diff
// ============================================================

mod feature_diff {
    use super::*;

    #[tokio::test]
    async fn returns_diff_with_no_changes_when_no_desired_details() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: Some("Current details".to_string()),
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .get(&format!("/api/v1/features/{}/diff", feature.id))
            .await;

        response.assert_status_ok();
        let diff: FeatureDiff = response.json();
        assert!(!diff.has_changes);
        assert_eq!(diff.current, Some("Current details".to_string()));
        assert!(diff.desired.is_none());
    }

    #[tokio::test]
    async fn returns_diff_with_changes_when_desired_details_set() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: Some("Current".to_string()),
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        // Update with desired_details
        server
            .put(&format!("/api/v1/features/{}", feature.id))
            .json(&UpdateFeatureInput {
                parent_id: None,
                title: None,
                details: None,
                desired_details: Some("Desired".to_string()),
                priority: None,
                state: None,
            })
            .await;

        let response = server
            .get(&format!("/api/v1/features/{}/diff", feature.id))
            .await;

        response.assert_status_ok();
        let diff: FeatureDiff = response.json();
        assert!(diff.has_changes);
        assert_eq!(diff.current, Some("Current".to_string()));
        assert_eq!(diff.desired, Some("Desired".to_string()));
    }

    #[tokio::test]
    async fn returns_not_found_for_nonexistent_feature() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .get(&format!("/api/v1/features/{}/diff", fake_id))
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Feature Tree
// ============================================================

mod feature_tree {
    use super::*;

    #[tokio::test]
    async fn returns_nested_tree_structure() {
        let server = setup();
        let project = create_test_project(&server).await;

        let root = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Authentication".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Login".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await;

        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Logout".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await;

        let response = server
            .get(&format!("/api/v1/projects/{}/features/tree", project.id))
            .await;

        response.assert_status_ok();
        let tree: Vec<FeatureTreeNode> = response.json();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].feature.title, "Authentication");
        assert_eq!(tree[0].children.len(), 2);
    }
}

// ============================================================
// Sessions
// ============================================================

mod sessions {
    use super::*;

    #[tokio::test]
    async fn get_returns_session_by_id() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Implement the feature".to_string(),
                tasks: vec![],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .get(&format!("/api/v1/sessions/{}", session_response.session.id))
            .await;

        response.assert_status_ok();
        let session: Session = response.json();
        assert_eq!(session.id, session_response.session.id);
        assert_eq!(session.goal, "Implement the feature");
        assert_eq!(session.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_nonexistent_session() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server.get(&format!("/api/v1/sessions/{}", fake_id)).await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn status_returns_session_with_feature_and_tasks() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature Title".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "Task 1".to_string(),
                    scope: "Scope".to_string(),
                    agent_type: AgentType::Claude,
                }],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .get(&format!(
                "/api/v1/sessions/{}/status",
                session_response.session.id
            ))
            .await;

        response.assert_status_ok();
        let status: SessionStatusResponse = response.json();
        assert_eq!(status.session.id, session_response.session.id);
        assert_eq!(status.feature.title, "Feature Title");
        assert_eq!(status.tasks.len(), 1);
        assert_eq!(status.tasks[0].title, "Task 1");
    }
}

// ============================================================
// Tasks
// ============================================================

mod tasks {
    use super::*;

    #[tokio::test]
    async fn get_returns_task_by_id() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "My Task".to_string(),
                    scope: "Task scope".to_string(),
                    agent_type: AgentType::Gemini,
                }],
            })
            .await
            .json::<SessionResponse>();

        let task_id = session_response.tasks[0].id;

        let response = server.get(&format!("/api/v1/tasks/{}", task_id)).await;

        response.assert_status_ok();
        let task: Task = response.json();
        assert_eq!(task.id, task_id);
        assert_eq!(task.title, "My Task");
        assert_eq!(task.scope, "Task scope");
        assert_eq!(task.agent_type, AgentType::Gemini);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_nonexistent_task() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server.get(&format!("/api/v1/tasks/{}", fake_id)).await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn update_modifies_task_status() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),

                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "Task".to_string(),
                    scope: "Scope".to_string(),
                    agent_type: AgentType::Claude,
                }],
            })
            .await
            .json::<SessionResponse>();

        let task_id = session_response.tasks[0].id;

        let response = server
            .put(&format!("/api/v1/tasks/{}", task_id))
            .json(&UpdateTaskInput {
                status: Some(TaskStatus::Running),
                worktree_path: Some("/tmp/worktree".to_string()),
                branch: Some("feature-branch".to_string()),
            })
            .await;

        response.assert_status_ok();

        // Verify the update
        let fetched = server
            .get(&format!("/api/v1/tasks/{}", task_id))
            .await
            .json::<Task>();
        assert_eq!(fetched.status, TaskStatus::Running);
        assert_eq!(fetched.worktree_path, Some("/tmp/worktree".to_string()));
        assert_eq!(fetched.branch, Some("feature-branch".to_string()));
    }

    #[tokio::test]
    async fn update_returns_not_found_for_nonexistent_task() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .put(&format!("/api/v1/tasks/{}", fake_id))
            .json(&UpdateTaskInput {
                status: Some(TaskStatus::Running),
                worktree_path: None,
                branch: None,
            })
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Session Tasks
// ============================================================

mod session_tasks {
    use super::*;

    #[tokio::test]
    async fn create_task_in_session() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .post(&format!(
                "/api/v1/sessions/{}/tasks",
                session_response.session.id
            ))
            .json(&CreateTaskInput {
                parent_id: None,
                title: "New Task".to_string(),
                scope: "Task scope".to_string(),
                agent_type: AgentType::Claude,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let task: Task = response.json();
        assert_eq!(task.title, "New Task");
        assert_eq!(task.scope, "Task scope");
        assert_eq!(task.agent_type, AgentType::Claude);
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn create_task_returns_not_found_for_nonexistent_session() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .post(&format!("/api/v1/sessions/{}/tasks", fake_id))
            .json(&CreateTaskInput {
                parent_id: None,
                title: "Task".to_string(),
                scope: "Scope".to_string(),
                agent_type: AgentType::Claude,
            })
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn list_tasks_in_session() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![
                    CreateTaskInput {
                        parent_id: None,
                        title: "Task 1".to_string(),
                        scope: "Scope 1".to_string(),
                        agent_type: AgentType::Claude,
                    },
                    CreateTaskInput {
                        parent_id: None,
                        title: "Task 2".to_string(),
                        scope: "Scope 2".to_string(),
                        agent_type: AgentType::Gemini,
                    },
                ],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .get(&format!(
                "/api/v1/sessions/{}/tasks",
                session_response.session.id
            ))
            .await;

        response.assert_status_ok();
        let tasks: Vec<Task> = response.json();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn list_tasks_returns_not_found_for_nonexistent_session() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .get(&format!("/api/v1/sessions/{}/tasks", fake_id))
            .await;

        response.assert_status_not_found();
    }
}

// ============================================================
// Feature Sessions
// ============================================================

mod feature_sessions {
    use super::*;

    #[tokio::test]
    async fn returns_empty_list_when_no_sessions() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .get(&format!("/api/v1/features/{}/sessions", feature.id))
            .await;

        response.assert_status_ok();
        let sessions: Vec<Session> = response.json();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn returns_sessions_for_feature() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        // Create a session
        let session_response = server
            .post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "First session".to_string(),
                tasks: vec![],
            })
            .await
            .json::<SessionResponse>();

        let response = server
            .get(&format!("/api/v1/features/{}/sessions", feature.id))
            .await;

        response.assert_status_ok();
        let sessions: Vec<Session> = response.json();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session_response.session.id);
        assert_eq!(sessions[0].goal, "First session");
    }

    #[tokio::test]
    async fn returns_not_found_for_nonexistent_feature() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .get(&format!("/api/v1/features/{}/sessions", fake_id))
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn creates_session_via_restful_endpoint() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .post(&format!("/api/v1/features/{}/sessions", feature.id))
            .json(&CreateFeatureSessionInput {
                goal: "RESTful session".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let session_response: SessionResponse = response.json();
        assert_eq!(session_response.session.feature_id, feature.id);
        assert_eq!(session_response.session.goal, "RESTful session");
        assert_eq!(session_response.session.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn creates_session_with_tasks_via_restful_endpoint() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server
            .post(&format!("/api/v1/features/{}/sessions", feature.id))
            .json(&CreateFeatureSessionInput {
                goal: "Session with tasks".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "Task 1".to_string(),
                    scope: "Scope".to_string(),
                    agent_type: AgentType::Claude,
                }],
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let session_response: SessionResponse = response.json();
        assert_eq!(session_response.tasks.len(), 1);
        assert_eq!(session_response.tasks[0].title, "Task 1");
    }

    #[tokio::test]
    async fn create_session_returns_not_found_for_nonexistent_feature() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server
            .post(&format!("/api/v1/features/{}/sessions", fake_id))
            .json(&CreateFeatureSessionInput {
                goal: "Goal".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn create_session_rejects_non_leaf_feature() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await
            .json::<Feature>();

        // Create a child to make parent non-leaf
        server
            .post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Child".to_string(),
                details: None,
                priority: None,
                state: None,
            })
            .await;

        let response = server
            .post(&format!("/api/v1/features/{}/sessions", parent.id))
            .json(&CreateFeatureSessionInput {
                goal: "Goal".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status_bad_request();
        let body = response.text();
        assert!(body.contains("leaf"));
    }
}

// ============================================================
// Security - API Key Authentication
// ============================================================

mod security_auth {
    use super::*;
    use rocket_manifest::api::{create_router_with_config, SecurityConfig};

    fn setup_with_auth(api_key: &str) -> TestServer {
        let db = Database::open_memory().expect("Failed to create database");
        db.migrate().expect("Failed to migrate");
        let config = SecurityConfig::with_api_key(api_key);
        let app = create_router_with_config(db, config);
        TestServer::new(app).expect("Failed to create test server")
    }

    #[tokio::test]
    async fn health_endpoint_is_accessible_without_auth() {
        let server = setup_with_auth("test-secret-key");

        let response = server.get("/api/v1/health").await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn protected_endpoint_requires_auth() {
        let server = setup_with_auth("test-secret-key");

        let response = server.get("/api/v1/projects").await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn protected_endpoint_accepts_valid_bearer_token() {
        let server = setup_with_auth("test-secret-key");

        let response = server
            .get("/api/v1/projects")
            .add_header("Authorization", "Bearer test-secret-key")
            .await;

        response.assert_status_ok();
    }

    #[tokio::test]
    async fn protected_endpoint_rejects_invalid_bearer_token() {
        let server = setup_with_auth("test-secret-key");

        let response = server
            .get("/api/v1/projects")
            .add_header("Authorization", "Bearer wrong-key")
            .await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn protected_endpoint_rejects_malformed_auth_header() {
        let server = setup_with_auth("test-secret-key");

        let response = server
            .get("/api/v1/projects")
            .add_header("Authorization", "Basic dXNlcjpwYXNz")
            .await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn post_endpoint_requires_auth() {
        let server = setup_with_auth("test-secret-key");

        let response = server
            .post("/api/v1/projects")
            .json(&CreateProjectInput {
                name: "Test".to_string(),
                description: None,
                instructions: None,
            })
            .await;

        response.assert_status(StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn post_endpoint_works_with_valid_auth() {
        let server = setup_with_auth("test-secret-key");

        let response = server
            .post("/api/v1/projects")
            .add_header("Authorization", "Bearer test-secret-key")
            .json(&CreateProjectInput {
                name: "Test".to_string(),
                description: None,
                instructions: None,
            })
            .await;

        response.assert_status(StatusCode::CREATED);
    }
}
