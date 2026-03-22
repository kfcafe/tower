use crate::unit::Unit;

/// Score a unit's relevance to the current working context.
///
/// Scoring formula (no embeddings):
///   score = path_overlap × 3 + dependency_match × 5 + recency × 1
///
/// Higher score = more relevant.
pub fn relevance_score(unit: &Unit, working_paths: &[String], working_deps: &[String]) -> u32 {
    let mut score = 0u32;

    // Path overlap: how many of the unit's paths overlap with working paths
    let path_overlap = count_path_overlap(&unit.paths, working_paths);
    score += path_overlap * 3;

    // Dependency match: unit produces something we require, or requires something we produce
    let dep_match = count_dependency_overlap(unit, working_deps);
    score += dep_match * 5;

    // Recency: within last 7 days = 1 point, last day = 2 points
    let age = chrono::Utc::now() - unit.updated_at;
    if age.num_days() <= 1 {
        score += 2;
    } else if age.num_days() <= 7 {
        score += 1;
    }

    score
}

/// Count how many paths overlap between two sets.
/// Uses prefix matching — "src/auth" matches "src/auth/types.rs".
fn count_path_overlap(unit_paths: &[String], working_paths: &[String]) -> u32 {
    let mut count = 0;
    for bp in unit_paths {
        for wp in working_paths {
            if paths_overlap(bp, wp) {
                count += 1;
                break;
            }
        }
    }
    count
}

/// Check if two paths overlap (prefix match in either direction).
fn paths_overlap(a: &str, b: &str) -> bool {
    a.starts_with(b) || b.starts_with(a) || a == b
}

/// Count dependency overlap between a unit and a list of working dependency artifacts.
fn count_dependency_overlap(unit: &Unit, working_deps: &[String]) -> u32 {
    let mut count = 0;
    for prod in &unit.produces {
        if working_deps.contains(prod) {
            count += 1;
        }
    }
    for req in &unit.requires {
        if working_deps.contains(req) {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    #[test]
    fn test_paths_overlap_exact() {
        assert!(paths_overlap("src/auth.rs", "src/auth.rs"));
    }

    #[test]
    fn test_paths_overlap_prefix() {
        assert!(paths_overlap("src/auth", "src/auth/types.rs"));
        assert!(paths_overlap("src/auth/types.rs", "src/auth"));
    }

    #[test]
    fn test_paths_no_overlap() {
        assert!(!paths_overlap("src/auth.rs", "src/config.rs"));
    }

    #[test]
    fn test_relevance_score_path_overlap() {
        let mut unit = Unit::new("1", "Auth fact");
        unit.paths = vec!["src/auth.rs".to_string()];

        let score = relevance_score(&unit, &["src/auth.rs".to_string()], &[]);
        assert!(score >= 3); // path_overlap * 3
    }

    #[test]
    fn test_relevance_score_dependency_match() {
        let mut unit = Unit::new("1", "Auth types");
        unit.produces = vec!["AuthProvider".to_string()];

        let score = relevance_score(&unit, &[], &["AuthProvider".to_string()]);
        assert!(score >= 5); // dep_match * 5
    }

    #[test]
    fn test_relevance_score_combined() {
        let mut unit = Unit::new("1", "Auth fact");
        unit.paths = vec!["src/auth.rs".to_string()];
        unit.produces = vec!["AuthProvider".to_string()];

        let score = relevance_score(
            &unit,
            &["src/auth.rs".to_string()],
            &["AuthProvider".to_string()],
        );
        // path (3) + dep (5) + recency (2 if recent) = at least 8
        assert!(score >= 8);
    }
}
