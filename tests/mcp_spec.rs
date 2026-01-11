//! MCP server integration tests.
//!
//! Tests are organized into two sections:
//! - Agent tools: Used by agents working on assigned tasks
//! - Orchestrator tools: Used to manage sessions and tasks

use rocket_manifest::db::Database;
use rocket_manifest::mcp::McpServer;
use rocket_manifest::models::*;

/// Helper to create a test MCP server with in-memory database.
fn setup() -> (McpServer, Database) {
    let db = Database::open_memory().expect("Failed to create database");
    db.migrate().expect("Failed to migrate");
    let server = McpServer::new(db.clone());
    (server, db)
}

/// Helper to create a test project.
fn create_test_project(db: &Database) -> Project {
    db.create_project(CreateProjectInput {
        name: "Test Project".to_string(),
        description: None,
        instructions: None,
    })
    .expect("Failed to create project")
}

/// Helper to create a test feature.
fn create_test_feature(db: &Database, project_id: uuid::Uuid) -> Feature {
    db.create_feature(
        project_id,
        CreateFeatureInput {
            parent_id: None,
            title: "Test Feature".to_string(),
            story: Some("As a user, I want to test".to_string()),
            details: Some("Implementation details".to_string()),
            state: Some(FeatureState::Specified),
        },
    )
    .expect("Failed to create feature")
}

// ============================================================
// Agent Tools Tests
// ============================================================

mod agent_tools {
    use super::*;

    mod get_task_context {
        use super::*;

        #[tokio::test]
        async fn returns_task_with_feature_context() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            // Create session and task
            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Test goal".to_string(),
                    tasks: vec![CreateTaskInput {
                        parent_id: None,
                        title: "Test Task".to_string(),
                        scope: "Test scope".to_string(),
                        agent_type: AgentType::Claude,
                    }],
                })
                .expect("Failed to create session");

            let task_id = session_response.tasks[0].id.to_string();

            let response = server
                .test_get_task_context(&task_id)
                .await
                .expect("Tool failed");

            assert_eq!(response.task.id, task_id);
            assert_eq!(response.task.title, "Test Task");
            assert_eq!(response.task.scope, "Test scope");
            assert_eq!(response.feature.title, "Test Feature");
            assert_eq!(response.session_goal, "Test goal");
        }

        #[tokio::test]
        async fn returns_error_for_invalid_uuid() {
            let (server, _db) = setup();

            let result = server.test_get_task_context("not-a-uuid").await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_task() {
            let (server, _db) = setup();

            let result = server
                .test_get_task_context(&uuid::Uuid::new_v4().to_string())
                .await;

            assert!(result.is_err());
        }
    }

    mod start_task {
        use super::*;

        #[tokio::test]
        async fn sets_task_status_to_running() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Test".to_string(),
                    tasks: vec![CreateTaskInput {
                        parent_id: None,
                        title: "Task".to_string(),
                        scope: "Scope".to_string(),
                        agent_type: AgentType::Claude,
                    }],
                })
                .expect("Failed to create session");

            let task_id = session_response.tasks[0].id;

            server
                .test_start_task(&task_id.to_string())
                .expect("Tool failed");

            // Verify in database
            let task = db.get_task(task_id).expect("Query failed").unwrap();
            assert_eq!(task.status, TaskStatus::Running);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_task() {
            let (server, _db) = setup();

            let result = server.test_start_task(&uuid::Uuid::new_v4().to_string());

            assert!(result.is_err());
        }
    }

    mod complete_task {
        use super::*;

        #[tokio::test]
        async fn sets_task_status_to_completed() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Test".to_string(),
                    tasks: vec![CreateTaskInput {
                        parent_id: None,
                        title: "Task".to_string(),
                        scope: "Scope".to_string(),
                        agent_type: AgentType::Claude,
                    }],
                })
                .expect("Failed to create session");

            let task_id = session_response.tasks[0].id;

            server
                .test_complete_task(&task_id.to_string())
                .expect("Tool failed");

            // Verify in database
            let task = db.get_task(task_id).expect("Query failed").unwrap();
            assert_eq!(task.status, TaskStatus::Completed);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_task() {
            let (server, _db) = setup();

            let result = server.test_complete_task(&uuid::Uuid::new_v4().to_string());

            assert!(result.is_err());
        }
    }
}

// ============================================================
// Orchestrator Tools Tests
// ============================================================

mod orchestrator_tools {
    use super::*;

    mod create_session {
        use super::*;

        #[tokio::test]
        async fn creates_session_on_leaf_feature() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let response = server
                .test_create_session(&feature.id.to_string(), "Implement the feature")
                .expect("Tool failed");

            assert_eq!(response.feature_id, feature.id.to_string());
            assert_eq!(response.goal, "Implement the feature");
            assert_eq!(response.status, "active");
        }

        #[tokio::test]
        async fn rejects_session_on_non_leaf_feature() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let parent = create_test_feature(&db, project.id);

            // Create a child to make parent non-leaf
            db.create_feature(
                project.id,
                CreateFeatureInput {
                    parent_id: Some(parent.id),
                    title: "Child".to_string(),
                    story: None,
                    details: None,
                    state: None,
                },
            )
            .expect("Failed to create child");

            let result = server.test_create_session(&parent.id.to_string(), "Goal");

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_feature() {
            let (server, _db) = setup();

            let result = server.test_create_session(&uuid::Uuid::new_v4().to_string(), "Goal");

            assert!(result.is_err());
        }
    }

    mod create_task {
        use super::*;

        #[tokio::test]
        async fn creates_task_in_session() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Goal".to_string(),
                    tasks: vec![],
                })
                .expect("Failed to create session");

            let response = server
                .test_create_task(
                    &session_response.session.id.to_string(),
                    "New Task",
                    "Task scope",
                    "claude",
                )
                .expect("Tool failed");

            assert_eq!(response.title, "New Task");
            assert_eq!(response.scope, "Task scope");
            assert_eq!(response.status, "pending");
            assert_eq!(response.agent_type, "claude");
        }

        #[tokio::test]
        async fn returns_error_for_invalid_agent_type() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Goal".to_string(),
                    tasks: vec![],
                })
                .expect("Failed to create session");

            let result = server.test_create_task(
                &session_response.session.id.to_string(),
                "Task",
                "Scope",
                "invalid",
            );

            assert!(result.is_err());
        }
    }

    mod list_session_tasks {
        use super::*;

        #[tokio::test]
        async fn returns_all_tasks_in_session() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
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
                .expect("Failed to create session");

            let response = server
                .test_list_session_tasks(&session_response.session.id.to_string())
                .expect("Tool failed");

            assert_eq!(response.tasks.len(), 2);
        }

        #[tokio::test]
        async fn returns_empty_list_for_session_with_no_tasks() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Goal".to_string(),
                    tasks: vec![],
                })
                .expect("Failed to create session");

            let response = server
                .test_list_session_tasks(&session_response.session.id.to_string())
                .expect("Tool failed");

            assert!(response.tasks.is_empty());
        }
    }

    mod complete_session {
        use super::*;

        #[tokio::test]
        async fn completes_session_and_creates_history_entry() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Goal".to_string(),
                    tasks: vec![],
                })
                .expect("Failed to create session");

            let response = server
                .test_complete_session(
                    &session_response.session.id.to_string(),
                    "Work completed",
                    vec!["src/main.rs".to_string()],
                    true,
                )
                .expect("Tool failed");

            assert_eq!(response.feature_state, "implemented");

            // Verify session completed
            let session = db
                .get_session(session_response.session.id)
                .expect("Query failed")
                .unwrap();
            assert_eq!(session.status, SessionStatus::Completed);

            // Verify feature state updated
            let feature = db.get_feature(feature.id).expect("Query failed").unwrap();
            assert_eq!(feature.state, FeatureState::Implemented);
        }

        #[tokio::test]
        async fn does_not_change_feature_state_when_mark_implemented_is_false() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let session_response = db
                .create_session(CreateSessionInput {
                    feature_id: feature.id,
                    goal: "Goal".to_string(),
                    tasks: vec![],
                })
                .expect("Failed to create session");

            let response = server
                .test_complete_session(
                    &session_response.session.id.to_string(),
                    "Partial work",
                    vec![],
                    false,
                )
                .expect("Tool failed");

            assert_eq!(response.feature_state, "unchanged");

            // Verify feature state NOT updated
            let feature = db.get_feature(feature.id).expect("Query failed").unwrap();
            assert_eq!(feature.state, FeatureState::Specified);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_session() {
            let (server, _db) = setup();

            let result = server.test_complete_session(
                &uuid::Uuid::new_v4().to_string(),
                "Summary",
                vec![],
                true,
            );

            assert!(result.is_err());
        }
    }
}

// ============================================================
// Discovery Tools Tests
// ============================================================

mod discovery_tools {
    use super::*;

    mod list_features {
        use super::*;

        #[tokio::test]
        async fn returns_all_features_when_no_filter() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            create_test_feature(&db, project.id);

            db.create_feature(
                project.id,
                CreateFeatureInput {
                    parent_id: None,
                    title: "Second Feature".to_string(),
                    story: None,
                    details: None,
                    state: Some(FeatureState::Proposed),
                },
            )
            .expect("Failed to create feature");

            let response = server.test_list_features(None, None).expect("Tool failed");

            assert_eq!(response.features.len(), 2);
        }

        #[tokio::test]
        async fn filters_by_project() {
            let (server, db) = setup();
            let project1 = create_test_project(&db);
            let project2 = db
                .create_project(CreateProjectInput {
                    name: "Other Project".to_string(),
                    description: None,
                    instructions: None,
                })
                .expect("Failed to create project");

            create_test_feature(&db, project1.id);
            db.create_feature(
                project2.id,
                CreateFeatureInput {
                    parent_id: None,
                    title: "Other Feature".to_string(),
                    story: None,
                    details: None,
                    state: None,
                },
            )
            .expect("Failed to create feature");

            let response = server
                .test_list_features(Some(&project1.id.to_string()), None)
                .expect("Tool failed");

            assert_eq!(response.features.len(), 1);
            assert_eq!(response.features[0].title, "Test Feature");
        }

        #[tokio::test]
        async fn filters_by_state() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            create_test_feature(&db, project.id); // state = Specified

            db.create_feature(
                project.id,
                CreateFeatureInput {
                    parent_id: None,
                    title: "Proposed Feature".to_string(),
                    story: None,
                    details: None,
                    state: Some(FeatureState::Proposed),
                },
            )
            .expect("Failed to create feature");

            let response = server
                .test_list_features(None, Some("specified"))
                .expect("Tool failed");

            assert_eq!(response.features.len(), 1);
            assert_eq!(response.features[0].state, "specified");
        }

        #[tokio::test]
        async fn returns_error_for_invalid_state() {
            let (server, _db) = setup();

            let result = server.test_list_features(None, Some("invalid"));

            assert!(result.is_err());
        }
    }

    mod get_feature {
        use super::*;

        #[tokio::test]
        async fn returns_feature_details() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let response = server
                .test_get_feature(&feature.id.to_string())
                .expect("Tool failed");

            assert_eq!(response.id, feature.id.to_string());
            assert_eq!(response.title, "Test Feature");
            assert_eq!(
                response.story,
                Some("As a user, I want to test".to_string())
            );
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_feature() {
            let (server, _db) = setup();

            let result = server.test_get_feature(&uuid::Uuid::new_v4().to_string());

            assert!(result.is_err());
        }
    }

    mod get_project_context {
        use super::*;

        #[tokio::test]
        async fn returns_project_for_exact_directory_match() {
            let (server, db) = setup();
            let project = db
                .create_project(CreateProjectInput {
                    name: "My Project".to_string(),
                    description: Some("A test project".to_string()),
                    instructions: Some("Follow coding-guidelines.md".to_string()),
                })
                .expect("Failed to create project");

            db.add_project_directory(
                project.id,
                AddDirectoryInput {
                    path: "/Users/dev/my-project".to_string(),
                    git_remote: Some("git@github.com:org/repo.git".to_string()),
                    is_primary: true,
                    instructions: Some("Run tests with cargo test".to_string()),
                },
            )
            .expect("Failed to add directory");

            let response = server
                .test_get_project_context("/Users/dev/my-project")
                .expect("Tool failed");

            assert_eq!(response.project.name, "My Project");
            assert_eq!(
                response.project.instructions,
                Some("Follow coding-guidelines.md".to_string())
            );
            assert_eq!(response.directory.path, "/Users/dev/my-project");
            assert!(response.directory.is_primary);
            assert_eq!(
                response.directory.instructions,
                Some("Run tests with cargo test".to_string())
            );
        }

        #[tokio::test]
        async fn returns_project_for_subdirectory() {
            let (server, db) = setup();
            let project = create_test_project(&db);

            db.add_project_directory(
                project.id,
                AddDirectoryInput {
                    path: "/Users/dev/project".to_string(),
                    git_remote: None,
                    is_primary: true,
                    instructions: None,
                },
            )
            .expect("Failed to add directory");

            let response = server
                .test_get_project_context("/Users/dev/project/src/components")
                .expect("Tool failed");

            assert_eq!(response.project.name, "Test Project");
        }

        #[tokio::test]
        async fn returns_error_for_unknown_directory() {
            let (server, _db) = setup();

            let result = server.test_get_project_context("/unknown/path");

            assert!(result.is_err());
        }
    }

    mod update_feature_state {
        use super::*;

        #[tokio::test]
        async fn updates_feature_state() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id); // state = Specified

            let response = server
                .test_update_feature_state(&feature.id.to_string(), "implemented")
                .expect("Tool failed");

            assert_eq!(response.state, "implemented");

            // Verify in database
            let feature = db.get_feature(feature.id).expect("Query failed").unwrap();
            assert_eq!(feature.state, FeatureState::Implemented);
        }

        #[tokio::test]
        async fn returns_error_for_invalid_state() {
            let (server, db) = setup();
            let project = create_test_project(&db);
            let feature = create_test_feature(&db, project.id);

            let result = server.test_update_feature_state(&feature.id.to_string(), "invalid");

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_feature() {
            let (server, _db) = setup();

            let result =
                server.test_update_feature_state(&uuid::Uuid::new_v4().to_string(), "implemented");

            assert!(result.is_err());
        }
    }
}
