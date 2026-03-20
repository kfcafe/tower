use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::unit::Status;
use crate::blocking::check_blocked;
use crate::index::{Index, IndexEntry};
use crate::util::natural_cmp;

/// Display dependency graph in ASCII, Mermaid, or DOT format
/// Default format is ASCII (terminal-friendly visualization)
/// Use --format mermaid for Mermaid graph TD syntax
/// Use --format dot for Graphviz DOT format
pub fn cmd_graph(mana_dir: &Path, format: &str) -> Result<()> {
    let index = Index::load_or_rebuild(mana_dir)?;

    match format {
        "mermaid" => output_mermaid_graph(&index)?,
        "dot" => output_dot_graph(&index)?,
        _ => output_ascii_graph(&index)?,
    }

    Ok(())
}

fn output_mermaid_graph(index: &Index) -> Result<()> {
    println!("graph TD");

    // Create a set of all nodes we'll reference
    let mut nodes = std::collections::HashSet::new();

    // Output edges (dependencies)
    for entry in &index.units {
        for dep_id in &entry.dependencies {
            println!(
                "    {}[{}] --> {}[{}]",
                format_node_id(&entry.id),
                escape_for_mermaid(&entry.title),
                format_node_id(dep_id),
                escape_for_mermaid(
                    index
                        .units
                        .iter()
                        .find(|e| &e.id == dep_id)
                        .map(|e| e.title.as_str())
                        .unwrap_or(dep_id)
                )
            );
            nodes.insert(entry.id.clone());
            nodes.insert(dep_id.clone());
        }
    }

    // Add isolated nodes (units with no dependencies and no dependents)
    for entry in &index.units {
        if entry.dependencies.is_empty()
            && !index
                .units
                .iter()
                .any(|e| e.dependencies.contains(&entry.id))
            && !nodes.contains(&entry.id)
        {
            println!(
                "    {}[{}]",
                format_node_id(&entry.id),
                escape_for_mermaid(&entry.title)
            );
        }
    }

    Ok(())
}

fn output_ascii_graph(index: &Index) -> Result<()> {
    if index.units.is_empty() {
        println!("Empty graph");
        println!("\n→ 0 units, 0 dependencies");
        return Ok(());
    }

    // Check for cycles and warn
    let cycles = crate::graph::find_all_cycles(index)?;
    if !cycles.is_empty() {
        eprintln!(
            "⚠ Warning: {} dependency cycle(s). Run 'mana dep cycles' for details.",
            cycles.len()
        );
    }

    // Build lookup maps
    let id_map: HashMap<&str, &IndexEntry> =
        index.units.iter().map(|e| (e.id.as_str(), e)).collect();

    // Build parent → children map
    let mut children_map: HashMap<&str, Vec<&IndexEntry>> = HashMap::new();
    for entry in &index.units {
        if let Some(ref parent_id) = entry.parent {
            children_map
                .entry(parent_id.as_str())
                .or_default()
                .push(entry);
        }
    }

    // Sort children by ID
    for children in children_map.values_mut() {
        children.sort_by(|a, b| natural_cmp(&a.id, &b.id));
    }

    // Build reverse dependency map: who depends on this unit (blockers)
    let mut blocked_by: HashMap<&str, Vec<&str>> = HashMap::new();
    for entry in &index.units {
        for dep_id in &entry.dependencies {
            blocked_by
                .entry(entry.id.as_str())
                .or_default()
                .push(dep_id.as_str());
        }
    }

    // Build forward dependency map: what does this unit block
    let mut blocks: HashMap<&str, Vec<&str>> = HashMap::new();
    for entry in &index.units {
        for dep_id in &entry.dependencies {
            blocks
                .entry(dep_id.as_str())
                .or_default()
                .push(entry.id.as_str());
        }
    }

    // Find root units (no parent)
    let mut roots: Vec<&IndexEntry> = index.units.iter().filter(|e| e.parent.is_none()).collect();
    roots.sort_by(|a, b| natural_cmp(&a.id, &b.id));

    // Track what we've printed to avoid duplicates
    let mut printed: HashSet<&str> = HashSet::new();

    // Render each root tree
    for (i, root) in roots.iter().enumerate() {
        if i > 0 {
            println!();
        }
        render_tree(
            root,
            &children_map,
            &blocked_by,
            &blocks,
            &id_map,
            index,
            &mut printed,
            "",
            true,
            true, // is_root
        );
    }

    // Print orphan units (have parent that doesn't exist)
    let orphans: Vec<&IndexEntry> = index
        .units
        .iter()
        .filter(|e| {
            e.parent.is_some()
                && !id_map.contains_key(e.parent.as_ref().unwrap().as_str())
                && !printed.contains(e.id.as_str())
        })
        .collect();

    if !orphans.is_empty() {
        println!("\n┌─ Orphans (missing parent)");
        for orphan in orphans {
            println!("│  {}", format_node(orphan, index));
            printed.insert(&orphan.id);
        }
        println!("└─");
    }

    // Summary
    let dep_count: usize = index.units.iter().map(|e| e.dependencies.len()).sum();
    println!(
        "\n→ {} units, {} dependencies",
        index.units.len(),
        dep_count
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_tree<'a>(
    entry: &'a IndexEntry,
    children_map: &HashMap<&str, Vec<&'a IndexEntry>>,
    blocked_by: &HashMap<&str, Vec<&str>>,
    blocks: &HashMap<&str, Vec<&str>>,
    id_map: &HashMap<&str, &IndexEntry>,
    index: &Index,
    printed: &mut HashSet<&'a str>,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) {
    printed.insert(&entry.id);

    // Build the node line
    let connector = if is_root {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    let node_str = format_node(entry, index);

    // Add dependency annotations
    let deps_annotation = if let Some(deps) = blocked_by.get(entry.id.as_str()) {
        if deps.is_empty() {
            String::new()
        } else {
            let dep_list: Vec<&str> = deps
                .iter()
                .filter(|d| {
                    // Only show non-parent deps (cross-cutting)
                    entry.parent.as_deref() != Some(**d)
                })
                .copied()
                .collect();
            if dep_list.is_empty() {
                String::new()
            } else {
                format!("  ◄── {}", dep_list.join(", "))
            }
        }
    } else {
        String::new()
    };

    println!("{}{}{}{}", prefix, connector, node_str, deps_annotation);

    // Get children
    let children = children_map.get(entry.id.as_str());

    // Show what this unit blocks (non-child dependencies)
    if let Some(blocked_list) = blocks.get(entry.id.as_str()) {
        let non_child_blocks: Vec<&str> = blocked_list
            .iter()
            .filter(|b| {
                // Only show if not a child of this unit
                if let Some(blocked_entry) = id_map.get(*b) {
                    blocked_entry.parent.as_deref() != Some(&entry.id)
                } else {
                    true
                }
            })
            .copied()
            .collect();

        if !non_child_blocks.is_empty() {
            let child_prefix = if is_root {
                if children.is_some() && !children.unwrap().is_empty() {
                    "│   "
                } else {
                    "    "
                }
            } else if is_last {
                &format!("{}    ", prefix)
            } else {
                &format!("{}│   ", prefix)
            };

            let blocks_str = non_child_blocks.join(", ");
            println!("{}──► blocks {}", child_prefix, blocks_str);
        }
    }

    // Render children
    if let Some(children) = children {
        let new_prefix = if is_root {
            String::new() // Children of root get empty prefix
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        for (i, child) in children.iter().enumerate() {
            let child_is_last = i == children.len() - 1;
            render_tree(
                child,
                children_map,
                blocked_by,
                blocks,
                id_map,
                index,
                printed,
                &new_prefix,
                child_is_last,
                false, // children are not roots
            );
        }
    }
}

fn format_node(entry: &IndexEntry, index: &Index) -> String {
    let status_icon = match entry.status {
        Status::Closed => "[✓]",
        Status::InProgress => "[●]",
        Status::Open => {
            if check_blocked(entry, index).is_some() {
                "[!]"
            } else {
                "[ ]"
            }
        }
    };

    let suffix = match check_blocked(entry, index) {
        Some(reason) => format!("  ({})", reason),
        None => {
            // Show scope warning as annotation, not blocking indicator
            crate::blocking::check_scope_warning(entry)
                .map(|w| format!("  (⚠ {})", w))
                .unwrap_or_default()
        }
    };

    format!("{} {}  {}{}", status_icon, entry.id, entry.title, suffix)
}

fn output_dot_graph(index: &Index) -> Result<()> {
    println!("digraph {{");
    println!("    rankdir=LR;");

    // Node declarations
    for entry in &index.units {
        println!(
            "    \"{}\" [label=\"{}\"];",
            entry.id,
            entry.title.replace("\"", "\\\"")
        );
    }

    // Edge declarations
    for entry in &index.units {
        for dep_id in &entry.dependencies {
            println!("    \"{}\" -> \"{}\";", entry.id, dep_id);
        }
    }

    println!("}}");

    Ok(())
}

/// Format node ID for Mermaid (replace dots with underscores)
fn format_node_id(id: &str) -> String {
    format!("N{}", id.replace('.', "_"))
}

/// Escape text for Mermaid graph labels
fn escape_for_mermaid(text: &str) -> String {
    text.replace("\"", "&quot;")
        .replace("[", "&lsqb;")
        .replace("]", "&rsqb;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_beans() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let bean1 = Unit::new("1", "Task one");
        let bean2 = Unit::new("2", "Task two");
        let mut bean3 = Unit::new("3", "Task three");
        bean3.dependencies = vec!["1".to_string(), "2".to_string()];

        bean1.to_file(mana_dir.join("1.yaml")).unwrap();
        bean2.to_file(mana_dir.join("2.yaml")).unwrap();
        bean3.to_file(mana_dir.join("3.yaml")).unwrap();

        (dir, mana_dir)
    }

    #[test]
    fn mermaid_output_valid() {
        let (_dir, mana_dir) = setup_test_beans();
        let result = cmd_graph(&mana_dir, "mermaid");
        assert!(result.is_ok());
    }

    #[test]
    fn dot_output_valid() {
        let (_dir, mana_dir) = setup_test_beans();
        let result = cmd_graph(&mana_dir, "dot");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_output_valid() {
        let (_dir, mana_dir) = setup_test_beans();
        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn default_format_is_ascii() {
        let (_dir, mana_dir) = setup_test_beans();
        let result = cmd_graph(&mana_dir, "");
        assert!(result.is_ok());
    }

    #[test]
    fn escaping_special_chars() {
        let id = "test.id";
        let formatted = format_node_id(id);
        assert_eq!(formatted, "Ntest_id");
    }

    #[test]
    fn mermaid_escape() {
        let text = "Task [with] brackets";
        let escaped = escape_for_mermaid(text);
        assert!(escaped.contains("&lsqb;"));
        assert!(escaped.contains("&rsqb;"));
    }

    // ASCII graph tests

    #[test]
    fn ascii_with_empty_graph() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_with_single_isolated_bean() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let unit = Unit::new("1", "Single task");
        unit.to_file(mana_dir.join("1.yaml")).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_with_multiple_isolated_beans() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let bean1 = Unit::new("1", "Task one");
        let bean2 = Unit::new("2", "Task two");
        let bean3 = Unit::new("3", "Task three");

        bean1.to_file(mana_dir.join("1.yaml")).unwrap();
        bean2.to_file(mana_dir.join("2.yaml")).unwrap();
        bean3.to_file(mana_dir.join("3.yaml")).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_with_diamond_dependencies() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let bean1 = Unit::new("1", "Root");
        let mut bean2 = Unit::new("2", "Left branch");
        let mut bean3 = Unit::new("3", "Right branch");
        let mut bean4 = Unit::new("4", "Merge");

        bean2.dependencies = vec!["1".to_string()];
        bean3.dependencies = vec!["1".to_string()];
        bean4.dependencies = vec!["2".to_string(), "3".to_string()];

        bean1.to_file(mana_dir.join("1.yaml")).unwrap();
        bean2.to_file(mana_dir.join("2.yaml")).unwrap();
        bean3.to_file(mana_dir.join("3.yaml")).unwrap();
        bean4.to_file(mana_dir.join("4.yaml")).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_with_cycle_warning() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let mut bean1 = Unit::new("1", "Task one");
        let mut bean2 = Unit::new("2", "Task two");
        let mut bean3 = Unit::new("3", "Task three");

        bean1.dependencies = vec!["2".to_string()];
        bean2.dependencies = vec!["3".to_string()];
        bean3.dependencies = vec!["1".to_string()];

        bean1.to_file(mana_dir.join("1.yaml")).unwrap();
        bean2.to_file(mana_dir.join("2.yaml")).unwrap();
        bean3.to_file(mana_dir.join("3.yaml")).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }

    #[test]
    fn ascii_long_title_not_truncated() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let long_title = "This is a very long title that should not be truncated";
        let unit = Unit::new("1", long_title);
        unit.to_file(mana_dir.join("1.yaml")).unwrap();

        // Verify the full title appears in format_node output
        let index = Index::load_or_rebuild(&mana_dir).unwrap();
        let node = format_node(&index.units[0], &index);
        assert!(
            node.contains(long_title),
            "Full title should appear in graph node"
        );
    }

    #[test]
    fn ascii_status_badges() {
        let dir = TempDir::new().unwrap();
        let mana_dir = dir.path().join(".mana");
        fs::create_dir(&mana_dir).unwrap();

        let bean1 = Unit::new("1", "Open task");
        let mut bean2 = Unit::new("2", "In progress task");
        let mut bean3 = Unit::new("3", "Closed task");

        bean2.status = Status::InProgress;
        bean3.status = Status::Closed;

        bean1.to_file(mana_dir.join("1.yaml")).unwrap();
        bean2.to_file(mana_dir.join("2.yaml")).unwrap();
        bean3.to_file(mana_dir.join("3.yaml")).unwrap();

        let result = cmd_graph(&mana_dir, "ascii");
        assert!(result.is_ok());
    }
}
