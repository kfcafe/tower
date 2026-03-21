use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::index::Index;
use crate::stream::{self, StreamEvent};
use crate::unit::Status;
use crate::util::natural_cmp;

use super::plan::SizedBean;
use super::ready_queue::run_single_direct;
use super::{AgentResult, BeanAction, SpawnMode};

/// A wave of units that can be dispatched in parallel.
pub struct Wave {
    pub units: Vec<SizedBean>,
}

/// Compute waves of units grouped by dependency order.
/// Wave 0: no deps. Wave 1: deps all in wave 0. Etc.
pub(super) fn compute_waves(units: &[SizedBean], index: &Index) -> Vec<Wave> {
    let mut waves = Vec::new();
    let bean_ids: HashSet<String> = units.iter().map(|b| b.id.clone()).collect();

    // Already-closed units count as completed
    let mut completed: HashSet<String> = index
        .units
        .iter()
        .filter(|e| e.status == Status::Closed)
        .map(|e| e.id.clone())
        .collect();

    let mut remaining: Vec<SizedBean> = units.to_vec();

    while !remaining.is_empty() {
        let (ready, blocked): (Vec<SizedBean>, Vec<SizedBean>) =
            remaining.into_iter().partition(|b| {
                // All explicit deps must be completed or not in our dispatch set
                let explicit_ok = b
                    .dependencies
                    .iter()
                    .all(|d| completed.contains(d) || !bean_ids.contains(d));

                // All requires must be satisfied (producer completed or not in set)
                let requires_ok = b.requires.iter().all(|req| {
                    // Find the sibling producer for this artifact
                    if let Some(producer) = units.iter().find(|other| {
                        other.id != b.id && other.parent == b.parent && other.produces.contains(req)
                    }) {
                        completed.contains(&producer.id)
                    } else {
                        true // No producer in set, assume satisfied
                    }
                });

                explicit_ok && requires_ok
            });

        if ready.is_empty() {
            // Remaining units have unresolvable deps (cycle or missing)
            // Add them all as a final wave to avoid losing them
            eprintln!(
                "Warning: {} unit(s) have unresolvable dependencies, adding to final wave",
                blocked.len()
            );
            waves.push(Wave { units: blocked });
            break;
        }

        for b in &ready {
            completed.insert(b.id.clone());
        }

        waves.push(Wave { units: ready });
        remaining = blocked;
    }

    // Sort units within each wave by priority then ID
    for wave in &mut waves {
        wave.units.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| natural_cmp(&a.id, &b.id))
        });
    }

    waves
}

/// Compute downstream weight for each bean.
/// Weight = 1 + count of all transitively dependent beans.
/// Beans on the critical path will have the highest weights.
pub(super) fn compute_downstream_weights(beans: &[SizedBean]) -> HashMap<String, u32> {
    let bean_ids: HashSet<String> = beans.iter().map(|b| b.id.clone()).collect();

    // Build reverse dependency graph: for each bean, which beans directly depend on it.
    let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();

    for b in beans {
        reverse_deps.entry(b.id.clone()).or_default();

        // Explicit dependencies: b depends on dep → dep's reverse_deps includes b
        for dep in &b.dependencies {
            if bean_ids.contains(dep) {
                reverse_deps
                    .entry(dep.clone())
                    .or_default()
                    .push(b.id.clone());
            }
        }

        // Requires/produces: if b requires artifact X and producer makes X
        // (same parent), then b depends on producer.
        for req in &b.requires {
            if let Some(producer) = beans.iter().find(|other| {
                other.id != b.id && other.parent == b.parent && other.produces.contains(req)
            }) {
                if bean_ids.contains(&producer.id) {
                    reverse_deps
                        .entry(producer.id.clone())
                        .or_default()
                        .push(b.id.clone());
                }
            }
        }
    }

    // For each bean, count all transitively reachable descendants via BFS.
    let mut weights: HashMap<String, u32> = HashMap::new();

    for b in beans {
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: Vec<String> = Vec::new();

        // Seed with direct dependents
        for dep in reverse_deps.get(&b.id).unwrap_or(&Vec::new()) {
            if visited.insert(dep.clone()) {
                queue.push(dep.clone());
            }
        }

        // BFS to find all transitive dependents
        while let Some(current) = queue.pop() {
            for next in reverse_deps.get(&current).unwrap_or(&Vec::new()) {
                if visited.insert(next.clone()) {
                    queue.push(next.clone());
                }
            }
        }

        weights.insert(b.id.clone(), 1 + visited.len() as u32);
    }

    weights
}

// ---------------------------------------------------------------------------
// Wave execution
// ---------------------------------------------------------------------------

/// Spawn agents for a wave of units, respecting max parallelism.
pub(super) fn run_wave(
    mana_dir: &Path,
    units: &[SizedBean],
    spawn_mode: &SpawnMode,
    cfg: &super::RunConfig,
    wave_number: usize,
) -> Result<Vec<AgentResult>> {
    match spawn_mode {
        SpawnMode::Template {
            run_template,
            plan_template,
        } => run_wave_template(
            units,
            run_template,
            plan_template.as_deref(),
            cfg.max_jobs,
            cfg.timeout_minutes,
        ),
        SpawnMode::Direct => run_wave_direct(
            mana_dir,
            units,
            cfg.max_jobs,
            cfg.timeout_minutes,
            cfg.idle_timeout_minutes,
            cfg.json_stream,
            wave_number,
            cfg.file_locking,
        ),
    }
}

/// Template mode: spawn agents via `sh -c <template>` (backward compat).
fn run_wave_template(
    units: &[SizedBean],
    run_template: &str,
    _plan_template: Option<&str>,
    max_jobs: usize,
    _timeout_minutes: u32,
) -> Result<Vec<AgentResult>> {
    let mut results = Vec::new();
    let mut children: Vec<(SizedBean, std::process::Child, Instant)> = Vec::new();

    let mut pending: Vec<&SizedBean> = units.iter().collect();

    while !pending.is_empty() || !children.is_empty() {
        // Check for shutdown signal
        if super::shutdown_requested() {
            for (sb, mut child, started) in children {
                let _ = child.kill();
                let _ = child.wait();
                results.push(AgentResult {
                    id: sb.id.clone(),
                    title: sb.title.clone(),
                    action: sb.action,
                    success: false,
                    duration: started.elapsed(),
                    total_tokens: None,
                    total_cost: None,
                    error: Some("Interrupted by shutdown signal".to_string()),
                    tool_count: 0,
                    turns: 0,
                    failure_summary: None,
                });
            }
            return Ok(results);
        }

        // Spawn up to max_jobs
        while children.len() < max_jobs && !pending.is_empty() {
            let sb = pending.remove(0);
            let template = match sb.action {
                BeanAction::Implement => run_template,
            };

            let cmd = template.replace("{id}", &sb.id);
            match Command::new("sh").args(["-c", &cmd]).spawn() {
                Ok(child) => {
                    children.push((sb.clone(), child, Instant::now()));
                }
                Err(e) => {
                    eprintln!("  Failed to spawn agent for {}: {}", sb.id, e);
                    results.push(AgentResult {
                        id: sb.id.clone(),
                        title: sb.title.clone(),
                        action: sb.action,
                        success: false,
                        duration: Duration::ZERO,
                        total_tokens: None,
                        total_cost: None,
                        error: Some(format!("Failed to spawn: {}", e)),
                        tool_count: 0,
                        turns: 0,
                        failure_summary: None,
                    });
                }
            }
        }

        if children.is_empty() {
            break;
        }

        // Poll for completions
        let mut still_running = Vec::new();
        for (sb, mut child, started) in children {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let err = if status.success() {
                        None
                    } else {
                        Some(format!("Exit code {}", status.code().unwrap_or(-1)))
                    };
                    results.push(AgentResult {
                        id: sb.id.clone(),
                        title: sb.title.clone(),
                        action: sb.action,
                        success: status.success(),
                        duration: started.elapsed(),
                        total_tokens: None,
                        total_cost: None,
                        error: err,
                        tool_count: 0,
                        turns: 0,
                        failure_summary: None,
                    });
                }
                Ok(None) => {
                    still_running.push((sb, child, started));
                }
                Err(e) => {
                    eprintln!("  Error checking agent for {}: {}", sb.id, e);
                    results.push(AgentResult {
                        id: sb.id.clone(),
                        title: sb.title.clone(),
                        action: sb.action,
                        success: false,
                        duration: started.elapsed(),
                        total_tokens: None,
                        total_cost: None,
                        error: Some(format!("Error checking process: {}", e)),
                        tool_count: 0,
                        turns: 0,
                        failure_summary: None,
                    });
                }
            }
        }
        children = still_running;

        if !children.is_empty() {
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    Ok(results)
}

/// Direct mode: spawn pi directly with JSON output and monitoring.
#[allow(clippy::too_many_arguments)]
fn run_wave_direct(
    mana_dir: &Path,
    units: &[SizedBean],
    max_jobs: usize,
    timeout_minutes: u32,
    idle_timeout_minutes: u32,
    json_stream: bool,
    wave_number: usize,
    file_locking: bool,
) -> Result<Vec<AgentResult>> {
    let results = Arc::new(Mutex::new(Vec::new()));
    let mut pending: Vec<SizedBean> = units.to_vec();
    let mut handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    while !pending.is_empty() || !handles.is_empty() {
        // Check for shutdown signal
        if super::shutdown_requested() {
            super::kill_all_children();
            // Wait for threads to finish (they should exit after children are killed)
            for handle in handles {
                let _ = handle.join();
            }
            return Ok(Arc::try_unwrap(results).unwrap().into_inner().unwrap());
        }

        // Spawn up to max_jobs threads
        while handles.len() < max_jobs && !pending.is_empty() {
            let sb = pending.remove(0);
            let mana_dir = mana_dir.to_path_buf();
            let results = Arc::clone(&results);
            let timeout_min = timeout_minutes;
            let idle_min = idle_timeout_minutes;

            if json_stream {
                stream::emit(&StreamEvent::BeanStart {
                    id: sb.id.clone(),
                    title: sb.title.clone(),
                    round: wave_number,
                    file_overlaps: None,
                    attempt: None,
                    priority: None,
                });
            }

            let handle = std::thread::spawn(move || {
                let result = run_single_direct(
                    &mana_dir,
                    &sb,
                    timeout_min,
                    idle_min,
                    json_stream,
                    file_locking,
                );
                results.lock().unwrap().push(result);
            });
            handles.push(handle);
        }

        // Wait for at least one thread to finish
        let prev_count = handles.len();
        let mut still_running = Vec::new();
        for handle in handles.drain(..) {
            if handle.is_finished() {
                let _ = handle.join();
            } else {
                still_running.push(handle);
            }
        }

        // If nothing finished, wait briefly before polling again
        if still_running.len() == prev_count && !still_running.is_empty() {
            std::thread::sleep(Duration::from_millis(200));
        }

        handles = still_running;
    }

    // Wait for any remaining threads
    for handle in handles {
        let _ = handle.join();
    }

    Ok(Arc::try_unwrap(results).unwrap().into_inner().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::run::BeanAction;
    use crate::index::Index;

    #[test]
    fn compute_waves_no_deps() {
        let index = Index { units: vec![] };
        let units = vec![
            SizedBean {
                id: "1".to_string(),
                title: "A".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec![],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "2".to_string(),
                title: "B".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec![],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
        ];
        let waves = compute_waves(&units, &index);
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].units.len(), 2);
    }

    #[test]
    fn compute_waves_linear_chain() {
        let index = Index { units: vec![] };
        let units = vec![
            SizedBean {
                id: "1".to_string(),
                title: "A".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec![],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "2".to_string(),
                title: "B".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec!["1".to_string()],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "3".to_string(),
                title: "C".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec!["2".to_string()],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
        ];
        let waves = compute_waves(&units, &index);
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0].units[0].id, "1");
        assert_eq!(waves[1].units[0].id, "2");
        assert_eq!(waves[2].units[0].id, "3");
    }

    #[test]
    fn compute_waves_diamond() {
        let index = Index { units: vec![] };
        // 1 → (2, 3) → 4
        let units = vec![
            SizedBean {
                id: "1".to_string(),
                title: "Root".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec![],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "2".to_string(),
                title: "Left".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec!["1".to_string()],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "3".to_string(),
                title: "Right".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec!["1".to_string()],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
            SizedBean {
                id: "4".to_string(),
                title: "Join".to_string(),
                action: BeanAction::Implement,
                priority: 2,
                dependencies: vec!["2".to_string(), "3".to_string()],
                parent: None,
                produces: vec![],
                requires: vec![],
                paths: vec![],
            },
        ];
        let waves = compute_waves(&units, &index);
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0].units.len(), 1); // 1
        assert_eq!(waves[1].units.len(), 2); // 2, 3
        assert_eq!(waves[2].units.len(), 1); // 4
    }

    #[test]
    fn template_wave_execution_with_echo() {
        let units = vec![SizedBean {
            id: "1".to_string(),
            title: "Test".to_string(),
            action: BeanAction::Implement,
            priority: 2,
            dependencies: vec![],
            parent: None,
            produces: vec![],
            requires: vec![],
            paths: vec![],
        }];

        let results = run_wave_template(&units, "echo {id}", None, 4, 30).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].id, "1");
    }

    #[test]
    fn template_wave_runs_implement_action() {
        let units = vec![SizedBean {
            id: "1".to_string(),
            title: "Test".to_string(),
            action: BeanAction::Implement,
            priority: 2,
            dependencies: vec![],
            parent: None,
            produces: vec![],
            requires: vec![],
            paths: vec![],
        }];

        let results = run_wave_template(&units, "echo {id}", None, 4, 30).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].id, "1");
    }

    #[test]
    fn template_wave_failed_command() {
        let units = vec![SizedBean {
            id: "1".to_string(),
            title: "Fail".to_string(),
            action: BeanAction::Implement,
            priority: 2,
            dependencies: vec![],
            parent: None,
            produces: vec![],
            requires: vec![],
            paths: vec![],
        }];

        let results = run_wave_template(&units, "false", None, 4, 30).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
    }

    // -- downstream weight tests --

    fn make_bean(id: &str, deps: Vec<&str>, produces: Vec<&str>, requires: Vec<&str>) -> SizedBean {
        SizedBean {
            id: id.to_string(),
            title: format!("Bean {}", id),
            action: BeanAction::Implement,
            priority: 2,
            dependencies: deps.into_iter().map(|s| s.to_string()).collect(),
            parent: Some("p".to_string()),
            produces: produces.into_iter().map(|s| s.to_string()).collect(),
            requires: requires.into_iter().map(|s| s.to_string()).collect(),
            paths: vec![],
        }
    }

    #[test]
    fn downstream_weights_single_bean() {
        let beans = vec![make_bean("A", vec![], vec![], vec![])];
        let weights = compute_downstream_weights(&beans);
        assert_eq!(weights.get("A").copied(), Some(1));
    }

    #[test]
    fn downstream_weights_linear_chain() {
        // A → B → C (B depends on A, C depends on B)
        let beans = vec![
            make_bean("A", vec![], vec![], vec![]),
            make_bean("B", vec!["A"], vec![], vec![]),
            make_bean("C", vec!["B"], vec![], vec![]),
        ];
        let weights = compute_downstream_weights(&beans);
        assert_eq!(weights.get("A").copied(), Some(3)); // blocks B and C
        assert_eq!(weights.get("B").copied(), Some(2)); // blocks C
        assert_eq!(weights.get("C").copied(), Some(1)); // leaf
    }

    #[test]
    fn downstream_weights_diamond() {
        // A → (B, C) → D
        let beans = vec![
            make_bean("A", vec![], vec![], vec![]),
            make_bean("B", vec!["A"], vec![], vec![]),
            make_bean("C", vec!["A"], vec![], vec![]),
            make_bean("D", vec!["B", "C"], vec![], vec![]),
        ];
        let weights = compute_downstream_weights(&beans);
        assert_eq!(weights.get("D").copied(), Some(1)); // leaf
        assert_eq!(weights.get("B").copied(), Some(2)); // blocks D
        assert_eq!(weights.get("C").copied(), Some(2)); // blocks D
        assert_eq!(weights.get("A").copied(), Some(4)); // blocks B, C, D (3 descendants + 1)
    }

    #[test]
    fn downstream_weights_independent() {
        let beans = vec![
            make_bean("A", vec![], vec![], vec![]),
            make_bean("B", vec![], vec![], vec![]),
        ];
        let weights = compute_downstream_weights(&beans);
        assert_eq!(weights.get("A").copied(), Some(1));
        assert_eq!(weights.get("B").copied(), Some(1));
    }

    #[test]
    fn downstream_weights_wide_fan() {
        // A → (B, C, D)
        let beans = vec![
            make_bean("A", vec![], vec![], vec![]),
            make_bean("B", vec!["A"], vec![], vec![]),
            make_bean("C", vec!["A"], vec![], vec![]),
            make_bean("D", vec!["A"], vec![], vec![]),
        ];
        let weights = compute_downstream_weights(&beans);
        assert_eq!(weights.get("A").copied(), Some(4)); // 1 + 1 + 1 + 1
        assert_eq!(weights.get("B").copied(), Some(1));
        assert_eq!(weights.get("C").copied(), Some(1));
        assert_eq!(weights.get("D").copied(), Some(1));
    }
}
