use std::path::Path;

use anyhow::Result;

use crate::unit::{Unit, Status};
use crate::discovery::{find_archived_unit, find_unit_file};
use crate::index::Index;

/// Search units by substring matching (MVP — no embeddings).
///
/// Searches title, description, notes, close_reason, and paths.
/// Returns matching units sorted by relevance (title match first, then recency).
pub fn cmd_recall(mana_dir: &Path, query: &str, all: bool, json: bool) -> Result<()> {
    let query_lower = query.to_lowercase();
    let index = Index::load_or_rebuild(mana_dir)?;

    let mut matches: Vec<(Unit, u32)> = Vec::new(); // (unit, score)

    // Search active units
    for entry in &index.units {
        if !all && entry.status == Status::Closed {
            continue;
        }

        let bean_path = match find_unit_file(mana_dir, &entry.id) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let unit = match Unit::from_file(&bean_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if let Some(score) = score_match(&unit, &query_lower) {
            matches.push((unit, score));
        }
    }

    // Search archived units too
    if all {
        let archived = Index::collect_archived(mana_dir).unwrap_or_default();
        for entry in &archived {
            let bean_path = match find_archived_unit(mana_dir, &entry.id) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let unit = match Unit::from_file(&bean_path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            if let Some(score) = score_match(&unit, &query_lower) {
                matches.push((unit, score));
            }
        }
    }

    // Sort by score (descending), then by recency (descending)
    matches.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.0.updated_at.cmp(&a.0.updated_at))
    });

    if json {
        let results: Vec<serde_json::Value> = matches
            .iter()
            .map(|(unit, score)| {
                serde_json::json!({
                    "id": unit.id,
                    "title": unit.title,
                    "type": unit.bean_type,
                    "status": unit.status,
                    "score": score,
                    "close_reason": unit.close_reason,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if matches.is_empty() {
            println!("No matches for \"{}\"", query);
            return Ok(());
        }

        println!("Found {} result(s) for \"{}\":\n", matches.len(), query);

        for (unit, _score) in &matches {
            let type_icon = if unit.bean_type == "fact" {
                "📌"
            } else {
                match unit.status {
                    Status::Closed => "✓",
                    Status::InProgress => "►",
                    Status::Open => "○",
                }
            };

            let status_str = match unit.status {
                Status::Closed => {
                    let reason = unit.close_reason.as_deref().unwrap_or("closed");
                    format!("({})", reason)
                }
                _ => format!("({})", unit.status),
            };

            println!(
                "  {} [{}] {} {}",
                type_icon, unit.id, unit.title, status_str
            );

            // Show failed attempts as negative memory
            let failed_attempts: Vec<_> = unit
                .attempt_log
                .iter()
                .filter(|a| a.outcome == crate::unit::AttemptOutcome::Failed)
                .collect();

            for attempt in &failed_attempts {
                if let Some(ref notes) = attempt.notes {
                    println!("    ⚠ Attempt #{} failed: {}", attempt.num, notes);
                }
            }

            // Show description preview
            if let Some(ref desc) = unit.description {
                let preview: String = desc.chars().take(120).collect();
                let preview = preview.lines().next().unwrap_or("");
                if !preview.is_empty() {
                    println!("    {}", preview);
                }
            }
        }
    }

    Ok(())
}

/// Score how well a unit matches a query. Returns None if no match.
fn score_match(unit: &Unit, query_lower: &str) -> Option<u32> {
    let mut score = 0u32;

    // Title match (highest weight)
    if unit.title.to_lowercase().contains(query_lower) {
        score += 10;
    }

    // Description match
    if let Some(ref desc) = unit.description {
        if desc.to_lowercase().contains(query_lower) {
            score += 5;
        }
    }

    // Notes match
    if let Some(ref notes) = unit.notes {
        if notes.to_lowercase().contains(query_lower) {
            score += 3;
        }
    }

    // Close reason match
    if let Some(ref reason) = unit.close_reason {
        if reason.to_lowercase().contains(query_lower) {
            score += 3;
        }
    }

    // Path match
    for path in &unit.paths {
        if path.to_lowercase().contains(query_lower) {
            score += 4;
            break;
        }
    }

    // Labels match
    for label in &unit.labels {
        if label.to_lowercase().contains(query_lower) {
            score += 2;
            break;
        }
    }

    // Attempt notes match (negative memory search)
    for attempt in &unit.attempt_log {
        if let Some(ref notes) = attempt.notes {
            if notes.to_lowercase().contains(query_lower) {
                score += 4;
                break;
            }
        }
    }

    if score > 0 {
        Some(score)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bean(id: &str, title: &str) -> Unit {
        Unit::new(id, title)
    }

    #[test]
    fn score_match_title() {
        let unit = make_bean("1", "Auth uses RS256");
        assert!(score_match(&unit, "rs256").is_some());
        assert!(score_match(&unit, "auth").is_some());
        assert!(score_match(&unit, "xyz").is_none());
    }

    #[test]
    fn score_match_description() {
        let mut unit = make_bean("1", "Config");
        unit.description = Some("Uses YAML format for configuration".to_string());
        assert!(score_match(&unit, "yaml").is_some());
    }

    #[test]
    fn score_match_paths() {
        let mut unit = make_bean("1", "Config");
        unit.paths = vec!["src/auth.rs".to_string()];
        assert!(score_match(&unit, "auth").is_some());
    }

    #[test]
    fn score_match_notes() {
        let mut unit = make_bean("1", "Task");
        unit.notes = Some("Blocked by database migration".to_string());
        assert!(score_match(&unit, "migration").is_some());
    }

    #[test]
    fn score_match_close_reason() {
        let mut unit = make_bean("1", "Task");
        unit.close_reason = Some("Superseded by new approach".to_string());
        assert!(score_match(&unit, "superseded").is_some());
    }

    #[test]
    fn title_scores_higher_than_description() {
        let mut unit = make_bean("1", "Auth module");
        unit.description = Some("Auth is important".to_string());

        let score = score_match(&unit, "auth").unwrap();
        // Title (10) + Description (5) = 15
        assert_eq!(score, 15);
    }
}
