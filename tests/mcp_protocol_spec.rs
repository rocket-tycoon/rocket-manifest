//! MCP protocol integration tests.
//!
//! These tests spawn the actual `rmf mcp` process and communicate via
//! JSON-RPC over stdio, testing the complete MCP protocol flow.
//!
//! The rmcp library uses line-delimited JSON (each message is one line):
//! ```
//! {"jsonrpc":"2.0","id":1,"method":"initialize",...}\n
//! {"jsonrpc":"2.0","id":1,"result":{...}}\n
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

/// JSON-RPC 2.0 request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

/// MCP test client that spawns and communicates with the server
struct McpTestClient {
    child: Child,
    request_id: u64,
    reader: BufReader<std::process::ChildStdout>,
}

impl McpTestClient {
    /// Spawn a new MCP server process with an isolated test database
    fn spawn() -> Self {
        // Create temp directory for test database
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        let mut child = Command::new(env!("CARGO_BIN_EXE_rmf"))
            .arg("mcp")
            .env("XDG_DATA_HOME", temp_dir.path())
            .env("HOME", temp_dir.path()) // For macOS directories crate
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn rmf mcp");

        let stdout = child.stdout.take().expect("Failed to get stdout");
        let reader = BufReader::new(stdout);

        // Keep temp_dir alive by leaking it (tests are short-lived anyway)
        std::mem::forget(temp_dir);

        Self {
            child,
            request_id: 0,
            reader,
        }
    }

    /// Send a message as line-delimited JSON
    fn send_message(&mut self, content: &str) {
        let stdin = self.child.stdin.as_mut().expect("Failed to get stdin");
        writeln!(stdin, "{}", content).expect("Failed to write message");
        stdin.flush().expect("Failed to flush stdin");
    }

    /// Read a message as line-delimited JSON
    fn read_message(&mut self) -> String {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .expect("Failed to read line");
        line.trim().to_string()
    }

    /// Send a JSON-RPC request and get the response
    fn request(&mut self, method: &str, params: Option<Value>) -> JsonRpcResponse {
        self.request_id += 1;
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: self.request_id,
            method: method.to_string(),
            params,
        };

        let request_json = serde_json::to_string(&request).expect("Failed to serialize request");
        self.send_message(&request_json);

        let response_json = self.read_message();
        serde_json::from_str(&response_json).expect("Failed to parse response")
    }

    /// Send initialize request and initialized notification (required first messages)
    fn initialize(&mut self) -> JsonRpcResponse {
        let response = self.request(
            "initialize",
            Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            })),
        );

        // Send initialized notification (required by MCP protocol)
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        self.send_message(&notification.to_string());

        response
    }

    /// List available tools
    fn list_tools(&mut self) -> JsonRpcResponse {
        self.request("tools/list", None)
    }

    /// Call a tool with parameters
    fn call_tool(&mut self, name: &str, arguments: Value) -> JsonRpcResponse {
        self.request(
            "tools/call",
            Some(json!({
                "name": name,
                "arguments": arguments
            })),
        )
    }
}

impl Drop for McpTestClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ============================================================
// Protocol Tests
// ============================================================

mod protocol {
    use super::*;

    #[test]
    fn initialize_returns_server_info() {
        let mut client = McpTestClient::spawn();
        let response = client.initialize();

        assert!(response.error.is_none(), "Expected success, got error");
        let result = response.result.expect("Expected result");

        // Check server info
        assert!(result.get("serverInfo").is_some());
        assert!(result.get("capabilities").is_some());
    }

    #[test]
    fn tools_list_returns_all_tools() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        let response = client.list_tools();
        assert!(response.error.is_none(), "Expected success, got error");

        let result = response.result.expect("Expected result");
        let tools = result.get("tools").expect("Expected tools array");
        let tools_array = tools.as_array().expect("Tools should be array");

        // We have 17 tools
        assert_eq!(
            tools_array.len(),
            17,
            "Expected 17 tools, got {}",
            tools_array.len()
        );

        // Verify tool names
        let tool_names: Vec<&str> = tools_array
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        assert!(tool_names.contains(&"get_task_context"));
        assert!(tool_names.contains(&"start_task"));
        assert!(tool_names.contains(&"complete_task"));
        assert!(tool_names.contains(&"create_session"));
        assert!(tool_names.contains(&"create_task"));
        assert!(tool_names.contains(&"list_session_tasks"));
        assert!(tool_names.contains(&"complete_session"));
        assert!(tool_names.contains(&"list_features"));
        assert!(tool_names.contains(&"search_features"));
        assert!(tool_names.contains(&"get_feature"));
        assert!(tool_names.contains(&"get_feature_history"));
        assert!(tool_names.contains(&"get_project_context"));
        assert!(tool_names.contains(&"update_feature_state"));
        assert!(tool_names.contains(&"create_project"));
        assert!(tool_names.contains(&"add_project_directory"));
        assert!(tool_names.contains(&"create_feature"));
        assert!(tool_names.contains(&"plan_features"));
    }

    #[test]
    fn tools_have_descriptions_and_schemas() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        let response = client.list_tools();
        let result = response.result.expect("Expected result");
        let tools = result
            .get("tools")
            .expect("Expected tools")
            .as_array()
            .expect("Tools should be array");

        for tool in tools {
            let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
            assert!(
                tool.get("description").is_some(),
                "Tool {} missing description",
                name
            );
            assert!(
                tool.get("inputSchema").is_some(),
                "Tool {} missing inputSchema",
                name
            );
        }
    }
}

// ============================================================
// Tool Call Tests
// ============================================================

mod tool_calls {
    use super::*;

    #[test]
    fn create_project_succeeds() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        let response = client.call_tool(
            "create_project",
            json!({
                "name": "Test Project",
                "description": "A test project",
                "instructions": "Follow TDD"
            }),
        );

        assert!(response.error.is_none(), "Expected success, got error");
        let result = response.result.expect("Expected result");

        // MCP tool results have content array
        let content = result.get("content").expect("Expected content");
        let text = content
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .expect("Expected text content");

        let project: Value = serde_json::from_str(text).expect("Expected JSON in text");
        assert_eq!(
            project.get("name").and_then(|n| n.as_str()),
            Some("Test Project")
        );
        assert!(project.get("id").is_some());
    }

    #[test]
    fn create_feature_and_list_features() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        // Create project first
        let project_response =
            client.call_tool("create_project", json!({ "name": "Feature Test Project" }));
        let project_text = extract_text_content(&project_response);
        let project: Value = serde_json::from_str(&project_text).unwrap();
        let project_id = project.get("id").and_then(|id| id.as_str()).unwrap();

        // Create feature
        let feature_response = client.call_tool(
            "create_feature",
            json!({
                "project_id": project_id,
                "title": "User Authentication",
                "details": "As a user, I want to log in",
                "state": "specified"
            }),
        );
        assert!(feature_response.error.is_none());

        let feature_text = extract_text_content(&feature_response);
        let feature: Value = serde_json::from_str(&feature_text).unwrap();
        assert_eq!(
            feature.get("title").and_then(|t| t.as_str()),
            Some("User Authentication")
        );

        // List features
        let list_response = client.call_tool("list_features", json!({ "project_id": project_id }));
        assert!(list_response.error.is_none());

        let list_text = extract_text_content(&list_response);
        let list: Value = serde_json::from_str(&list_text).unwrap();
        let features = list.get("features").and_then(|f| f.as_array()).unwrap();
        assert_eq!(features.len(), 1);
    }

    #[test]
    fn full_session_workflow() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        // Setup: create project and feature
        let project_text = extract_text_content(
            &client.call_tool("create_project", json!({ "name": "Workflow Test" })),
        );
        let project: Value = serde_json::from_str(&project_text).unwrap();
        let project_id = project["id"].as_str().unwrap();

        let feature_text = extract_text_content(&client.call_tool(
            "create_feature",
            json!({
                "project_id": project_id,
                "title": "Test Feature",
                "state": "specified"
            }),
        ));
        let feature: Value = serde_json::from_str(&feature_text).unwrap();
        let feature_id = feature["id"].as_str().unwrap();

        // 1. Create session
        let session_text = extract_text_content(&client.call_tool(
            "create_session",
            json!({
                "feature_id": feature_id,
                "goal": "Implement the feature"
            }),
        ));
        let session: Value = serde_json::from_str(&session_text).unwrap();
        let session_id = session["id"].as_str().unwrap();
        assert_eq!(session["status"].as_str(), Some("active"));

        // 2. Create task
        let task_text = extract_text_content(&client.call_tool(
            "create_task",
            json!({
                "session_id": session_id,
                "title": "Write tests",
                "scope": "Add unit tests for feature",
                "agent_type": "claude"
            }),
        ));
        let task: Value = serde_json::from_str(&task_text).unwrap();
        let task_id = task["id"].as_str().unwrap();
        assert_eq!(task["status"].as_str(), Some("pending"));

        // 3. Get task context
        let context_text = extract_text_content(
            &client.call_tool("get_task_context", json!({ "task_id": task_id })),
        );
        let context: Value = serde_json::from_str(&context_text).unwrap();
        assert_eq!(context["task"]["title"].as_str(), Some("Write tests"));
        assert_eq!(context["feature"]["title"].as_str(), Some("Test Feature"));

        // 4. Start task
        let start_response = client.call_tool("start_task", json!({ "task_id": task_id }));
        assert!(start_response.error.is_none());

        // 5. Complete task
        let complete_response = client.call_tool("complete_task", json!({ "task_id": task_id }));
        assert!(complete_response.error.is_none());

        // 6. Complete session
        let complete_session_text = extract_text_content(&client.call_tool(
            "complete_session",
            json!({
                "session_id": session_id,
                "summary": "Implemented tests",
                "files_changed": ["tests/feature_spec.rs"],
                "mark_implemented": true
            }),
        ));
        let complete_session: Value = serde_json::from_str(&complete_session_text).unwrap();
        assert_eq!(
            complete_session["feature_state"].as_str(),
            Some("implemented")
        );

        // 7. Verify feature state changed
        let get_feature_text = extract_text_content(
            &client.call_tool("get_feature", json!({ "feature_id": feature_id })),
        );
        let updated_feature: Value = serde_json::from_str(&get_feature_text).unwrap();
        assert_eq!(updated_feature["state"].as_str(), Some("implemented"));
    }

    #[test]
    fn add_directory_and_get_project_context() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        // Create project
        let project_text = extract_text_content(&client.call_tool(
            "create_project",
            json!({
                "name": "Directory Test",
                "instructions": "Use TDD"
            }),
        ));
        let project: Value = serde_json::from_str(&project_text).unwrap();
        let project_id = project["id"].as_str().unwrap();

        // Add directory
        let dir_response = client.call_tool(
            "add_project_directory",
            json!({
                "project_id": project_id,
                "path": "/Users/test/my-project",
                "is_primary": true,
                "instructions": "cargo test"
            }),
        );
        assert!(dir_response.error.is_none());

        // Get project context
        let context_text = extract_text_content(&client.call_tool(
            "get_project_context",
            json!({ "directory_path": "/Users/test/my-project/src" }),
        ));
        let context: Value = serde_json::from_str(&context_text).unwrap();
        assert_eq!(context["project"]["name"].as_str(), Some("Directory Test"));
        assert_eq!(context["project"]["instructions"].as_str(), Some("Use TDD"));
        assert_eq!(
            context["directory"]["path"].as_str(),
            Some("/Users/test/my-project")
        );
    }

    /// Helper to extract text content from MCP tool response
    fn extract_text_content(response: &JsonRpcResponse) -> String {
        response
            .result
            .as_ref()
            .and_then(|r| r.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .expect("Expected text content in response")
            .to_string()
    }
}

// ============================================================
// Error Handling Tests
// ============================================================

mod errors {
    use super::*;

    #[test]
    fn invalid_tool_name_returns_error() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        let response = client.call_tool("nonexistent_tool", json!({}));

        assert!(response.error.is_some(), "Expected error for invalid tool");
    }

    #[test]
    fn invalid_uuid_returns_error() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        let response = client.call_tool("get_feature", json!({ "feature_id": "not-a-uuid" }));

        assert!(
            response.error.is_some() || {
                // Some implementations return error in result
                response
                    .result
                    .as_ref()
                    .and_then(|r| r.get("isError"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false)
            }
        );
    }

    #[test]
    fn missing_required_param_returns_error() {
        let mut client = McpTestClient::spawn();
        client.initialize();

        // create_project requires 'name'
        let response = client.call_tool("create_project", json!({}));

        assert!(
            response.error.is_some() || {
                response
                    .result
                    .as_ref()
                    .and_then(|r| r.get("isError"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false)
            }
        );
    }
}
