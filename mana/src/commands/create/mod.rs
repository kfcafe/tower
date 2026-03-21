use std::fs;
use std::path::Path;
use std::process::Command as ShellCommand;

use anyhow::{anyhow, Context, Result};

use crate::commands::claim::cmd_claim;
use crate::config::Config;
use crate::hooks::{execute_hook, HookEvent};
use crate::index::Index;
use crate::project::suggest_verify_command;
use crate::unit::{validate_priority, OnFailAction, Unit};
use crate::util::{find_similar_titles, title_to_slug, DEFAULT_SIMILARITY_THRESHOLD};

/// Create arguments structure for organizing all the parameters passed to create.
pub struct CreateArgs {
    pub title: String,
    pub description: Option<String>,
    pub acceptance: Option<String>,
    pub notes: Option<String>,
    pub design: Option<String>,
    pub verify: Option<String>,
    pub priority: Option<u8>,
    pub labels: Option<String>,
    pub assignee: Option<String>,
    pub deps: Option<String>,
    pub parent: Option<String>,
    pub produces: Option<String>,
    pub requires: Option<String>,
    /// Comma-separated file paths relevant to this unit.
    pub paths: Option<String>,
    /// Action on verify failure
    pub on_fail: Option<OnFailAction>,
    /// Skip fail-first check (allow verify to already pass)
    pub pass_ok: bool,
    /// Claim the unit immediately after creation
    pub claim: bool,
    /// Who is claiming (used with claim)
    pub by: Option<String>,
    /// Timeout in seconds for the verify command (kills process on expiry).
    pub verify_timeout: Option<u64>,
    /// Mark as a product feature (human-only close, no verify gate required).
    pub feature: bool,
    /// Unresolved decisions that block autonomous execution.
    pub decisions: Vec<String>,
    /// Skip duplicate title check
    pub force: bool,
}

/// Assign a child ID for a parent unit.
/// Scans .mana/ for {parent_id}.{N}-*.md, finds highest N, returns "{parent_id}.{N+1}".
pub fn assign_child_id(mana_dir: &Path, parent_id: &str) -> Result<String> {
    let mut max_child: u32 = 0;

    let dir_entries = fs::read_dir(mana_dir)
        .with_context(|| format!("Failed to read directory: {}", mana_dir.display()))?;

    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        // Look for files matching "{parent_id}.{N}-*.md" (new format)
        if let Some(name_without_ext) = filename.strip_suffix(".md") {
            if let Some(name_without_parent) = name_without_ext.strip_prefix(parent_id) {
                if let Some(after_dot) = name_without_parent.strip_prefix('.') {
                    // Extract the number part before the hyphen
                    let num_part = after_dot.split('-').next().unwrap_or_default();
                    if let Ok(child_num) = num_part.parse::<u32>() {
                        if child_num > max_child {
                            max_child = child_num;
                        }
                    }
                }
            }
        }

        // Also support legacy format for backward compatibility: {parent_id}.{N}.yaml
        if let Some(name_without_ext) = filename.strip_suffix(".yaml") {
            if let Some(name_without_parent) = name_without_ext.strip_prefix(parent_id) {
                if let Some(after_dot) = name_without_parent.strip_prefix('.') {
                    if let Ok(child_num) = after_dot.parse::<u32>() {
                        if child_num > max_child {
                            max_child = child_num;
                        }
                    }
                }
            }
        }
    }

    Ok(format!("{}.{}", parent_id, max_child + 1))
}

/// Parse an `--on-fail` CLI string into an `OnFailAction`.
///
/// Accepted formats:
/// - `retry` → Retry { max: None, delay_secs: None }
/// - `retry:5` → Retry { max: Some(5), delay_secs: None }
/// - `escalate` → Escalate { priority: None, message: None }
/// - `escalate:P0` or `escalate:0` → Escalate { priority: Some(0), message: None }
pub fn parse_on_fail(s: &str) -> Result<OnFailAction> {
    let (action, arg) = match s.split_once(':') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };

    match action {
        "retry" => {
            let max = arg.map(|a| a.parse::<u32>()).transpose().map_err(|_| {
                anyhow!(
                    "Invalid retry max: '{}'. Expected a number (e.g. retry:5)",
                    arg.unwrap_or("")
                )
            })?;
            Ok(OnFailAction::Retry {
                max,
                delay_secs: None,
            })
        }
        "escalate" => {
            let priority = match arg {
                Some(a) => {
                    let stripped = a
                        .strip_prefix('P')
                        .or_else(|| a.strip_prefix('p'))
                        .unwrap_or(a);
                    let p = stripped.parse::<u8>().map_err(|_| {
                        anyhow!("Invalid escalate priority: '{}'. Expected P0-P4 or 0-4", a)
                    })?;
                    validate_priority(p)?;
                    Some(p)
                }
                None => None,
            };
            Ok(OnFailAction::Escalate {
                priority,
                message: None,
            })
        }
        _ => Err(anyhow!(
            "Unknown on-fail action: '{}'. Expected 'retry' or 'escalate'",
            action
        )),
    }
}

/// Create a new unit.
///
/// If `args.parent` is given, assign a child ID ({parent_id}.{next_child}).
/// Otherwise, use the next sequential ID from config and increment it.
/// Returns the created unit ID on success.
pub fn cmd_create(mana_dir: &Path, args: CreateArgs) -> Result<String> {
    // Validate priority if provided
    if let Some(priority) = args.priority {
        validate_priority(priority)?;
    }

    // When --claim is used without --parent, require validation criteria
    // (same as bn quick). Parent/goal units (no --claim) remain exempt.
    if args.claim && args.parent.is_none() && args.acceptance.is_none() && args.verify.is_none() {
        anyhow::bail!(
            "Unit must have validation criteria: provide --acceptance or --verify (or both)\n\
             Hint: parent/goal units (without --claim) don't require this."
        );
    }

    // Fail-first check (default): verify command must FAIL before unit can be created
    // This prevents "cheating tests" like `assert True` that always pass
    // Use --pass-ok / -p to skip this check
    if !args.pass_ok {
        if let Some(verify_cmd) = args.verify.as_ref() {
            let project_root = mana_dir
                .parent()
                .ok_or_else(|| anyhow!("Cannot determine project root"))?;

            eprintln!("Running verify (must fail): {}", verify_cmd);

            let status = ShellCommand::new("sh")
                .args(["-c", verify_cmd])
                .current_dir(project_root)
                .status()
                .with_context(|| format!("Failed to execute verify command: {}", verify_cmd))?;

            if status.success() {
                anyhow::bail!(
                    "Cannot create unit: verify command already passes!\n\n\
                     The test must FAIL on current code to prove it tests something real.\n\
                     Either:\n\
                     - The test doesn't actually test the new behavior\n\
                     - The feature is already implemented\n\
                     - The test is a no-op (assert True)\n\n\
                     Use --pass-ok / -p to skip this check."
                );
            }

            eprintln!("✓ Verify failed as expected - test is real");
        }
    }

    // Duplicate title check (skip with --force)
    if !args.force {
        if let Ok(index) = Index::load_or_rebuild(mana_dir) {
            let similar = find_similar_titles(&index, &args.title, DEFAULT_SIMILARITY_THRESHOLD);
            if !similar.is_empty() {
                let mut msg = String::from("Similar unit(s) already exist:\n");
                for s in &similar {
                    msg.push_str(&format!(
                        "  [{}] {} (similarity: {:.0}%)\n",
                        s.id,
                        s.title,
                        s.score * 100.0
                    ));
                }
                msg.push_str("\nUse --force to create anyway.");
                anyhow::bail!(msg);
            }
        }
    }

    // Load config
    let mut config = Config::load(mana_dir)?;

    // Determine the unit ID
    let bean_id = if let Some(parent_id) = &args.parent {
        assign_child_id(mana_dir, parent_id)?
    } else {
        let id = config.increment_id();
        config.save(mana_dir)?;
        id.to_string()
    };

    // Generate slug from title
    let slug = title_to_slug(&args.title);

    // Track if verify was provided for suggestion later
    let has_verify = args.verify.is_some();

    // Create the unit
    let mut unit = Unit::new(&bean_id, &args.title);
    unit.slug = Some(slug.clone());

    if let Some(desc) = args.description {
        unit.description = Some(desc);
    }
    if let Some(acceptance) = args.acceptance {
        unit.acceptance = Some(acceptance);
    }
    if let Some(notes) = args.notes {
        unit.notes = Some(notes);
    }
    if let Some(design) = args.design {
        unit.design = Some(design);
    }
    let has_fail_first = !args.pass_ok && args.verify.is_some();
    if let Some(verify) = args.verify {
        unit.verify = Some(verify);
    }
    if has_fail_first {
        unit.fail_first = true;
    }
    if args.feature {
        unit.feature = true;
    }
    if let Some(priority) = args.priority {
        unit.priority = priority;
    }
    if let Some(assignee) = args.assignee {
        unit.assignee = Some(assignee);
    }
    if let Some(parent) = args.parent {
        unit.parent = Some(parent);
    }

    // Parse labels
    if let Some(labels_str) = args.labels {
        unit.labels = labels_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }

    // Parse dependencies
    if let Some(deps_str) = args.deps {
        unit.dependencies = deps_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Parse produces
    if let Some(produces_str) = args.produces {
        unit.produces = produces_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }

    // Parse requires
    if let Some(requires_str) = args.requires {
        unit.requires = requires_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }

    // Parse paths
    if let Some(paths_str) = args.paths {
        unit.paths = paths_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    // Set on_fail action
    if let Some(on_fail) = args.on_fail {
        unit.on_fail = Some(on_fail);
    }

    // Set verify_timeout if provided
    if let Some(timeout) = args.verify_timeout {
        unit.verify_timeout = Some(timeout);
    }

    // Set decisions if provided
    if !args.decisions.is_empty() {
        unit.decisions = args.decisions;
    }

    // Get the project directory (parent of mana_dir which is .mana)
    let project_dir = mana_dir
        .parent()
        .ok_or_else(|| anyhow!("Failed to determine project directory"))?;

    // Call pre-create hook (blocking - abort if it fails)
    let pre_passed = execute_hook(HookEvent::PreCreate, &unit, project_dir, None)
        .context("Pre-create hook execution failed")?;

    if !pre_passed {
        return Err(anyhow!("Pre-create hook rejected unit creation"));
    }

    // Write the unit file with new naming convention: {id}-{slug}.md
    let bean_path = mana_dir.join(format!("{}-{}.md", bean_id, slug));
    unit.to_file(&bean_path)?;

    // Update the index by rebuilding from disk (includes the unit we just wrote)
    let index = Index::build(mana_dir)?;
    index.save(mana_dir)?;

    eprintln!("Created unit {}: {}", bean_id, args.title);

    // Suggest verify command if none was provided
    if !has_verify {
        if let Some(suggested) = suggest_verify_command(project_dir) {
            eprintln!(
                "Tip: Consider adding a verify command: --verify \"{}\"",
                suggested
            );
        }
    }

    // Call post-create hook (non-blocking - log warning if it fails)
    if let Err(e) = execute_hook(HookEvent::PostCreate, &unit, project_dir, None) {
        eprintln!("Warning: post-create hook failed: {}", e);
    }

    // If --claim was passed, claim the unit immediately (skip verify-on-claim check)
    if args.claim {
        cmd_claim(mana_dir, &bean_id, args.by, true)?;
    }

    Ok(bean_id)
}

/// Create a new unit that automatically depends on @latest (the most recently updated unit).
///
/// This enables sequential chaining:
/// ```bash
/// mana create "Step 1" -p
/// mana create next "Step 2" --verify "cargo test step2"
/// mana create next "Step 3" --verify "cargo test step3"
/// ```
///
/// If `args.deps` already contains dependencies, @latest is prepended.
/// Returns the created unit ID on success.
pub fn cmd_create_next(mana_dir: &Path, args: CreateArgs) -> Result<String> {
    // Resolve @latest — find the most recently updated unit
    let index = Index::load(mana_dir).or_else(|_| Index::build(mana_dir))?;
    let latest_id = index
        .units
        .iter()
        .max_by_key(|e| e.updated_at)
        .map(|e| e.id.clone())
        .ok_or_else(|| {
            anyhow!(
                "No previous unit found. 'mana create next' requires at least one existing unit.\n\
                 Use 'mana create' for the first unit in a chain."
            )
        })?;

    // Merge @latest dep with any explicit deps
    let merged_deps = match args.deps {
        Some(ref d) => Some(format!("{},{}", latest_id, d)),
        None => Some(latest_id.clone()),
    };

    eprintln!("⛓ Chained after unit {} (@latest)", latest_id);

    let new_args = CreateArgs {
        deps: merged_deps,
        ..args
    };

    cmd_create(mana_dir, new_args)
}

#[cfg(test)]
mod tests;
