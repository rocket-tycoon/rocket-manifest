//! Domain models for RocketManifest.
//!
//! # Core Concepts
//!
//! ## Permanent Entities
//!
//! - [`Feature`]: Living documentation of system capabilities, forming a hierarchical tree.
//!   Any node can have content, but only leaf nodes can have sessions.
//! - [`FeatureHistory`]: Append-only log of work done on features (like `git log` for a feature).
//! - [`Project`]: Top-level container with associated directories and features.
//! - [`ImplementationNote`]: Permanent notes attached to features.
//!
//! ## Ephemeral Entities
//!
//! These exist only during active work and are deleted when sessions complete:
//!
//! - [`Session`]: Active work session on a leaf feature (one at a time per feature).
//! - [`Task`]: Work unit within a session, assigned to an AI agent.
//! - [`ImplementationNote`]: Notes attached to tasks (deleted with the task).

mod feature;
mod history;
mod note;
mod project;
mod session;
mod task;

pub use feature::*;
pub use history::*;
pub use note::*;
pub use project::*;
pub use session::*;
pub use task::*;
