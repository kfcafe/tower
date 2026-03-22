use std::path::Path;

use anyhow::Result;

use crate::discovery::{find_archived_unit, find_unit_file};
use crate::index::Index;
use crate::unit::{Status, Unit};

/// A matched unit with its relevance score.
#[derive(Debug)]
pub struct RecallMatch {
    pub unit: Unit,
    pub score: u32,
}

/// Search units by substring matching.
///
/// Searches title, description, notes, close_reason, paths, labels, and
/// attempt notes. Returns matching units sorted by score (descending)
/// then recency (descending).
///
/// When `all` is true, also searches archived units.
pub fn recall(mana_dir: &Path, query: &str, all: bool) -> Result<Vec<RecallMatch>> {
    let query_lower = query.to_lowercase();
    let index = Index::load_or_rebuild(mana_dir)?;

    let mut matches: Vec<RecallMatch> = Vec::new();

    for entry in &index.units {
        if !all && entry.status == Status::Closed {
            continue;
        }

        let unit_path = match find_unit_file(mana_dir, &entry.id) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let unit = match Unit::from_file(&unit_path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if let Some(score) = score_match(&unit, &query_lower) {
            matches.push(RecallMatch { unit, score });
        }
    }

    if all {
        let archived = Index::collect_archived(mana_dir).unwrap_or_default();
        for entry in &archived {
            let unit_path = match find_archived_unit(mana_dir, &entry.id) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let unit = match Unit::from_file(&unit_path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            if let Some(score) = score_match(&unit, &query_lower) {
                matches.push(RecallMatch { unit, score });
            }
        }
    }

    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.unit.updated_at.cmp(&a.unit.updated_at))
    });

    Ok(matches)
}

/// Score how well a unit matches a query. Returns None if no match.
fn score_match(unit: &Unit, query_lower: &str) -> Option<u32> {
    let mut score = 0u32;

    if unit.title.to_lowercase().contains(query_lower) {
        score += 10;
    }

    if let Some(ref desc) = unit.description {
        if desc.to_lowercase().contains(query_lower) {
            score += 5;
        }
    }

    if let Some(ref notes) = unit.notes {
        if notes.to_lowercase().contains(query_lower) {
            score += 3;
        }
    }

    if let Some(ref reason) = unit.close_reason {
        if reason.to_lowercase().contains(query_lower) {
            score += 3;
        }
    }

    for path in &unit.paths {
        if path.to_lowercase().contains(query_lower) {
            score += 4;
            break;
        }
    }

    for label in &unit.labels {
        if label.to_lowercase().contains(query_lower) {
            score += 2;
            break;
        }
    }

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

    fn make_unit(id: &str, title: &str) -> Unit {
        Unit::new(id, title)
    }

    #[test]
    fn score_match_title() {
        let unit = make_unit("1", "Auth uses RS256");
        assert!(score_match(&unit, "rs256").is_some());
        assert!(score_match(&unit, "auth").is_some());
        assert!(score_match(&unit, "xyz").is_none());
    }

    #[test]
    fn score_match_description() {
        let mut unit = make_unit("1", "Config");
        unit.description = Some("Uses YAML format for configuration".to_string());
        assert!(score_match(&unit, "yaml").is_some());
    }

    #[test]
    fn score_match_paths() {
        let mut unit = make_unit("1", "Config");
        unit.paths = vec!["src/auth.rs".to_string()];
        assert!(score_match(&unit, "auth").is_some());
    }

    #[test]
    fn title_scores_higher_than_description() {
        let mut unit = make_unit("1", "Auth module");
        unit.description = Some("Auth is important".to_string());
        let score = score_match(&unit, "auth").unwrap();
        assert_eq!(score, 15); // Title (10) + Description (5)
    }
}
