//! Structured review feedback for agent retry context.
//!
//! When a reviewer requests changes, this module formats the feedback
//! into a structure that can be injected into the next agent's prompt
//! via imp's `TaskContext`.

use crate::types::{AnnotationSeverity, Review, ReviewDecision};

/// Format a review into context text for the next agent attempt.
///
/// This output is designed to be appended to imp's system prompt
/// as part of the task layer (Layer 5), giving the agent explicit
/// instructions about what to fix.
pub fn format_for_agent(review: &Review) -> String {
    let mut s = String::new();

    s.push_str("## Review Feedback\n\n");
    s.push_str(&format!(
        "Your last attempt (attempt {}) was reviewed by a human. ",
        review.attempt
    ));

    match &review.decision {
        ReviewDecision::ChangesRequested => {
            s.push_str("Changes were requested.\n\n");
        }
        ReviewDecision::Rejected => {
            s.push_str("The work was rejected.\n\n");
        }
        ReviewDecision::Approved => {
            s.push_str("The work was approved.\n\n");
            return s;
        }
    }

    if let Some(summary) = &review.summary {
        s.push_str(&format!("Reviewer summary: {summary}\n\n"));
    }

    if !review.annotations.is_empty() {
        s.push_str("### Specific Issues\n\n");

        for (i, ann) in review.annotations.iter().enumerate() {
            let severity_label = match ann.severity {
                AnnotationSeverity::Required => "MUST FIX",
                AnnotationSeverity::Recommended => "SHOULD FIX",
                AnnotationSeverity::Minor => "MINOR",
            };

            s.push_str(&format!("**[{}]** ", severity_label));

            if let Some(lines) = &ann.lines {
                s.push_str(&format!("`{}:{}`\n", ann.file, lines));
            } else {
                s.push_str(&format!("`{}`\n", ann.file));
            }

            s.push_str(&format!("{}\n", ann.comment));

            if let Some(suggestion) = &ann.suggestion {
                s.push_str(&format!("Suggestion: {suggestion}\n"));
            }

            if i < review.annotations.len() - 1 {
                s.push('\n');
            }
        }

        let required_count = review
            .annotations
            .iter()
            .filter(|a| a.severity == AnnotationSeverity::Required)
            .count();

        if required_count > 0 {
            s.push_str(&format!(
                "\nYou MUST address all {required_count} required issue(s). \
                 Make targeted fixes — do not rewrite working code.\n"
            ));
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use chrono::Utc;

    #[test]
    fn format_changes_requested_with_annotations() {
        let review = Review {
            unit_id: "1.3".into(),
            attempt: 3,
            decision: ReviewDecision::ChangesRequested,
            summary: Some("Good overall, fix the pool issue.".into()),
            annotations: vec![
                Annotation {
                    file: "src/auth/middleware.rs".into(),
                    lines: Some("42-58".into()),
                    comment: "Don't create a new connection pool. Use db::get_pool().".into(),
                    suggestion: Some("Call db::get_pool() instead of Pool::new()".into()),
                    severity: AnnotationSeverity::Required,
                },
                Annotation {
                    file: "src/auth/middleware.rs".into(),
                    lines: Some("10".into()),
                    comment: "Unused import.".into(),
                    suggestion: None,
                    severity: AnnotationSeverity::Minor,
                },
            ],
            reviewed_at: Utc::now(),
            reviewer: "human".into(),
        };

        let output = format_for_agent(&review);

        assert!(output.contains("attempt 3"));
        assert!(output.contains("Changes were requested"));
        assert!(output.contains("Good overall, fix the pool issue."));
        assert!(output.contains("[MUST FIX]"));
        assert!(output.contains("db::get_pool()"));
        assert!(output.contains("[MINOR]"));
        assert!(output.contains("MUST address all 1 required issue"));
    }

    #[test]
    fn format_approved_is_minimal() {
        let review = Review {
            unit_id: "1.3".into(),
            attempt: 1,
            decision: ReviewDecision::Approved,
            summary: None,
            annotations: vec![],
            reviewed_at: Utc::now(),
            reviewer: "human".into(),
        };

        let output = format_for_agent(&review);
        assert!(output.contains("approved"));
        assert!(!output.contains("Specific Issues"));
    }
}
