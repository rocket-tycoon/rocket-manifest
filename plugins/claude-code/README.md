# RocketManifest Claude Code Plugin

Living feature documentation for AI-assisted development.

## Installation

```bash
# Add the marketplace
/plugin marketplace add rocket-tycoon/rocket-manifest

# Install the plugin
/plugin install rocketmanifest
```

## What it does

RocketManifest provides MCP tools for managing features as living documentation:

- **Project Management**: Create projects and associate directories
- **Feature Tracking**: Define features as system capabilities (not work items)
- **Session Workflow**: Create sessions and tasks for implementing features
- **Agent Coordination**: Get task context, start/complete tasks

## Tools Available

| Tool | Description |
|------|-------------|
| `create_project` | Create a new project with coding instructions |
| `add_project_directory` | Associate a directory with a project |
| `create_feature` | Define a system capability |
| `list_features` | Browse features by project or state |
| `get_feature` | Get full feature details |
| `get_project_context` | Find project from current directory |
| `create_session` | Start work on a leaf feature |
| `create_task` | Break work into agent-sized units |
| `get_task_context` | Get task assignment with feature context |
| `start_task` / `complete_task` | Signal task progress |
| `complete_session` | Finish work and record history |

## Philosophy

Features are **living documentation** of system capabilities - not work items to close.

- Name by capability: "Router", "Authentication", "Validation"
- NOT by phase: "Phase 1", "Step 2", "Sprint 3"
- Features persist and evolve with the codebase
