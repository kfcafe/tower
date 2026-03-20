//! # Beans Library API
//!
//! Programmatic access to units operations. Use this module when embedding units
//! in another application (e.g., a GUI, MCP server, or custom tooling).
//!
//! The API is organized into layers:
//!
//! - **Types** — Core data structures (`Unit`, `Index`, `Status`, etc.)
//! - **Discovery** — Find `.mana/` directories and unit files
//! - **Query** — Read-only operations (list, get, tree, status, graph)
//! - **Mutations** — Write operations (create, update, close, delete)
//! - **Orchestration** — Agent dispatch, monitoring, and control
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use mana_core::api::*;
//!
//! // Find the .mana/ directory
//! let mana_dir = mana_core::discovery::find_mana_dir(std::path::Path::new(".")).unwrap();
//!
//! // Load the index (cached, rebuilds if stale)
//! let index = Index::load_or_rebuild(&mana_dir).unwrap();
//!
//! // Get a specific unit
//! let unit = get_bean(&mana_dir, "1").unwrap();
//! println!("{}: {}", unit.id, unit.title);
//! ```
//!
//! ## Design Principles
//!
//! - **No I/O side effects** — Library functions never print to stdout/stderr.
//!   All output is returned as structured data.
//! - **Structured params and results** — Each operation takes a `Params` struct
//!   and returns a `Result` type. No raw CLI argument passing.
//! - **Serializable** — All types derive `Serialize`/`Deserialize` for easy
//!   IPC (Tauri, JSON-RPC, MCP).
//! - **Composable** — Functions take `&Path` (mana_dir) and return owned data.
//!   No global state, no singletons.

use std::path::Path;

use anyhow::Result;

// ---------------------------------------------------------------------------
// Re-exported core types
// ---------------------------------------------------------------------------

// Unit and related types
pub use crate::unit::{
    AttemptOutcome, AttemptRecord, Unit, OnCloseAction, OnFailAction, RunRecord, RunResult, Status,
};

// Index types
pub use crate::index::{Index, IndexEntry};

// Configuration
pub use crate::config::Config;

// Discovery functions
pub use crate::discovery::{
    archive_path_for_bean, find_archived_unit, find_unit_file, find_mana_dir,
};

// Graph functions
pub use crate::graph::{
    build_dependency_tree, build_full_graph, count_subtree_attempts, detect_cycle, find_all_cycles,
};

// Utility
pub use crate::unit::validate_priority;

// ---------------------------------------------------------------------------
// Query functions
// ---------------------------------------------------------------------------

/// Load a unit by ID.
///
/// Finds the unit file in the `.mana/` directory and deserializes it.
/// Works for both active and legacy unit formats.
///
/// # Errors
/// - Unit ID is invalid
/// - No unit file found for the given ID
/// - File cannot be parsed
pub fn get_bean(mana_dir: &Path, id: &str) -> Result<Unit> {
    let path = find_unit_file(mana_dir, id)?;
    Unit::from_file(&path)
}

/// Load a unit from the archive by ID.
///
/// # Errors
/// - Unit ID not found in archive
/// - File cannot be parsed
pub fn get_archived_bean(mana_dir: &Path, id: &str) -> Result<Unit> {
    let path = find_archived_unit(mana_dir, id)?;
    Unit::from_file(&path)
}

/// Load the index, rebuilding from unit files if stale.
///
/// This is the main entry point for reading unit metadata.
/// The index is a YAML cache that's faster than reading every unit file.
pub fn load_index(mana_dir: &Path) -> Result<Index> {
    Index::load_or_rebuild(mana_dir)
}

// ---------------------------------------------------------------------------
// Submodules (added as they are implemented)
// ---------------------------------------------------------------------------

// pub mod query;         // Phase 1: 88.2.2
// pub mod mutations;     // Phase 1: 88.2.5, 88.2.6, 88.2.7
// pub mod orchestration; // Phase 1: 88.2.4
