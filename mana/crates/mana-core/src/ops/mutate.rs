//! Language-agnostic mutation testing for verify gates.
//!
//! After a unit passes its verify command, this module mutates the git diff
//! (flips operators, swaps booleans, deletes lines) and re-runs verify.
//! Surviving mutants indicate a weak verify gate.
//!
//! The approach is git-diff-scoped: only lines that were actually changed
//! are candidates for mutation, keeping the mutation set focused and fast.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::ops::verify::run_verify_command;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single mutation applied to a source line.
#[derive(Debug, Clone)]
pub struct Mutant {
    /// File path relative to project root.
    pub file: PathBuf,
    /// 1-based line number in the file.
    pub line_number: usize,
    /// The original line content.
    pub original: String,
    /// The mutated line content.
    pub mutated: String,
    /// Which operator was applied.
    pub operator: MutationOperator,
}

/// The type of mutation applied.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationOperator {
    /// Flip comparison: `==` ↔ `!=`, `<` ↔ `>=`, `>` ↔ `<=`
    FlipComparison,
    /// Flip logical: `&&` ↔ `||`
    FlipLogical,
    /// Swap boolean: `true` ↔ `false`
    SwapBoolean,
    /// Flip arithmetic: `+` ↔ `-`, `*` ↔ `/`
    FlipArithmetic,
    /// Delete the entire line (replace with empty/comment).
    DeleteLine,
}

impl std::fmt::Display for MutationOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MutationOperator::FlipComparison => write!(f, "flip-comparison"),
            MutationOperator::FlipLogical => write!(f, "flip-logical"),
            MutationOperator::SwapBoolean => write!(f, "swap-boolean"),
            MutationOperator::FlipArithmetic => write!(f, "flip-arithmetic"),
            MutationOperator::DeleteLine => write!(f, "delete-line"),
        }
    }
}

/// Result of testing a single mutant.
#[derive(Debug, Clone)]
pub struct MutantResult {
    /// The mutant that was tested.
    pub mutant: Mutant,
    /// Whether the mutant was killed (verify failed = good).
    pub killed: bool,
    /// Whether the verify command timed out.
    pub timed_out: bool,
}

/// Summary of a mutation testing run.
#[derive(Debug)]
pub struct MutationReport {
    /// Total mutants generated and tested.
    pub total: usize,
    /// Mutants killed by the verify command (verify failed).
    pub killed: usize,
    /// Mutants that survived (verify still passed = weak gate).
    pub survived: usize,
    /// Mutants where verify timed out (counted as killed).
    pub timed_out: usize,
    /// Mutation score as a percentage (killed / total * 100).
    pub score: f64,
    /// Details of each mutant test.
    pub results: Vec<MutantResult>,
}

/// Options for running mutation tests.
pub struct MutateOpts {
    /// Maximum number of mutants to test (0 = all).
    pub max_mutants: usize,
    /// Timeout per verify run in seconds.
    pub timeout_secs: Option<u64>,
    /// Git ref to diff against (default: HEAD).
    pub diff_base: String,
}

impl Default for MutateOpts {
    fn default() -> Self {
        Self {
            max_mutants: 0,
            timeout_secs: Some(60),
            diff_base: "HEAD".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Core entry point
// ---------------------------------------------------------------------------

/// Run mutation testing for a unit's verify gate.
///
/// 1. Gets the git diff to identify changed lines
/// 2. Generates mutations for each changed line
/// 3. Applies each mutation, runs verify, and restores
/// 4. Returns a report of surviving mutants
pub fn run_mutation_test(
    project_root: &Path,
    verify_cmd: &str,
    opts: &MutateOpts,
) -> Result<MutationReport> {
    // Step 1: Get the diff hunks
    let hunks = get_diff_hunks(project_root, &opts.diff_base)?;
    if hunks.is_empty() {
        return Ok(MutationReport {
            total: 0,
            killed: 0,
            survived: 0,
            timed_out: 0,
            score: 100.0,
            results: vec![],
        });
    }

    // Step 2: Generate all possible mutants
    let mut mutants = Vec::new();
    for hunk in &hunks {
        let file_path = project_root.join(&hunk.file);
        if !file_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read {}", hunk.file.display()))?;
        let lines: Vec<&str> = content.lines().collect();

        for &line_num in &hunk.added_lines {
            if line_num == 0 || line_num > lines.len() {
                continue;
            }
            let line = lines[line_num - 1];
            let line_mutants = generate_mutations(&hunk.file, line_num, line);
            mutants.extend(line_mutants);
        }
    }

    // Step 3: Cap mutants if requested
    if opts.max_mutants > 0 && mutants.len() > opts.max_mutants {
        mutants.truncate(opts.max_mutants);
    }

    let total = mutants.len();
    if total == 0 {
        return Ok(MutationReport {
            total: 0,
            killed: 0,
            survived: 0,
            timed_out: 0,
            score: 100.0,
            results: vec![],
        });
    }

    // Step 4: Test each mutant
    // Group mutants by file to minimize re-reads
    let mut results = Vec::with_capacity(total);
    let mut killed = 0;
    let mut survived = 0;
    let mut timed_out_count = 0;

    // Cache original file contents for restoration
    let mut originals: HashMap<PathBuf, String> = HashMap::new();

    for mutant in mutants {
        let file_path = project_root.join(&mutant.file);
        let abs_file = file_path.clone();

        // Read and cache original content
        if !originals.contains_key(&abs_file) {
            let content = fs::read_to_string(&abs_file)
                .with_context(|| format!("Failed to read {}", mutant.file.display()))?;
            originals.insert(abs_file.clone(), content);
        }
        let original_content = originals[&abs_file].clone();

        // Apply mutation
        let mutated_content = apply_line_mutation(&original_content, mutant.line_number, &mutant.mutated);
        fs::write(&abs_file, &mutated_content)
            .with_context(|| format!("Failed to write mutated {}", mutant.file.display()))?;

        // Run verify
        let verify_result = run_verify_command(verify_cmd, project_root, opts.timeout_secs);

        // Restore original
        fs::write(&abs_file, &original_content)
            .with_context(|| format!("Failed to restore {}", mutant.file.display()))?;

        let (is_killed, is_timed_out) = match verify_result {
            Ok(vr) => {
                if vr.timed_out {
                    (true, true) // timeout = killed
                } else {
                    (!vr.passed, false) // verify failed = killed
                }
            }
            Err(_) => (true, false), // error running verify = killed
        };

        if is_killed {
            killed += 1;
        } else {
            survived += 1;
        }
        if is_timed_out {
            timed_out_count += 1;
        }

        results.push(MutantResult {
            mutant,
            killed: is_killed,
            timed_out: is_timed_out,
        });
    }

    let score = if total > 0 {
        (killed as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    Ok(MutationReport {
        total,
        killed,
        survived,
        timed_out: timed_out_count,
        score,
        results,
    })
}

// ---------------------------------------------------------------------------
// Diff parsing
// ---------------------------------------------------------------------------

/// A parsed diff hunk: file + which lines were added/changed.
#[derive(Debug)]
pub struct DiffHunk {
    /// File path relative to project root.
    pub file: PathBuf,
    /// 1-based line numbers of added/changed lines in the new version.
    pub added_lines: Vec<usize>,
}

/// Parse `git diff` output to extract changed lines per file.
///
/// Uses `--diff-filter=M` to only look at modified files (not deleted),
/// and the unified diff format to identify added lines.
pub fn get_diff_hunks(project_root: &Path, base_ref: &str) -> Result<Vec<DiffHunk>> {
    // Get the unified diff
    let output = Command::new("git")
        .args(["diff", base_ref, "--unified=0", "--no-color"])
        .current_dir(project_root)
        .output()
        .context("Failed to run git diff")?;

    if !output.status.success() {
        // Try diffing against empty tree (for initial commits)
        let output2 = Command::new("git")
            .args([
                "diff",
                "--cached",
                "--unified=0",
                "--no-color",
            ])
            .current_dir(project_root)
            .output()
            .context("Failed to run git diff --cached")?;

        if !output2.status.success() {
            return Ok(vec![]);
        }
        return parse_unified_diff(&String::from_utf8_lossy(&output2.stdout));
    }

    parse_unified_diff(&String::from_utf8_lossy(&output.stdout))
}

/// Parse unified diff output into DiffHunks.
fn parse_unified_diff(diff_text: &str) -> Result<Vec<DiffHunk>> {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current_file: Option<PathBuf> = None;
    let mut current_lines: Vec<usize> = Vec::new();
    let mut new_line_num: usize = 0;

    for line in diff_text.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            // Flush previous file
            if let Some(ref file) = current_file {
                if !current_lines.is_empty() {
                    hunks.push(DiffHunk {
                        file: file.clone(),
                        added_lines: std::mem::take(&mut current_lines),
                    });
                }
            }
            current_file = Some(PathBuf::from(rest));
        } else if line.starts_with("@@ ") {
            // Parse hunk header: @@ -old_start,old_count +new_start,new_count @@
            if let Some(new_range) = parse_hunk_header(line) {
                new_line_num = new_range.0;
            }
        } else if let Some(added) = line.strip_prefix('+') {
            // Added line — this is a mutation candidate
            if current_file.is_some() && !added.trim().is_empty() {
                current_lines.push(new_line_num);
            }
            new_line_num += 1;
        } else if !line.starts_with('-') && !line.starts_with("diff ") && !line.starts_with("index ") && !line.starts_with("--- ") {
            // Context line (no prefix) — advances line counter
            new_line_num += 1;
        }
        // Lines starting with '-' are deleted — don't advance new line counter
    }

    // Flush last file
    if let Some(file) = current_file {
        if !current_lines.is_empty() {
            hunks.push(DiffHunk {
                file,
                added_lines: current_lines,
            });
        }
    }

    Ok(hunks)
}

/// Parse a unified diff hunk header to extract the new-file range.
///
/// Format: `@@ -old_start[,old_count] +new_start[,new_count] @@`
/// Returns `(start, count)` for the new file side.
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // Find the +N,M or +N part
    let plus_idx = line.find('+')?;
    let rest = &line[plus_idx + 1..];
    let end = rest.find(' ').unwrap_or(rest.len());
    let range_str = &rest[..end];

    if let Some((start_s, count_s)) = range_str.split_once(',') {
        let start: usize = start_s.parse().ok()?;
        let count: usize = count_s.parse().ok()?;
        Some((start, count))
    } else {
        let start: usize = range_str.parse().ok()?;
        Some((start, 1))
    }
}

// ---------------------------------------------------------------------------
// Mutation generation
// ---------------------------------------------------------------------------

/// Generate all possible mutations for a single source line.
pub fn generate_mutations(
    file: &Path,
    line_number: usize,
    line: &str,
) -> Vec<Mutant> {
    let mut mutants = Vec::new();
    let trimmed = line.trim();

    // Skip empty lines, comments, and pure-whitespace
    if trimmed.is_empty() || is_comment_line(trimmed) {
        return mutants;
    }

    // Flip comparison operators
    for (from, to) in COMPARISON_SWAPS {
        if let Some(mutated) = try_replace_operator(line, from, to) {
            mutants.push(Mutant {
                file: file.to_path_buf(),
                line_number,
                original: line.to_string(),
                mutated,
                operator: MutationOperator::FlipComparison,
            });
        }
    }

    // Flip logical operators
    for (from, to) in LOGICAL_SWAPS {
        if let Some(mutated) = try_replace_operator(line, from, to) {
            mutants.push(Mutant {
                file: file.to_path_buf(),
                line_number,
                original: line.to_string(),
                mutated,
                operator: MutationOperator::FlipLogical,
            });
        }
    }

    // Swap booleans
    for (from, to) in BOOLEAN_SWAPS {
        if let Some(mutated) = try_replace_word(line, from, to) {
            mutants.push(Mutant {
                file: file.to_path_buf(),
                line_number,
                original: line.to_string(),
                mutated,
                operator: MutationOperator::SwapBoolean,
            });
        }
    }

    // Flip arithmetic operators (only when not in comments/strings — best effort)
    for (from, to) in ARITHMETIC_SWAPS {
        if let Some(mutated) = try_replace_arithmetic(line, from, to) {
            mutants.push(Mutant {
                file: file.to_path_buf(),
                line_number,
                original: line.to_string(),
                mutated,
                operator: MutationOperator::FlipArithmetic,
            });
        }
    }

    // Delete line — only for non-trivial lines
    if is_deletable_line(trimmed) {
        mutants.push(Mutant {
            file: file.to_path_buf(),
            line_number,
            original: line.to_string(),
            mutated: String::new(),
            operator: MutationOperator::DeleteLine,
        });
    }

    mutants
}

// ---------------------------------------------------------------------------
// Operator swap tables
// ---------------------------------------------------------------------------

/// Comparison operator swaps. Each pair is (from, to).
const COMPARISON_SWAPS: &[(&str, &str)] = &[
    ("===", "!=="),
    ("!==", "==="),
    ("==", "!="),
    ("!=", "=="),
    (">=", "<"),
    ("<=", ">"),
    // We handle > and < carefully to avoid matching >= and <=
];

/// Logical operator swaps.
const LOGICAL_SWAPS: &[(&str, &str)] = &[
    ("&&", "||"),
    ("||", "&&"),
    (" and ", " or "),
    (" or ", " and "),
];

/// Boolean literal swaps.
const BOOLEAN_SWAPS: &[(&str, &str)] = &[
    ("true", "false"),
    ("false", "true"),
    ("True", "False"),
    ("False", "True"),
];

/// Arithmetic operator swaps.
const ARITHMETIC_SWAPS: &[(&str, &str)] = &[
    (" + ", " - "),
    (" - ", " + "),
    (" * ", " / "),
    (" / ", " * "),
];

// ---------------------------------------------------------------------------
// Replacement helpers
// ---------------------------------------------------------------------------

/// Try to replace an operator in a line. Returns None if the operator isn't found.
fn try_replace_operator(line: &str, from: &str, to: &str) -> Option<String> {
    if line.contains(from) {
        // Replace only the first occurrence to create one mutant per swap
        Some(line.replacen(from, to, 1))
    } else {
        None
    }
}

/// Replace a word-boundary-aware token (boolean literals).
/// Avoids replacing "true" inside "truecolor" etc.
fn try_replace_word(line: &str, from: &str, to: &str) -> Option<String> {
    // Find the first occurrence and check word boundaries
    let mut search_from = 0;
    while let Some(pos) = line[search_from..].find(from) {
        let abs_pos = search_from + pos;
        let before_ok = abs_pos == 0
            || !line.as_bytes()[abs_pos - 1].is_ascii_alphanumeric()
                && line.as_bytes()[abs_pos - 1] != b'_';
        let after_pos = abs_pos + from.len();
        let after_ok = after_pos >= line.len()
            || !line.as_bytes()[after_pos].is_ascii_alphanumeric()
                && line.as_bytes()[after_pos] != b'_';

        if before_ok && after_ok {
            let mut result = String::with_capacity(line.len());
            result.push_str(&line[..abs_pos]);
            result.push_str(to);
            result.push_str(&line[after_pos..]);
            return Some(result);
        }
        search_from = abs_pos + from.len();
    }
    None
}

/// Replace arithmetic operators, avoiding common false positives.
/// Skips lines that look like imports, includes, or string-heavy lines.
fn try_replace_arithmetic(line: &str, from: &str, to: &str) -> Option<String> {
    let trimmed = line.trim();
    // Skip import/include/use/require lines
    if trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("#include")
        || trimmed.starts_with("require")
        || trimmed.starts_with("from ")
    {
        return None;
    }

    if line.contains(from) {
        Some(line.replacen(from, to, 1))
    } else {
        None
    }
}

/// Check if a line is a comment in common languages.
fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("--")
        || trimmed.starts_with(";;")
        || trimmed.starts_with('%')
}

/// Check if a line is worth deleting as a mutation.
/// Skips trivial lines like braces, imports, blank-ish content.
fn is_deletable_line(trimmed: &str) -> bool {
    // Skip trivial structural lines
    if trimmed == "{"
        || trimmed == "}"
        || trimmed == "};"
        || trimmed == "("
        || trimmed == ")"
        || trimmed == ");"
        || trimmed == "]"
        || trimmed == "];"
        || trimmed == "end"
        || trimmed == "else"
        || trimmed == "else {"
    {
        return false;
    }

    // Skip imports/includes
    if trimmed.starts_with("use ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("#include")
        || trimmed.starts_with("require")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("pub mod ")
        || trimmed.starts_with("pub use ")
    {
        return false;
    }

    // Must have some actual content
    trimmed.len() > 3
}

// ---------------------------------------------------------------------------
// File manipulation
// ---------------------------------------------------------------------------

/// Replace a specific line in file content and return the new content.
fn apply_line_mutation(content: &str, line_number: usize, replacement: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = String::with_capacity(content.len());
    let has_trailing_newline = content.ends_with('\n');

    for (i, line) in lines.iter().enumerate() {
        if i + 1 == line_number {
            if !replacement.is_empty() {
                result.push_str(replacement);
                result.push('\n');
            }
            // If replacement is empty, we skip the line (delete mutation)
        } else {
            result.push_str(line);
            if i < lines.len() - 1 || has_trailing_newline {
                result.push('\n');
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // =====================================================================
    // Hunk header parsing
    // =====================================================================

    #[test]
    fn parse_hunk_header_with_count() {
        let result = parse_hunk_header("@@ -10,5 +20,3 @@ fn foo()");
        assert_eq!(result, Some((20, 3)));
    }

    #[test]
    fn parse_hunk_header_single_line() {
        let result = parse_hunk_header("@@ -10 +20 @@ fn foo()");
        assert_eq!(result, Some((20, 1)));
    }

    #[test]
    fn parse_hunk_header_no_plus() {
        let result = parse_hunk_header("not a hunk header");
        assert_eq!(result, None);
    }

    // =====================================================================
    // Mutation generation
    // =====================================================================

    #[test]
    fn generate_comparison_mutations() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    if x == 5 {",
        );
        let comparison: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::FlipComparison)
            .collect();
        assert!(!comparison.is_empty());
        assert!(comparison[0].mutated.contains("!="));
    }

    #[test]
    fn generate_logical_mutations() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    if a && b {",
        );
        let logical: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::FlipLogical)
            .collect();
        assert!(!logical.is_empty());
        assert!(logical[0].mutated.contains("||"));
    }

    #[test]
    fn generate_boolean_mutations() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    let flag = true;",
        );
        let booleans: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::SwapBoolean)
            .collect();
        assert!(!booleans.is_empty());
        assert!(booleans[0].mutated.contains("false"));
    }

    #[test]
    fn generate_boolean_word_boundary() {
        // Should NOT mutate "true" inside "truecolor"
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    let truecolor = 1;",
        );
        let booleans: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::SwapBoolean)
            .collect();
        assert!(booleans.is_empty());
    }

    #[test]
    fn generate_arithmetic_mutations() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    let total = a + b;",
        );
        let arith: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::FlipArithmetic)
            .collect();
        assert!(!arith.is_empty());
        assert!(arith[0].mutated.contains(" - "));
    }

    #[test]
    fn generate_delete_mutations() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    println!(\"hello\");",
        );
        let deletes: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::DeleteLine)
            .collect();
        assert!(!deletes.is_empty());
        assert!(deletes[0].mutated.is_empty());
    }

    #[test]
    fn skip_comment_lines() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "    // if x == 5 {",
        );
        assert!(mutants.is_empty());
    }

    #[test]
    fn skip_trivial_lines() {
        let mutants_brace = generate_mutations(Path::new("test.rs"), 1, "    }");
        let deletes: Vec<_> = mutants_brace
            .iter()
            .filter(|m| m.operator == MutationOperator::DeleteLine)
            .collect();
        assert!(deletes.is_empty());
    }

    #[test]
    fn skip_import_lines_for_arithmetic() {
        let mutants = generate_mutations(
            Path::new("test.rs"),
            1,
            "use std::ops::{Add + Sub};",
        );
        let arith: Vec<_> = mutants
            .iter()
            .filter(|m| m.operator == MutationOperator::FlipArithmetic)
            .collect();
        assert!(arith.is_empty());
    }

    // =====================================================================
    // apply_line_mutation
    // =====================================================================

    #[test]
    fn apply_mutation_replaces_line() {
        let content = "line1\nline2\nline3\n";
        let result = apply_line_mutation(content, 2, "MUTATED");
        assert_eq!(result, "line1\nMUTATED\nline3\n");
    }

    #[test]
    fn apply_mutation_deletes_line() {
        let content = "line1\nline2\nline3\n";
        let result = apply_line_mutation(content, 2, "");
        assert_eq!(result, "line1\nline3\n");
    }

    #[test]
    fn apply_mutation_first_line() {
        let content = "line1\nline2\n";
        let result = apply_line_mutation(content, 1, "FIRST");
        assert_eq!(result, "FIRST\nline2\n");
    }

    #[test]
    fn apply_mutation_last_line() {
        let content = "line1\nline2\n";
        let result = apply_line_mutation(content, 2, "LAST");
        assert_eq!(result, "line1\nLAST\n");
    }

    // =====================================================================
    // Diff parsing
    // =====================================================================

    #[test]
    fn parse_unified_diff_basic() {
        let diff = r#"diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,2 +10,3 @@ fn main() {
     let x = 1;
+    let y = 2;
+    let z = x + y;
     println!("{}", x);
"#;
        let hunks = parse_unified_diff(diff).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].file, PathBuf::from("src/main.rs"));
        assert_eq!(hunks[0].added_lines.len(), 2);
        assert!(hunks[0].added_lines.contains(&11));
        assert!(hunks[0].added_lines.contains(&12));
    }

    #[test]
    fn parse_unified_diff_multiple_files() {
        let diff = r#"diff --git a/a.rs b/a.rs
--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,2 @@
 existing
+new line in a
diff --git a/b.rs b/b.rs
--- a/b.rs
+++ b/b.rs
@@ -5,0 +6,1 @@
+new line in b
"#;
        let hunks = parse_unified_diff(diff).unwrap();
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].file, PathBuf::from("a.rs"));
        assert_eq!(hunks[1].file, PathBuf::from("b.rs"));
    }

    #[test]
    fn parse_unified_diff_empty() {
        let hunks = parse_unified_diff("").unwrap();
        assert!(hunks.is_empty());
    }

    // =====================================================================
    // Full integration test with git repo
    // =====================================================================

    fn setup_git_repo() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();

        // Init git
        Command::new("git")
            .args(["init"])
            .current_dir(&root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&root)
            .output()
            .unwrap();

        // Initial commit
        fs::write(root.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&root)
            .output()
            .unwrap();

        (dir, root)
    }

    #[test]
    fn mutation_test_kills_mutant() {
        let (_dir, root) = setup_git_repo();

        // Make a change that can be mutated
        fs::write(
            root.join("main.rs"),
            "fn main() {\n    let x = true;\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&root)
            .output()
            .unwrap();

        // Verify command checks for "true" — mutation to "false" will kill it
        let report = run_mutation_test(
            &root,
            "grep -q 'true' main.rs",
            &MutateOpts {
                timeout_secs: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        // Should have generated at least a boolean swap mutant
        assert!(report.total > 0);
        // The boolean swap (true→false) should be killed by grep
        let bool_killed: Vec<_> = report
            .results
            .iter()
            .filter(|r| r.mutant.operator == MutationOperator::SwapBoolean && r.killed)
            .collect();
        assert!(!bool_killed.is_empty(), "Boolean mutant should be killed");
    }

    #[test]
    fn mutation_test_detects_survivor() {
        let (_dir, root) = setup_git_repo();

        // Make a change with operators
        fs::write(
            root.join("main.rs"),
            "fn main() {\n    if 1 == 1 { println!(\"yes\"); }\n}\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&root)
            .output()
            .unwrap();

        // Weak verify: always passes
        let report = run_mutation_test(
            &root,
            "true",
            &MutateOpts {
                timeout_secs: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        // All mutants should survive since verify always passes
        assert!(report.total > 0);
        assert_eq!(report.killed, 0);
        assert_eq!(report.survived, report.total);
    }

    #[test]
    fn mutation_test_no_diff() {
        let (_dir, root) = setup_git_repo();

        // No changes — no mutants
        let report = run_mutation_test(
            &root,
            "true",
            &MutateOpts::default(),
        )
        .unwrap();

        assert_eq!(report.total, 0);
        assert_eq!(report.score, 100.0);
    }

    #[test]
    fn mutation_test_max_mutants() {
        let (_dir, root) = setup_git_repo();

        // Make a change with many mutation candidates
        fs::write(
            root.join("main.rs"),
            "fn main() {\n    if a == b && c != d {\n        let x = a + b;\n        let y = true;\n    }\n}\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&root)
            .output()
            .unwrap();

        let report = run_mutation_test(
            &root,
            "true",
            &MutateOpts {
                max_mutants: 2,
                timeout_secs: Some(10),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report.total <= 2);
    }
}
