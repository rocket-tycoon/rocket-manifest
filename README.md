# RocketManifest

Living feature documentation for AI-assisted development.

RocketManifest is an MCP server and HTTP API that tracks **features** (system capabilities) rather than work items. Unlike traditional issue trackers where items are closed and forgotten, features are living descriptions that evolve with your codebase.

## Quick Start

```bash
# Start the server
rocket-manifest serve

# Or run as MCP server for Claude Code
rocket-manifest mcp
```

## Installation

### From Source

```bash
git clone https://github.com/rocket-tycoon/rocket-manifest
cd rocket-manifest
cargo install --path .
```

### Homebrew (coming soon)

```bash
brew install rocket-tycoon/tap/rocket-manifest
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Feature** | A capability of the system, organized in a hierarchical tree. Features progress through states: `proposed` → `specified` → `implemented` → `deprecated` |
| **Session** | A work session on a leaf feature. Only one active session per feature at a time. When completed, creates a history entry. |
| **Task** | A unit of work within a session, assigned to an AI agent. Small enough for one agent (1-3 story points). |
| **History** | Append-only log of implementation sessions—like `git log` for a feature |

### Feature Lifecycle

```
Traditional Tools          RocketManifest
─────────────────          ──────────────
Issue (work item)    →     Feature (system capability)
Open → Closed        →     Proposed → Implemented → Living
Changelog of what    →     Description of what IS
  happened
```

Features are not work items to be closed. They are living documentation that evolves with the codebase.

---

# For Users

## CLI Commands

```bash
# Start HTTP server on default port (3000)
rocket-manifest serve

# Start on custom port
rocket-manifest serve -p 8080

# Start MCP server via stdio (for Claude Code)
rocket-manifest mcp

# Check server status
rocket-manifest status

# Stop the server
rocket-manifest stop
```

## Claude Code Integration

Add RocketManifest as an MCP server in your Claude Code configuration:

```json
{
  "mcpServers": {
    "rocket-manifest": {
      "command": "rocket-manifest",
      "args": ["mcp"]
    }
  }
}
```

### MCP Tools

**Agent Tools** (for AI agents working on assigned tasks):

| Tool | Description |
|------|-------------|
| `get_task_context` | Get your assigned task with full feature context. Call this FIRST. |
| `start_task` | Signal you're beginning work. Sets status to `running`. |
| `add_implementation_note` | Document decisions, progress, or blockers. |
| `complete_task` | Signal task is finished. Only call when verified. |

**Orchestrator Tools** (for managing sessions and tasks):

| Tool | Description |
|------|-------------|
| `create_session` | Start work session on a leaf feature. |
| `create_task` | Create a task within a session. |
| `list_session_tasks` | Monitor progress of all tasks. |
| `complete_session` | Finalize session, mark feature as implemented. |

### Agent Workflow

```
1. get_task_context    → Understand the assignment
2. start_task          → Signal work is beginning
3. [implement]         → Write code, run tests
4. add_implementation_note → Document what was done
5. complete_task       → Signal completion
```

### Orchestrator Workflow

```
1. create_session      → Start work on a feature
2. create_task (×N)    → Break work into agent-sized units
3. [spawn agents]      → Each agent gets a task_id
4. list_session_tasks  → Monitor progress
5. complete_session    → Finalize and mark implemented
```

## HTTP API

Base URL: `http://localhost:3000/api/v1`

Full API documentation is available in [openapi.yaml](./openapi.yaml).

### Key Endpoints

```bash
# Projects
GET    /projects                    # List all projects
POST   /projects                    # Create project
GET    /projects/{id}/features      # List features for project
GET    /projects/{id}/features/tree # Get complete feature tree

# Features
GET    /features/{id}               # Get feature
PUT    /features/{id}               # Update feature
GET    /features/{id}/children      # Get direct children
GET    /features/{id}/history       # Get implementation history

# Sessions (leaf features only)
POST   /sessions                    # Create session
GET    /sessions/{id}/status        # Get status with tasks
POST   /sessions/{id}/complete      # Complete session

# Tasks
GET    /tasks/{id}                  # Get task
PUT    /tasks/{id}                  # Update task status
POST   /tasks/{id}/notes            # Add implementation note
```

### Example: Create a Feature and Session

```bash
# Create a project
curl -X POST http://localhost:3000/api/v1/projects \
  -H "Content-Type: application/json" \
  -d '{"name": "my-app", "description": "My application"}'

# Create a feature
curl -X POST http://localhost:3000/api/v1/projects/{project_id}/features \
  -H "Content-Type: application/json" \
  -d '{
    "title": "User Authentication",
    "story": "As a user, I want to log in so I can access my account",
    "state": "specified"
  }'

# Start a session
curl -X POST http://localhost:3000/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{
    "feature_id": "{feature_id}",
    "goal": "Implement login flow",
    "tasks": [{
      "title": "Login form",
      "scope": "Create login form with email/password validation",
      "agent_type": "claude"
    }]
  }'
```

## Data Storage

| Platform | Location |
|----------|----------|
| macOS | `~/.local/share/legion/legion.db` |
| Linux | `~/.local/share/legion/legion.db` |
| Windows | `%APPDATA%\legion\legion.db` |

The database auto-migrates on startup.

---

# For Contributors

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

## Project Structure

```
src/
├── main.rs          # CLI entry point (clap)
├── lib.rs           # Library root
├── api/
│   ├── mod.rs       # Router setup, all routes under /api/v1
│   └── handlers/    # Request handlers
├── db/
│   ├── mod.rs       # Database wrapper with CRUD operations
│   └── schema.rs    # SQLite schema (embedded, auto-migrated)
├── mcp/
│   ├── mod.rs       # MCP server and tool handlers
│   └── types.rs     # Request/response types for MCP tools
└── models/          # Domain types (Feature, Session, Task, etc.)

tests/
├── api_spec.rs      # HTTP API integration tests
└── db_spec.rs       # Database unit tests
```

## Testing

Tests use [speculate2](https://crates.io/crates/speculate2) for BDD-style specs:

```rust
speculate! {
    describe "features" {
        before {
            let db = Database::open_memory().expect("...");
            db.migrate().expect("...");
        }

        it "creates a feature" {
            // ...
        }
    }
}
```

Run tests:

```bash
cargo test                    # All tests
cargo test db_spec            # Database tests only
cargo test api_spec           # API tests only
```

## Code Patterns

- **Enums**: Use manual `as_str()`/`from_str()` for DB serialization (not derive macros)
- **Error handling**: `Result<Option<T>>` for get operations (None = not found, Err = DB error)
- **Updates**: Dynamic SQL building for partial updates (`UpdateFeatureInput`, etc.)
- **Thread safety**: Database wrapped in `Arc<Mutex<Connection>>`

## Contract-First Development

When adding or modifying API endpoints:

1. Update `openapi.yaml` first (or immediately after implementation)
2. Add tests for the new behavior
3. Implement the feature

The OpenAPI spec is the source of truth for the HTTP API.

## Architecture Decisions

### Why SQLite?

- Single-file database, no external dependencies
- WAL mode for concurrent reads
- Auto-migrates on startup
- Portable across platforms

### Why MCP + HTTP?

- **MCP**: Direct integration with Claude Code and other AI tools
- **HTTP**: For VSCode extension, web UI, CLI tools, and any HTTP client

### Why Features Instead of Issues?

Features are living documentation. Unlike issues that are "closed and forgotten," features describe the current state of the system and evolve with the codebase.

## Related Projects

- **RocketCrew** - VSCode extension that consumes this API
- **RocketIndex** - Code indexer for semantic navigation

## License

[Business Source License 1.1](./LICENSE) - Free for personal and internal use. Converts to Apache 2.0 after 4 years.

---

## Quick Reference

### Feature States

| State | Description |
|-------|-------------|
| `proposed` | Initial idea, not yet fully specified |
| `specified` | Requirements defined, ready for implementation |
| `implemented` | Built and deployed (enters "living" phase) |
| `deprecated` | No longer active, kept for historical reference |

### Task States

| State | Description |
|-------|-------------|
| `pending` | Not yet started |
| `running` | Agent is working on it |
| `completed` | Work finished successfully |
| `failed` | Work could not be completed |

### Agent Types

| Type | Description |
|------|-------------|
| `claude` | Anthropic Claude |
| `gemini` | Google Gemini |
| `codex` | OpenAI Codex |
