use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::discovery::find_unit_file;
use crate::unit::Unit;
use mana_core::config::Config;
use mana_core::ops::mutate::{run_mutation_test, MutateOpts, MutationReport};

/// Arguments for the mutate command.
pub struct MutateArgs {
    /// Unit ID to mutation-test.
    pub id: String,
    /// Maximum mutants to test (0 = all).
    pub max_mutants: usize,
    /// Timeout per verify run in seconds.
    pub timeout: Option<u64>,
    /// Git ref to diff against.
    pub diff_base: String,
    /// Output as JSON.
    pub json: bool,
}

/// Run mutation testing against a unit's verify gate.
///
/// Loads the unit, runs its verify to confirm it passes first, then mutates
/// the git diff and re-runs verify for each mutant. Reports surviving mutants.
pub fn cmd_mutate(mana_dir: &Path, args: MutateArgs) -> Result<()> {
    let bean_path =
        find_unit_file(mana_dir, &args.id).map_err(|_| anyhow!("Unit not found: {}", args.id))?;
    let unit =
        Unit::from_file(&bean_path).with_context(|| format!("Failed to load unit: {}", args.id))?;

    let verify_cmd = unit
        .verify
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow!("Unit {} has no verify command", args.id))?;

    let project_root = mana_dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot determine project root from .mana/ dir"))?;

    // Determine timeout
    let config = Config::load(mana_dir).ok();
    let timeout = args
        .timeout
        .or_else(|| unit.effective_verify_timeout(config.as_ref().and_then(|c| c.verify_timeout)));

    // First, confirm verify passes on the clean code
    eprintln!("Confirming verify passes on clean code...");
    let baseline = mana_core::ops::verify::run_verify_command(verify_cmd, project_root, timeout)?;
    if !baseline.passed {
        eprintln!("✗ Verify does not pass on clean code. Fix it before mutation testing.");
        eprintln!("  Command: {}", verify_cmd);
        if let Some(code) = baseline.exit_code {
            eprintln!("  Exit code: {}", code);
        }
        std::process::exit(1);
    }
    eprintln!("✓ Verify passes on clean code\n");

    // Run mutation testing
    eprintln!("Running mutation tests against: {}", verify_cmd);
    eprintln!("Diff base: {}", args.diff_base);
    if args.max_mutants > 0 {
        eprintln!("Max mutants: {}", args.max_mutants);
    }
    eprintln!();

    let opts = MutateOpts {
        max_mutants: args.max_mutants,
        timeout_secs: timeout,
        diff_base: args.diff_base,
    };

    let report = run_mutation_test(project_root, verify_cmd, &opts)?;

    if args.json {
        print_json_report(&report, &args.id);
    } else {
        print_human_report(&report, &args.id);
    }

    // Exit with non-zero if any mutants survived
    if report.survived > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn print_human_report(report: &MutationReport, id: &str) {
    if report.total == 0 {
        println!("No mutants generated — no changed lines found in git diff.");
        println!("Tip: Make sure you have uncommitted or staged changes.");
        return;
    }

    println!("Mutation Testing Report — Unit {}", id);
    println!("{}", "─".repeat(50));
    println!(
        "Total mutants: {}  |  Killed: {}  |  Survived: {}  |  Timed out: {}",
        report.total, report.killed, report.survived, report.timed_out
    );
    println!("Mutation score: {:.1}%", report.score);
    println!();

    // Show surviving mutants (the actionable items)
    let survivors: Vec<_> = report.results.iter().filter(|r| !r.killed).collect();
    if !survivors.is_empty() {
        println!("⚠ Surviving mutants (verify still passes with these changes):");
        println!();
        for (i, result) in survivors.iter().enumerate() {
            let m = &result.mutant;
            println!(
                "  {}. {}:{} [{}]",
                i + 1,
                m.file.display(),
                m.line_number,
                m.operator,
            );
            println!("     original: {}", m.original.trim());
            println!(
                "     mutated:  {}",
                if m.mutated.is_empty() {
                    "<deleted>"
                } else {
                    m.mutated.trim()
                }
            );
            println!();
        }
        println!("These mutations were NOT caught by the verify gate.");
        println!("Consider strengthening the verify command to detect these changes.");
    } else {
        println!("✓ All mutants killed — verify gate is strong.");
    }
}

fn print_json_report(report: &MutationReport, id: &str) {
    let survivors: Vec<serde_json::Value> = report
        .results
        .iter()
        .filter(|r| !r.killed)
        .map(|r| {
            serde_json::json!({
                "file": r.mutant.file.display().to_string(),
                "line": r.mutant.line_number,
                "operator": r.mutant.operator.to_string(),
                "original": r.mutant.original.trim(),
                "mutated": if r.mutant.mutated.is_empty() { "<deleted>" } else { r.mutant.mutated.trim() },
            })
        })
        .collect();

    let json = serde_json::json!({
        "id": id,
        "total": report.total,
        "killed": report.killed,
        "survived": report.survived,
        "timed_out": report.timed_out,
        "score": report.score,
        "survivors": survivors,
    });

    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
