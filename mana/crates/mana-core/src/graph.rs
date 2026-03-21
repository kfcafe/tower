//! Dependency graph utilities.
//!
//! Functions in this module operate on the unit [`Index`] to answer
//! graph-related questions: cycle detection, topological ordering,
//! tree rendering, and subtree statistics.
//!
//! Most callers should use the wrappers in [`crate::api`] rather than
//! calling these functions directly.
//!
//! ## Example
//!
//! ```rust,no_run
//! use mana_core::api::{load_index, find_mana_dir};
//! use mana_core::graph::{detect_cycle, build_dependency_tree};
//! use std::path::Path;
//!
//! let mana_dir = find_mana_dir(Path::new(".")).unwrap();
//! let index = load_index(&mana_dir).unwrap();
//!
//! // Check whether adding a dep would create a cycle
//! if detect_cycle(&index, "5", "1").unwrap() {
//!     eprintln!("Adding 5 -> 1 would create a cycle");
//! }
//!
//! // Render a tree rooted at unit 1
//! let tree = build_dependency_tree(&index, "1").unwrap();
//! println!("{}", tree);
//! ```

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::index::Index;

/// Detect a cycle in the dependency graph.
///
/// Uses DFS from `to_id` to check if `from_id` is reachable.
/// If so, adding the edge from_id -> to_id would create a cycle.
pub fn detect_cycle(index: &Index, from_id: &str, to_id: &str) -> Result<bool> {
    // Quick check: self-dependency
    if from_id == to_id {
        return Ok(true);
    }

    // Build adjacency list from index
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &index.units {
        graph.insert(entry.id.clone(), entry.dependencies.clone());
    }

    // DFS from to_id: if we reach from_id, there's a cycle
    let mut visited = HashSet::new();
    let mut stack = vec![to_id.to_string()];

    while let Some(current) = stack.pop() {
        if current == from_id {
            return Ok(true);
        }

        if visited.contains(&current) {
            continue;
        }
        visited.insert(current.clone());

        if let Some(deps) = graph.get(&current) {
            for dep in deps {
                if !visited.contains(dep) {
                    stack.push(dep.clone());
                }
            }
        }
    }

    Ok(false)
}

/// Build a dependency tree rooted at `id`.
/// Returns a string representation with box-drawing characters.
pub fn build_dependency_tree(index: &Index, id: &str) -> Result<String> {
    // Find the root unit
    let root_entry = index
        .units
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| anyhow!("Unit {} not found", id))?;

    let mut output = String::new();
    output.push_str(&format!("{} {}\n", root_entry.id, root_entry.title));

    // Build adjacency list
    let graph: HashMap<String, Vec<String>> = index
        .units
        .iter()
        .map(|e| (e.id.clone(), e.dependencies.clone()))
        .collect();

    // Build reverse graph (dependents)
    let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
    for (id, deps) in &graph {
        for dep in deps {
            reverse_graph
                .entry(dep.clone())
                .or_default()
                .push(id.clone());
        }
    }

    // Create index map
    let id_map: HashMap<String, &crate::index::IndexEntry> =
        index.units.iter().map(|e| (e.id.clone(), e)).collect();

    // DFS to build tree
    let mut visited = HashSet::new();
    build_tree_recursive(&mut output, id, &reverse_graph, &id_map, &mut visited, "");

    Ok(output)
}

fn build_tree_recursive(
    output: &mut String,
    current_id: &str,
    reverse_graph: &HashMap<String, Vec<String>>,
    id_map: &HashMap<String, &crate::index::IndexEntry>,
    visited: &mut HashSet<String>,
    prefix: &str,
) {
    if visited.contains(current_id) {
        return;
    }
    visited.insert(current_id.to_string());

    if let Some(dependents) = reverse_graph.get(current_id) {
        for (i, dependent_id) in dependents.iter().enumerate() {
            let is_last_dependent = i == dependents.len() - 1;

            let connector = if is_last_dependent {
                "└── "
            } else {
                "├── "
            };
            output.push_str(prefix);
            output.push_str(connector);

            if let Some(entry) = id_map.get(dependent_id) {
                output.push_str(&format!("{} {}\n", entry.id, entry.title));
            } else {
                output.push_str(&format!("{}\n", dependent_id));
            }

            let new_prefix = if is_last_dependent {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            build_tree_recursive(
                output,
                dependent_id,
                reverse_graph,
                id_map,
                visited,
                &new_prefix,
            );
        }
    }
}

/// Build a project-wide dependency graph as a text tree.
/// Shows all dependencies rooted at units with no parents.
pub fn build_full_graph(index: &Index) -> Result<String> {
    // Find root units (those with no parent)
    let root_beans: Vec<_> = index.units.iter().filter(|e| e.parent.is_none()).collect();

    if root_beans.is_empty() {
        return Ok("No units found.".to_string());
    }

    let mut output = String::new();

    // Build reverse graph (dependents)
    let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &index.units {
        for dep in &entry.dependencies {
            reverse_graph
                .entry(dep.clone())
                .or_default()
                .push(entry.id.clone());
        }
    }

    // Create index map
    let id_map: HashMap<String, &crate::index::IndexEntry> =
        index.units.iter().map(|e| (e.id.clone(), e)).collect();

    let mut visited = HashSet::new();
    for root in root_beans {
        output.push_str(&format!("{} {}\n", root.id, root.title));
        build_tree_recursive(
            &mut output,
            &root.id,
            &reverse_graph,
            &id_map,
            &mut visited,
            "",
        );
    }

    Ok(output)
}

/// Count total verify attempts across all descendants of a unit.
///
/// Includes the unit itself and archived descendants.
/// Used by the circuit breaker to detect runaway retry loops across a subtree.
#[must_use = "returns the total attempt count"]
pub fn count_subtree_attempts(mana_dir: &Path, root_id: &str) -> Result<u32> {
    let index = Index::build(mana_dir)?;
    let archived = Index::collect_archived(mana_dir).unwrap_or_default();

    // Combine active and archived units
    let mut all_beans = index.units;
    all_beans.extend(archived);

    let mut total = 0u32;
    let mut stack = vec![root_id.to_string()];
    let mut visited = HashSet::new();

    while let Some(id) = stack.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(entry) = all_beans.iter().find(|b| b.id == id) {
            total += entry.attempts;
            // Find children
            for child in all_beans
                .iter()
                .filter(|b| b.parent.as_deref() == Some(id.as_str()))
            {
                if !visited.contains(&child.id) {
                    stack.push(child.id.clone());
                }
            }
        }
    }
    Ok(total)
}

/// Find all cycles in the dependency graph.
/// Returns a list of cycle paths.
pub fn find_all_cycles(index: &Index) -> Result<Vec<Vec<String>>> {
    let mut cycles = Vec::new();

    // Build adjacency list
    let graph: HashMap<String, Vec<String>> = index
        .units
        .iter()
        .map(|e| (e.id.clone(), e.dependencies.clone()))
        .collect();

    let mut visited = HashSet::new();

    // For each node, check if there's a cycle starting from it
    for start_id in graph.keys() {
        if !visited.contains(start_id) {
            let mut path = Vec::new();
            find_cycle_dfs(&graph, start_id, &mut visited, &mut path, &mut cycles);
        }
    }

    Ok(cycles)
}

fn find_cycle_dfs(
    graph: &HashMap<String, Vec<String>>,
    current: &str,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    // Check if current is already on the DFS path (back edge = cycle)
    if let Some(pos) = path.iter().position(|id| id == current) {
        let cycle = path[pos..].to_vec();
        if !cycles.contains(&cycle) {
            cycles.push(cycle);
        }
        return;
    }

    // Skip if already fully explored
    if visited.contains(current) {
        return;
    }

    path.push(current.to_string());

    if let Some(deps) = graph.get(current) {
        for dep in deps {
            find_cycle_dfs(graph, dep, visited, path, cycles);
        }
    }

    path.pop();
    visited.insert(current.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_beans(specs: Vec<(&str, Vec<&str>)>) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        for (id, deps) in specs {
            let mut unit = Unit::new(id, format!("Task {}", id));
            unit.dependencies = deps.iter().map(|s| s.to_string()).collect();
            unit.to_file(mana_dir.join(format!("{}.yaml", id))).unwrap();
        }

        (dir, mana_dir)
    }

    #[test]
    fn detect_self_cycle() {
        let (_dir, mana_dir) = setup_test_beans(vec![("1", vec![])]);
        let index = Index::build(&mana_dir).unwrap();
        assert!(detect_cycle(&index, "1", "1").unwrap());
    }

    #[test]
    fn detect_two_node_cycle() {
        let (_dir, mana_dir) = setup_test_beans(vec![("1", vec!["2"]), ("2", vec![])]);
        let index = Index::build(&mana_dir).unwrap();
        assert!(detect_cycle(&index, "2", "1").unwrap());
        assert!(!detect_cycle(&index, "1", "2").unwrap());
    }

    #[test]
    fn detect_three_node_cycle() {
        let (_dir, mana_dir) =
            setup_test_beans(vec![("1", vec!["2"]), ("2", vec!["3"]), ("3", vec![])]);
        let index = Index::build(&mana_dir).unwrap();
        // If we add 3 -> 1, it creates a cycle
        assert!(detect_cycle(&index, "3", "1").unwrap());
        assert!(!detect_cycle(&index, "1", "3").unwrap());
    }

    #[test]
    fn no_cycle_linear_chain() {
        let (_dir, mana_dir) =
            setup_test_beans(vec![("1", vec!["2"]), ("2", vec!["3"]), ("3", vec![])]);
        let index = Index::build(&mana_dir).unwrap();
        assert!(!detect_cycle(&index, "1", "2").unwrap());
        assert!(!detect_cycle(&index, "2", "3").unwrap());
    }

    // =====================================================================
    // Subtree Attempts Tests
    // =====================================================================

    /// Helper: create units with parent + attempts for subtree tests.
    /// Each spec: (id, parent, attempts)
    fn setup_subtree_beans(specs: Vec<(&str, Option<&str>, u32)>) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        for (id, parent, attempts) in specs {
            let mut unit = Unit::new(id, format!("Task {}", id));
            unit.parent = parent.map(|s| s.to_string());
            unit.attempts = attempts;
            let slug = crate::util::title_to_slug(&unit.title);
            unit.to_file(mana_dir.join(format!("{}-{}.md", id, slug)))
                .unwrap();
        }

        (dir, mana_dir)
    }

    #[test]
    fn subtree_attempts_single_bean_no_children() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![("1", None, 5)]);
        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        assert_eq!(total, 5);
    }

    #[test]
    fn subtree_attempts_includes_root() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![
            ("1", None, 3),
            ("1.1", Some("1"), 2),
            ("1.2", Some("1"), 1),
        ]);
        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        // Root(3) + 1.1(2) + 1.2(1) = 6
        assert_eq!(total, 6);
    }

    #[test]
    fn subtree_attempts_sums_all_descendants() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![
            ("1", None, 0),
            ("1.1", Some("1"), 2),
            ("1.2", Some("1"), 3),
            ("1.1.1", Some("1.1"), 1),
            ("1.1.2", Some("1.1"), 4),
        ]);
        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        // 0 + 2 + 3 + 1 + 4 = 10
        assert_eq!(total, 10);
    }

    #[test]
    fn subtree_attempts_subtree_only() {
        // Only counts descendants of the given root, not siblings
        let (_dir, mana_dir) = setup_subtree_beans(vec![
            ("1", None, 1),
            ("1.1", Some("1"), 5),
            ("2", None, 10),
            ("2.1", Some("2"), 20),
        ]);
        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        // Only 1(1) + 1.1(5) = 6, not including "2" tree
        assert_eq!(total, 6);
    }

    #[test]
    fn subtree_attempts_unknown_root_returns_zero() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![("1", None, 5)]);
        let total = count_subtree_attempts(&mana_dir, "999").unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn subtree_attempts_zero_attempts_everywhere() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![
            ("1", None, 0),
            ("1.1", Some("1"), 0),
            ("1.2", Some("1"), 0),
        ]);
        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        assert_eq!(total, 0);
    }

    #[test]
    fn subtree_attempts_includes_archived_beans() {
        let (_dir, mana_dir) = setup_subtree_beans(vec![("1", None, 1), ("1.2", Some("1"), 2)]);

        // Create an archived child with attempts
        let archive_dir = mana_dir.join("archive").join("2026").join("02");
        fs::create_dir_all(&archive_dir).unwrap();
        let mut archived_bean = Unit::new("1.1", "Archived Child");
        archived_bean.parent = Some("1".to_string());
        archived_bean.attempts = 3;
        archived_bean.status = crate::unit::Status::Closed;
        archived_bean.is_archived = true;
        archived_bean
            .to_file(archive_dir.join("1.1-archived-child.md"))
            .unwrap();

        let total = count_subtree_attempts(&mana_dir, "1").unwrap();
        // Root(1) + active 1.2(2) + archived 1.1(3) = 6
        assert_eq!(total, 6);
    }
}
