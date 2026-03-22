//! HTML review page generation.
//!
//! Generates a self-contained HTML page for reviewing a unit's changes.
//! Works in any browser today, and in Sourcery/Photon tomorrow.

use crate::types::{ChangeType, ReviewCandidate, RiskFlagKind, RiskLevel};

/// Generate a self-contained HTML review page for a unit.
pub fn generate_html(candidate: &ReviewCandidate) -> String {
    let unit = &candidate.unit;

    let risk_class = match candidate.risk_level {
        RiskLevel::Low => "low",
        RiskLevel::Normal => "normal",
        RiskLevel::High => "high",
        RiskLevel::Critical => "critical",
    };

    let description = unit.description.as_deref().unwrap_or("(no description)");
    let verify = unit.verify.as_deref().unwrap_or("(none)");
    let notes = unit.notes.as_deref().unwrap_or("");
    let acceptance = unit.acceptance.as_deref().unwrap_or("");

    let total_add: u32 = candidate.file_changes.iter().map(|f| f.additions).sum();
    let total_del: u32 = candidate.file_changes.iter().map(|f| f.deletions).sum();

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Review: {id} — {title}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
:root {{
  --bg: #0e1117; --surface: #161b22; --elevated: #1c2129; --hover: #22272e;
  --border: #30363d; --border-subtle: #21262d;
  --text: #e6edf3; --text2: #8b949e; --text3: #6e7681; --muted: #484f58;
  --green: #3fb950; --red: #f85149; --yellow: #d29922; --blue: #58a6ff; --purple: #bc8cff;
  --mono: 'SF Mono','Fira Code','JetBrains Mono',monospace;
  --sans: -apple-system,BlinkMacSystemFont,'Inter','Segoe UI',sans-serif;
}}
body {{ font-family: var(--sans); background: var(--bg); color: var(--text); font-size: 13px; line-height: 1.5; }}

.layout {{ display: flex; min-height: 100vh; }}
.sidebar {{ width: 340px; background: var(--surface); border-right: 1px solid var(--border); padding: 20px; overflow-y: auto; flex-shrink: 0; position: sticky; top: 0; height: 100vh; }}
.main {{ flex: 1; padding: 20px 24px; overflow-y: auto; }}

h1 {{ font-size: 16px; font-weight: 600; margin-bottom: 4px; }}
.unit-id {{ font-family: var(--mono); font-size: 13px; color: var(--muted); }}

.risk {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.3px; }}
.risk.low {{ background: rgba(63,185,80,0.15); color: var(--green); }}
.risk.normal {{ background: rgba(88,166,255,0.15); color: var(--blue); }}
.risk.high {{ background: rgba(210,153,34,0.15); color: var(--yellow); }}
.risk.critical {{ background: rgba(248,81,73,0.15); color: var(--red); }}

.section {{ margin-top: 16px; }}
.section-title {{ font-size: 10px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.5px; color: var(--muted); margin-bottom: 6px; }}

.context-block {{ background: var(--elevated); border: 1px solid var(--border-subtle); border-radius: 6px; padding: 10px 12px; font-size: 12px; line-height: 1.6; color: var(--text2); white-space: pre-wrap; }}

.field {{ display: flex; padding: 3px 0; font-size: 12px; }}
.field .label {{ color: var(--text3); min-width: 90px; flex-shrink: 0; }}
.field .value {{ color: var(--text); font-family: var(--mono); font-size: 11px; }}

.flag {{ background: var(--elevated); border: 1px solid var(--border-subtle); border-radius: 4px; padding: 6px 10px; margin: 4px 0; font-size: 12px; display: flex; gap: 8px; align-items: flex-start; }}
.flag .icon {{ flex-shrink: 0; }}
.flag .kind {{ color: var(--yellow); font-family: var(--mono); font-size: 10px; font-weight: 600; text-transform: uppercase; }}
.flag.scope-creep .kind {{ color: var(--yellow); }}
.flag.test-modified .kind, .flag.security-sensitive .kind {{ color: var(--red); }}
.flag .msg {{ color: var(--text2); }}
.flag .files {{ font-family: var(--mono); font-size: 10px; color: var(--text3); margin-top: 2px; }}

.file-list {{ list-style: none; }}
.file-list li {{ display: flex; align-items: center; gap: 8px; padding: 3px 0; font-size: 12px; font-family: var(--mono); }}
.file-list .badge {{ font-size: 9px; font-weight: 700; padding: 1px 5px; border-radius: 3px; text-transform: uppercase; }}
.badge.added {{ background: rgba(63,185,80,0.15); color: var(--green); }}
.badge.modified {{ background: rgba(88,166,255,0.15); color: var(--blue); }}
.badge.deleted {{ background: rgba(248,81,73,0.15); color: var(--red); }}
.badge.renamed {{ background: rgba(188,140,255,0.15); color: var(--purple); }}
.file-list .stats {{ margin-left: auto; font-size: 10px; color: var(--text3); }}
.file-list .stats .plus {{ color: var(--green); }}
.file-list .stats .minus {{ color: var(--red); }}

.attempt {{ background: var(--elevated); border-left: 2px solid var(--border); border-radius: 0 4px 4px 0; padding: 6px 10px; margin: 4px 0; font-size: 12px; }}
.attempt .num {{ font-family: var(--mono); color: var(--muted); font-size: 10px; }}
.attempt.failed {{ border-left-color: var(--red); }}
.attempt.success {{ border-left-color: var(--green); }}

.actions {{ display: flex; gap: 8px; margin-top: 16px; padding-top: 16px; border-top: 1px solid var(--border-subtle); }}
.btn {{ padding: 7px 14px; border-radius: 6px; font-size: 12px; font-weight: 600; border: 1px solid var(--border); background: var(--elevated); color: var(--text2); cursor: pointer; text-decoration: none; text-align: center; }}
.btn:hover {{ background: var(--hover); color: var(--text); }}
.btn.approve {{ background: rgba(63,185,80,0.12); border-color: var(--green); color: var(--green); }}
.btn.approve:hover {{ background: rgba(63,185,80,0.25); }}
.btn.changes {{ background: rgba(210,153,34,0.12); border-color: var(--yellow); color: var(--yellow); }}
.btn.changes:hover {{ background: rgba(210,153,34,0.25); }}
.btn.reject {{ background: rgba(248,81,73,0.08); border-color: rgba(248,81,73,0.4); color: var(--red); }}
.btn.reject:hover {{ background: rgba(248,81,73,0.2); }}

.diff-header {{ font-size: 11px; font-weight: 600; color: var(--text2); padding: 8px 0 4px; }}
pre.diff {{ background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 12px 14px; font-family: var(--mono); font-size: 11.5px; line-height: 1.65; overflow-x: auto; tab-size: 4; }}
.diff-add {{ color: var(--green); background: rgba(63,185,80,0.08); display: inline-block; width: 100%; }}
.diff-del {{ color: var(--red); background: rgba(248,81,73,0.08); display: inline-block; width: 100%; }}
.diff-hunk {{ color: var(--purple); font-weight: 600; }}
.diff-meta {{ color: var(--muted); }}

.empty {{ color: var(--text3); font-style: italic; font-size: 12px; padding: 8px 0; }}
</style>
</head>
<body>

<div class="layout">

<!-- SIDEBAR: Context -->
<div class="sidebar">
  <span class="unit-id">{id}</span>
  <h1>{title}</h1>
  <span class="risk {risk_class}">{risk_level}</span>

  <div class="section">
    <div class="section-title">Description</div>
    <div class="context-block">{description}</div>
  </div>

  {acceptance_html}

  <div class="section">
    <div class="section-title">Verify</div>
    <div class="context-block" style="font-family: var(--mono); font-size: 11px;">{verify}</div>
  </div>

  <div class="section">
    <div class="section-title">Stats</div>
    <div class="field"><span class="label">Attempts</span><span class="value">{attempts}</span></div>
    <div class="field"><span class="label">Files</span><span class="value">{file_count}</span></div>
    <div class="field"><span class="label">Changes</span><span class="value"><span style="color:var(--green)">+{additions}</span> <span style="color:var(--red)">-{deletions}</span></span></div>
  </div>

  {risk_flags_html}

  {attempts_html}

  {notes_html}

  <div class="actions">
    <a class="btn approve" href="javascript:void(0)" onclick="copyCmd('approve')">✓ Approve</a>
    <a class="btn changes" href="javascript:void(0)" onclick="copyCmd('changes')">↺ Request Changes</a>
    <a class="btn reject" href="javascript:void(0)" onclick="copyCmd('reject')">✕ Reject</a>
  </div>
  <div id="cmd-hint" style="font-size:10px; color:var(--text3); margin-top:8px; font-family:var(--mono);"></div>
</div>

<!-- MAIN: Diff -->
<div class="main">

  <div class="section">
    <div class="section-title">Files Changed</div>
    <ul class="file-list">
      {file_list_html}
    </ul>
  </div>

  <div class="section">
    <div class="section-title">Diff</div>
    {diff_html}
  </div>

</div>

</div>

<script>
function copyCmd(action) {{
  const id = '{id}';
  let cmd = '';
  if (action === 'approve') cmd = `mana review ${{id}} --approve`;
  else if (action === 'changes') cmd = `mana review ${{id}} --request-changes "YOUR FEEDBACK HERE"`;
  else if (action === 'reject') cmd = `mana review ${{id}} --reject "REASON"`;

  if (navigator.clipboard) {{
    navigator.clipboard.writeText(cmd);
    document.getElementById('cmd-hint').textContent = 'Copied: ' + cmd;
  }} else {{
    document.getElementById('cmd-hint').textContent = cmd;
  }}
}}
</script>

</body>
</html>"##,
        id = unit.id,
        title = html_escape(&unit.title),
        risk_class = risk_class,
        risk_level = candidate.risk_level,
        description = html_escape(description),
        verify = html_escape(verify),
        attempts = unit.attempts,
        file_count = candidate.file_changes.len(),
        additions = total_add,
        deletions = total_del,
        acceptance_html = render_acceptance(acceptance),
        risk_flags_html = render_risk_flags(&candidate.risk_flags),
        attempts_html = render_attempts(&unit.attempt_log),
        notes_html = render_notes(notes),
        file_list_html = render_file_list(&candidate.file_changes),
        diff_html = render_diff(&candidate.diff),
    )
}

fn render_acceptance(acceptance: &str) -> String {
    if acceptance.is_empty() {
        return String::new();
    }
    format!(
        r#"<div class="section">
    <div class="section-title">Acceptance Criteria</div>
    <div class="context-block">{}</div>
  </div>"#,
        html_escape(acceptance)
    )
}

fn render_risk_flags(flags: &[crate::types::RiskFlag]) -> String {
    if flags.is_empty() {
        return String::new();
    }

    let mut s = String::from(r#"<div class="section"><div class="section-title">Risk Flags</div>"#);

    for flag in flags {
        let css_class = match flag.kind {
            RiskFlagKind::TestModified
            | RiskFlagKind::SecuritySensitive
            | RiskFlagKind::VerifyModified => "test-modified",
            RiskFlagKind::ScopeCreep => "scope-creep",
            _ => "",
        };

        let icon = match flag.kind {
            RiskFlagKind::TestModified => "⚠",
            RiskFlagKind::SecuritySensitive => "🔒",
            RiskFlagKind::ScopeCreep => "📦",
            RiskFlagKind::ManyAttempts => "↻",
            RiskFlagKind::LargeDiff => "📏",
            RiskFlagKind::FilesDeleted => "🗑",
            RiskFlagKind::VerifyModified => "⚠",
        };

        s.push_str(&format!(
            r#"<div class="flag {css_class}"><span class="icon">{icon}</span><div><span class="kind">{kind}</span> <span class="msg">{msg}</span>"#,
            css_class = css_class,
            icon = icon,
            kind = flag.kind,
            msg = html_escape(&flag.message),
        ));

        if !flag.files.is_empty() {
            s.push_str(&format!(
                r#"<div class="files">{}</div>"#,
                flag.files.join(", ")
            ));
        }

        s.push_str("</div></div>");
    }

    s.push_str("</div>");
    s
}

fn render_attempts(attempts: &[mana_core::unit::AttemptRecord]) -> String {
    if attempts.is_empty() {
        return String::new();
    }

    let mut s =
        String::from(r#"<div class="section"><div class="section-title">Attempt History</div>"#);

    for a in attempts {
        let class = match a.outcome {
            mana_core::unit::AttemptOutcome::Success => "success",
            mana_core::unit::AttemptOutcome::Failed => "failed",
            mana_core::unit::AttemptOutcome::Abandoned => "failed",
        };

        s.push_str(&format!(
            r#"<div class="attempt {class}"><span class="num">#{num}</span> {outcome:?}"#,
            class = class,
            num = a.num,
            outcome = a.outcome,
        ));

        if let Some(ref notes) = a.notes {
            s.push_str(&format!(" — {}", html_escape(notes)));
        }

        s.push_str("</div>");
    }

    s.push_str("</div>");
    s
}

fn render_notes(notes: &str) -> String {
    if notes.is_empty() {
        return String::new();
    }
    format!(
        r#"<div class="section">
    <div class="section-title">Agent Notes</div>
    <div class="context-block">{}</div>
  </div>"#,
        html_escape(notes)
    )
}

fn render_file_list(files: &[crate::types::FileChange]) -> String {
    if files.is_empty() {
        return r#"<li class="empty">No file changes detected</li>"#.to_string();
    }

    let mut s = String::new();
    for fc in files {
        let (badge_class, badge_text) = match fc.change_type {
            ChangeType::Added => ("added", "A"),
            ChangeType::Modified => ("modified", "M"),
            ChangeType::Deleted => ("deleted", "D"),
            ChangeType::Renamed => ("renamed", "R"),
        };

        s.push_str(&format!(
            r#"<li><span class="badge {badge_class}">{badge_text}</span> {path} <span class="stats"><span class="plus">+{add}</span> <span class="minus">-{del}</span></span></li>"#,
            badge_class = badge_class,
            badge_text = badge_text,
            path = html_escape(&fc.path),
            add = fc.additions,
            del = fc.deletions,
        ));
    }
    s
}

fn render_diff(diff: &str) -> String {
    if diff.is_empty() {
        return r#"<div class="empty">No diff available</div>"#.to_string();
    }

    let mut s = String::from(r#"<pre class="diff">"#);

    for line in diff.lines() {
        let escaped = html_escape(line);
        if line.starts_with("+++") || line.starts_with("---") {
            s.push_str(&format!(r#"<span class="diff-meta">{escaped}</span>"#));
        } else if line.starts_with('+') {
            s.push_str(&format!(r#"<span class="diff-add">{escaped}</span>"#));
        } else if line.starts_with('-') {
            s.push_str(&format!(r#"<span class="diff-del">{escaped}</span>"#));
        } else if line.starts_with("@@") {
            s.push_str(&format!(r#"<span class="diff-hunk">{escaped}</span>"#));
        } else if line.starts_with("diff ") {
            s.push_str(&format!(r#"<span class="diff-meta">{escaped}</span>"#));
        } else {
            s.push_str(&escaped);
        }
        s.push('\n');
    }

    s.push_str("</pre>");
    s
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
