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
    server.post("/api/v1/projects")
        .json(&CreateProjectInput {
            name: "Test Project".to_string(),
            description: None,
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

        let response = server.get(&format!("/api/v1/projects/{}/features/roots", project.id)).await;

        response.assert_status_ok();
        let features: Vec<Feature> = response.json();
        assert!(features.is_empty());
    }

    #[tokio::test]
    async fn returns_only_root_features() {
        let server = setup();
        let project = create_test_project(&server).await;

        // Create root feature
        let root = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Root".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        // Create child feature
        server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Child".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await;

        let response = server.get(&format!("/api/v1/projects/{}/features/roots", project.id)).await;

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

        let feature = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Leaf".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let response = server.get(&format!("/api/v1/features/{}/children", feature.id)).await;

        response.assert_status_ok();
        let children: Vec<Feature> = response.json();
        assert!(children.is_empty());
    }

    #[tokio::test]
    async fn returns_direct_children_ordered_by_title() {
        let server = setup();
        let project = create_test_project(&server).await;

        let parent = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Zebra".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await;

        server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Alpha".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await;

        let response = server.get(&format!("/api/v1/features/{}/children", parent.id)).await;

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

        let root = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Root".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let child = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(root.id),
                title: "Child".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(child.id),
                title: "Grandchild".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await;

        let response = server.get(&format!("/api/v1/features/{}/children", root.id)).await;

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

        let parent = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Authentication".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let response = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Login".to_string(),
                state: None,
                story: None,
                details: None,
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

        let level0 = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Authentication".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let level1 = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(level0.id),
                title: "OAuth".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let level2 = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(level1.id),
                title: "Google".to_string(),
                state: None,
                story: None,
                details: None,
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

        let parent = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let child = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Child".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        // Delete parent
        server.delete(&format!("/api/v1/features/{}", parent.id))
            .await
            .assert_status(StatusCode::NO_CONTENT);

        // Child should be gone
        server.get(&format!("/api/v1/features/{}", child.id))
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

        let feature = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "New Feature".to_string(),
                story: None,
                details: None,
                state: None,
            })
            .await
            .json::<Feature>();

        let response = server.get(&format!("/api/v1/features/{}/history", feature.id)).await;

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

        let leaf = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Leaf Feature".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let response = server.post("/api/v1/sessions")
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

        let parent = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Parent".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: Some(parent.id),
                title: "Child".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await;

        let response = server.post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: parent.id,
                goal: "Implement feature".to_string(),
                tasks: vec![],
            })
            .await;

        response.assert_status_internal_server_error();
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

        let feature = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let session = server.post("/api/v1/sessions")
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

        let response = server.post(&format!("/api/v1/sessions/{}/complete", session.session.id))
            .json(&CompleteSessionInput {
                summary: "Feature implemented".to_string(),
                feature_state: None,
            })
            .await;

        response.assert_status_ok();
        let result: SessionCompletionResult = response.json();
        assert_eq!(result.session.status, SessionStatus::Completed);
        assert!(result.session.completed_at.is_some());
        assert_eq!(result.history_entry.summary, "Feature implemented");
    }

    #[tokio::test]
    async fn creates_history_entry_on_completion() {
        let server = setup();
        let project = create_test_project(&server).await;

        let feature = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Feature".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let session = server.post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Goal".to_string(),
                tasks: vec![],
            })
            .await
            .json::<SessionResponse>();

        server.post(&format!("/api/v1/sessions/{}/complete", session.session.id))
            .json(&CompleteSessionInput {
                summary: "Work completed".to_string(),
                feature_state: None,
            })
            .await;

        // Check history was created
        let response = server.get(&format!("/api/v1/features/{}/history", feature.id)).await;
        let history: Vec<FeatureHistory> = response.json();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].summary, "Work completed");
    }

    #[tokio::test]
    async fn returns_not_found_for_nonexistent_session() {
        let server = setup();
        let fake_id = uuid::Uuid::new_v4();

        let response = server.post(&format!("/api/v1/sessions/{}/complete", fake_id))
            .json(&CompleteSessionInput {
                summary: "Done".to_string(),
                feature_state: None,
            })
            .await;

        response.assert_status_not_found();
    }
}

mod implementation_notes {
    use super::*;

    async fn create_task_for_test(server: &TestServer) -> (Project, Feature, SessionResponse) {
        let project = create_test_project(server).await;

        let feature = server.post(&format!("/api/v1/projects/{}/features", project.id))
            .json(&CreateFeatureInput {
                parent_id: None,
                title: "Test Feature".to_string(),
                state: None,
                story: None,
                details: None,
            })
            .await
            .json::<Feature>();

        let session = server.post("/api/v1/sessions")
            .json(&CreateSessionInput {
                feature_id: feature.id,
                goal: "Test goal".to_string(),
                tasks: vec![CreateTaskInput {
                    parent_id: None,
                    title: "Test Task".to_string(),
                    scope: "Test scope".to_string(),
                    agent_type: AgentType::Claude,
                }],
            })
            .await
            .json::<SessionResponse>();

        (project, feature, session)
    }

    #[tokio::test]
    async fn returns_empty_list_when_no_notes() {
        let server = setup();
        let (_, _, session) = create_task_for_test(&server).await;
        let task_id = session.tasks[0].id;

        let response = server.get(&format!("/api/v1/tasks/{}/notes", task_id)).await;

        response.assert_status_ok();
        let notes: Vec<ImplementationNote> = response.json();
        assert!(notes.is_empty());
    }

    #[tokio::test]
    async fn creates_note_for_task() {
        let server = setup();
        let (_, _, session) = create_task_for_test(&server).await;
        let task_id = session.tasks[0].id;

        let response = server.post(&format!("/api/v1/tasks/{}/notes", task_id))
            .json(&CreateImplementationNoteInput {
                content: "Implemented login flow using JWT".to_string(),
                files_changed: vec!["src/auth.rs".to_string()],
            })
            .await;

        response.assert_status(StatusCode::CREATED);
        let note: ImplementationNote = response.json();
        assert_eq!(note.content, "Implemented login flow using JWT");
        assert_eq!(note.files_changed, vec!["src/auth.rs"]);
        assert_eq!(note.task_id, Some(task_id));
    }

    #[tokio::test]
    async fn lists_notes_for_task() {
        let server = setup();
        let (_, _, session) = create_task_for_test(&server).await;
        let task_id = session.tasks[0].id;

        server.post(&format!("/api/v1/tasks/{}/notes", task_id))
            .json(&CreateImplementationNoteInput {
                content: "First note".to_string(),
                files_changed: vec![],
            })
            .await;

        server.post(&format!("/api/v1/tasks/{}/notes", task_id))
            .json(&CreateImplementationNoteInput {
                content: "Second note".to_string(),
                files_changed: vec![],
            })
            .await;

        let response = server.get(&format!("/api/v1/tasks/{}/notes", task_id)).await;

        response.assert_status_ok();
        let notes: Vec<ImplementationNote> = response.json();
        assert_eq!(notes.len(), 2);
    }

    #[tokio::test]
    async fn lists_notes_for_feature() {
        let server = setup();
        let (_, feature, session) = create_task_for_test(&server).await;
        let task_id = session.tasks[0].id;

        server.post(&format!("/api/v1/tasks/{}/notes", task_id))
            .json(&CreateImplementationNoteInput {
                content: "Note for feature".to_string(),
                files_changed: vec![],
            })
            .await;

        let response = server.get(&format!("/api/v1/features/{}/notes", feature.id)).await;

        response.assert_status_ok();
        let notes: Vec<ImplementationNote> = response.json();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].feature_id, Some(feature.id));
    }
}

