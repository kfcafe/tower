//! HTML review page generation.
//!
//! Generates a self-contained HTML page for reviewing a unit's changes.
//! Works in any browser today, and in Sourcery/Photon tomorrow.

use crate::types::ReviewCandidate;

/// Generate a self-contained HTML review page for a unit.
///
/// The page includes:
/// - Unit context (description, verify, attempts, dependencies)
/// - Risk flags
/// - Syntax-highlighted diff
/// - Action buttons (approve, request changes, reject)
///
/// Returns the complete HTML as a string.
pub fn generate_html(candidate: &ReviewCandidate) -> String {
    // TODO: implement full HTML generation
    // For now, return a placeholder that proves the pipeline works
    let unit = &candidate.unit;

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Review: {} — {}</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Inter', sans-serif; background: #0e1117; color: #e6edf3; padding: 24px; }}
  h1 {{ font-size: 18px; margin-bottom: 8px; }}
  .unit-id {{ color: #8b949e; font-family: monospace; }}
  .risk {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 12px; font-weight: 600; margin-left: 8px; }}
  .risk.low {{ background: rgba(63,185,80,0.15); color: #3fb950; }}
  .risk.normal {{ background: rgba(88,166,255,0.15); color: #58a6ff; }}
  .risk.high {{ background: rgba(210,153,34,0.15); color: #d29922; }}
  .risk.critical {{ background: rgba(248,81,73,0.15); color: #f85149; }}
  .section {{ margin-top: 20px; }}
  .section-title {{ font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; color: #8b949e; margin-bottom: 8px; }}
  .context {{ background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 12px; font-size: 13px; line-height: 1.5; }}
  .field {{ display: flex; gap: 12px; padding: 4px 0; }}
  .field .label {{ color: #8b949e; min-width: 100px; }}
  .flag {{ background: #1c2129; border: 1px solid #30363d; border-radius: 4px; padding: 6px 10px; margin: 4px 0; font-size: 12px; }}
  .flag .kind {{ color: #d29922; font-family: monospace; }}
  pre {{ background: #161b22; border: 1px solid #30363d; border-radius: 6px; padding: 12px; font-size: 12px; font-family: 'SF Mono', monospace; overflow-x: auto; white-space: pre; line-height: 1.5; }}
  .add {{ color: #3fb950; }}
  .del {{ color: #f85149; }}
  .hunk {{ color: #bc8cff; }}
</style>
</head>
<body>

<h1>
  <span class="unit-id">{}</span> {}
  <span class="risk {}">{}</span>
</h1>

<div class="section">
  <div class="section-title">Context</div>
  <div class="context">
    <div class="field"><span class="label">Description</span><span>{}</span></div>
    <div class="field"><span class="label">Verify</span><span><code>{}</code></span></div>
    <div class="field"><span class="label">Attempts</span><span>{}</span></div>
    <div class="field"><span class="label">Files changed</span><span>{}</span></div>
  </div>
</div>

{}

<div class="section">
  <div class="section-title">Diff</div>
  <pre>{}</pre>
</div>

</body>
</html>"#,
        unit.id,
        unit.title,
        unit.id,
        unit.title,
        risk_class(&candidate.risk_level),
        candidate.risk_level,
        unit.description.as_deref().unwrap_or("(no description)"),
        unit.verify.as_deref().unwrap_or("(none)"),
        unit.attempts,
        candidate.file_changes.len(),
        render_risk_flags(&candidate.risk_flags),
        syntax_highlight_diff(&candidate.diff),
    )
}

fn risk_class(level: &crate::types::RiskLevel) -> &'static str {
    match level {
        crate::types::RiskLevel::Low => "low",
        crate::types::RiskLevel::Normal => "normal",
        crate::types::RiskLevel::High => "high",
        crate::types::RiskLevel::Critical => "critical",
    }
}

fn render_risk_flags(flags: &[crate::types::RiskFlag]) -> String {
    if flags.is_empty() {
        return String::new();
    }

    let mut s = String::from(r#"<div class="section"><div class="section-title">Risk Flags</div>"#);

    for flag in flags {
        s.push_str(&format!(
            r#"<div class="flag"><span class="kind">{}</span> — {}</div>"#,
            flag.kind, flag.message,
        ));
    }

    s.push_str("</div>");
    s
}

fn syntax_highlight_diff(diff: &str) -> String {
    let mut out = String::new();

    for line in diff.lines() {
        let escaped = html_escape(line);
        if line.starts_with('+') && !line.starts_with("+++") {
            out.push_str(&format!(r#"<span class="add">{escaped}</span>"#));
        } else if line.starts_with('-') && !line.starts_with("---") {
            out.push_str(&format!(r#"<span class="del">{escaped}</span>"#));
        } else if line.starts_with("@@") {
            out.push_str(&format!(r#"<span class="hunk">{escaped}</span>"#));
        } else {
            out.push_str(&escaped);
        }
        out.push('\n');
    }

    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
