use std::path::Path;

use crate::unit::{Unit, OnCloseAction};
use crate::config::Config;
use crate::hooks::{
    current_git_branch, execute_config_hook, execute_hook, is_trusted, HookEvent, HookVars,
};

/// Run pre-close hook. Returns true if hook passes (or doesn't exist).
/// Returns false if hook rejects the close.
/// Errors from the hook itself are logged but treated as "pass" (allow close).
pub(crate) fn run_pre_close(
    unit: &Unit,
    project_root: &Path,
    reason: Option<&str>,
) -> bool {
    let result = execute_hook(
        HookEvent::PreClose,
        unit,
        project_root,
        reason.map(|s| s.to_string()),
    );

    match result {
        Ok(hook_passed) => hook_passed,
        Err(e) => {
            eprintln!("Unit {} pre-close hook error: {}", unit.id, e);
            true // Silently pass (allow close to proceed)
        }
    }
}

/// Run post-close hook + on_close actions + config hooks.
///
/// Failures are logged but never revert the close.
pub(crate) fn run_post_close(
    unit: &Unit,
    project_root: &Path,
    reason: Option<&str>,
    config: Option<&Config>,
) {
    // Fire post-close hook
    match execute_hook(
        HookEvent::PostClose,
        unit,
        project_root,
        reason.map(|s| s.to_string()),
    ) {
        Ok(false) => {
            eprintln!(
                "Warning: post-close hook returned non-zero for unit {}",
                unit.id
            );
        }
        Err(e) => {
            eprintln!(
                "Warning: post-close hook error for unit {}: {}",
                unit.id, e
            );
        }
        Ok(true) => {}
    }

    // Process on_close actions
    for action in &unit.on_close {
        match action {
            OnCloseAction::Run { command } => {
                if !is_trusted(project_root) {
                    eprintln!(
                        "on_close: skipping `{}` (not trusted — run `mana trust` to enable)",
                        command
                    );
                    continue;
                }
                eprintln!("on_close: running `{}`", command);
                let status = std::process::Command::new("sh")
                    .args(["-c", command.as_str()])
                    .current_dir(project_root)
                    .status();
                match status {
                    Ok(s) if !s.success() => {
                        eprintln!("on_close run command failed: {}", command)
                    }
                    Err(e) => eprintln!("on_close run command error: {}", e),
                    _ => {}
                }
            }
            OnCloseAction::Notify { message } => {
                println!("[unit {}] {}", unit.id, message);
            }
        }
    }

    // Fire on_close config hook
    if let Some(config) = config {
        if let Some(ref on_close_template) = config.on_close {
            let vars = HookVars {
                id: Some(unit.id.clone()),
                title: Some(unit.title.clone()),
                status: Some("closed".into()),
                branch: current_git_branch(),
                ..Default::default()
            };
            execute_config_hook("on_close", on_close_template, &vars, project_root);
        }
    }
}

/// Fire the on_fail config hook (async, non-blocking).
pub(crate) fn run_on_fail_hook(
    unit: &Unit,
    project_root: &Path,
    config: Option<&Config>,
    output: &str,
) {
    if let Some(config) = config {
        if let Some(ref on_fail_template) = config.on_fail {
            let vars = HookVars {
                id: Some(unit.id.clone()),
                title: Some(unit.title.clone()),
                status: Some(format!("{}", unit.status)),
                attempt: Some(unit.attempts),
                output: Some(output.to_string()),
                branch: current_git_branch(),
                ..Default::default()
            };
            execute_config_hook("on_fail", on_fail_template, &vars, project_root);
        }
    }
}
