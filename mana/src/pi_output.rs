//! Parse agent JSON stdout into structured events.
//!
//! Supports both pi's `--mode json` format and imp's headless JSON format.
//! - [`AgentEvent`] — high-level enum for every event an agent emits
//! - [`parse_agent_event`] — turn a raw `serde_json::Value` line into an `AgentEvent`
//! - [`extract_file_path`] — pull the most relevant file path from tool arguments

/// High-level events extracted from pi's JSON output stream.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// Thinking delta (extended-thinking / chain-of-thought).
    Thinking { text: String },
    /// Text delta (visible assistant output).
    Text { text: String },
    /// A tool invocation has started.
    ToolStart { name: String, id: String },
    /// A tool invocation has ended — arguments are now available.
    ToolEnd {
        name: String,
        arguments: serde_json::Value,
    },
    /// Result returned from a tool execution.
    ToolResult { id: String, output: String },
    /// Token-usage snapshot emitted after each turn.
    TokenUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        cache_write: u64,
        cost: f64,
    },
    /// Agent run finished.
    Finished { total_tokens: u64, cost: f64 },
}

/// Parse a single JSON line (as a [`serde_json::Value`]) from pi's `--mode json`
/// stdout into an [`AgentEvent`].
///
/// Returns `None` for lines that don't map to a meaningful event (e.g. session
/// bookkeeping, unknown event types).
pub fn parse_agent_event(raw: &serde_json::Value) -> Option<AgentEvent> {
    // ── assistantMessageEvent deltas ────────────────────────────────
    if let Some(assistant) = raw.get("assistantMessageEvent") {
        if let Some(event_type) = assistant.get("type").and_then(|t| t.as_str()) {
            match event_type {
                "thinking_delta" => {
                    if let Some(delta) = assistant.get("delta").and_then(|d| d.as_str()) {
                        return Some(AgentEvent::Thinking {
                            text: delta.to_string(),
                        });
                    }
                }
                "text_delta" => {
                    if let Some(delta) = assistant.get("delta").and_then(|d| d.as_str()) {
                        return Some(AgentEvent::Text {
                            text: delta.to_string(),
                        });
                    }
                }
                "toolcall_start" => {
                    if let Some(msg) = raw.get("message") {
                        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                            for block in content {
                                if block.get("type").and_then(|t| t.as_str()) == Some("toolCall") {
                                    if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                        let id = block
                                            .get("id")
                                            .and_then(|i| i.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        return Some(AgentEvent::ToolStart {
                                            name: name.to_string(),
                                            id,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                "toolcall_end" => {
                    if let Some(msg) = raw.get("message") {
                        if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                            for block in content {
                                if block.get("type").and_then(|t| t.as_str()) == Some("toolCall") {
                                    let name = block
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let arguments = block
                                        .get("arguments")
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);
                                    return Some(AgentEvent::ToolEnd { name, arguments });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ── turn_end → ToolResult or TokenUpdate ───────────────────────
    if raw.get("type").and_then(|t| t.as_str()) == Some("turn_end") {
        if let Some(msg) = raw.get("message") {
            // Tool results live in content blocks
            if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("toolResult") {
                        let id = block
                            .get("toolUseId")
                            .and_then(|i| i.as_str())
                            .unwrap_or("")
                            .to_string();
                        let output = block
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string();
                        return Some(AgentEvent::ToolResult { id, output });
                    }
                }
            }

            // Token usage
            if let Some(usage) = msg.get("usage") {
                let input = usage
                    .get("inputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let output = usage
                    .get("outputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let cache_read = usage
                    .get("cacheReadInputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let cache_write = usage
                    .get("cacheCreationInputTokens")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(0);
                let cost = usage
                    .get("cost")
                    .and_then(|c| c.get("total"))
                    .and_then(|t| t.as_f64())
                    .unwrap_or(0.0);

                if input > 0 || output > 0 {
                    return Some(AgentEvent::TokenUpdate {
                        input_tokens: input,
                        output_tokens: output,
                        cache_read,
                        cache_write,
                        cost,
                    });
                }
            }
        }
    }

    // ── result → Finished ──────────────────────────────────────────
    if let Some(result) = raw.get("result") {
        let total_tokens = result
            .get("totalTokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0);
        let cost = result.get("cost").and_then(|c| c.as_f64()).unwrap_or(0.0);
        return Some(AgentEvent::Finished { total_tokens, cost });
    }

    None
}

/// Extract the most relevant file path from a tool call's arguments.
///
/// For `Read`, `Write`, and `Edit` the `"path"` field is returned directly.
/// For `Bash` we scan the command string for tokens that look like file paths.
pub fn extract_file_path(tool_name: &str, arguments: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Write" | "Edit" => arguments
            .get("path")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string()),
        "Bash" => {
            if let Some(cmd) = arguments.get("command").and_then(|c| c.as_str()) {
                for word in cmd.split_whitespace() {
                    if word.contains('/')
                        || word.ends_with(".rs")
                        || word.ends_with(".ts")
                        || word.ends_with(".py")
                        || word.ends_with(".js")
                        || word.ends_with(".md")
                    {
                        return Some(word.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── parse_agent_event ──────────────────────────────────────────

    #[test]
    fn pi_output_thinking_delta() {
        let raw = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "thinking_delta",
                "delta": "Let me think..."
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::Thinking {
                text: "Let me think...".into()
            })
        );
    }

    #[test]
    fn pi_output_text_delta() {
        let raw = json!({
            "type": "message_update",
            "assistantMessageEvent": {
                "type": "text_delta",
                "delta": "Hello!"
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::Text {
                text: "Hello!".into()
            })
        );
    }

    #[test]
    fn pi_output_toolcall_start() {
        let raw = json!({
            "type": "message_update",
            "assistantMessageEvent": { "type": "toolcall_start", "content_index": 1 },
            "message": {
                "content": [
                    { "type": "text", "text": "hi" },
                    { "type": "toolCall", "id": "tc_01", "name": "Read" }
                ]
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::ToolStart {
                name: "Read".into(),
                id: "tc_01".into(),
            })
        );
    }

    #[test]
    fn pi_output_toolcall_end() {
        let raw = json!({
            "type": "message_update",
            "assistantMessageEvent": { "type": "toolcall_end" },
            "message": {
                "content": [{
                    "type": "toolCall",
                    "id": "tc_01",
                    "name": "Read",
                    "arguments": { "path": "src/main.rs" }
                }]
            }
        });
        match parse_agent_event(&raw) {
            Some(AgentEvent::ToolEnd { name, arguments }) => {
                assert_eq!(name, "Read");
                assert_eq!(arguments, json!({ "path": "src/main.rs" }));
            }
            other => unreachable!("expected ToolEnd, got {:?}", other),
        }
    }

    #[test]
    fn pi_output_tool_result() {
        let raw = json!({
            "type": "turn_end",
            "message": {
                "role": "user",
                "content": [{
                    "type": "toolResult",
                    "toolUseId": "tc_01",
                    "content": "fn main() {}"
                }]
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::ToolResult {
                id: "tc_01".into(),
                output: "fn main() {}".into(),
            })
        );
    }

    #[test]
    fn pi_output_token_update() {
        let raw = json!({
            "type": "turn_end",
            "message": {
                "role": "assistant",
                "content": [{ "type": "text", "text": "done" }],
                "usage": {
                    "inputTokens": 1000,
                    "outputTokens": 200,
                    "cacheReadInputTokens": 500,
                    "cacheCreationInputTokens": 100,
                    "cost": { "total": 0.005 }
                }
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::TokenUpdate {
                input_tokens: 1000,
                output_tokens: 200,
                cache_read: 500,
                cache_write: 100,
                cost: 0.005,
            })
        );
    }

    #[test]
    fn pi_output_token_update_zero_tokens_ignored() {
        let raw = json!({
            "type": "turn_end",
            "message": {
                "usage": {
                    "inputTokens": 0,
                    "outputTokens": 0
                }
            }
        });
        assert_eq!(parse_agent_event(&raw), None);
    }

    #[test]
    fn pi_output_finished() {
        let raw = json!({
            "type": "agent_end",
            "result": {
                "totalTokens": 5000,
                "cost": 0.03
            }
        });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::Finished {
                total_tokens: 5000,
                cost: 0.03,
            })
        );
    }

    #[test]
    fn pi_output_finished_missing_fields() {
        let raw = json!({ "result": {} });
        assert_eq!(
            parse_agent_event(&raw),
            Some(AgentEvent::Finished {
                total_tokens: 0,
                cost: 0.0,
            })
        );
    }

    #[test]
    fn pi_output_unknown_event_returns_none() {
        let raw = json!({ "type": "session", "id": "abc" });
        assert_eq!(parse_agent_event(&raw), None);
    }

    #[test]
    fn pi_output_empty_object_returns_none() {
        assert_eq!(parse_agent_event(&json!({})), None);
    }

    // ── extract_file_path ──────────────────────────────────────────

    #[test]
    fn pi_output_extract_read_path() {
        let args = json!({ "path": "src/lib.rs" });
        assert_eq!(extract_file_path("Read", &args), Some("src/lib.rs".into()));
    }

    #[test]
    fn pi_output_extract_write_path() {
        let args = json!({ "path": "out/result.json" });
        assert_eq!(
            extract_file_path("Write", &args),
            Some("out/result.json".into())
        );
    }

    #[test]
    fn pi_output_extract_edit_path() {
        let args = json!({ "path": "Cargo.toml", "oldText": "a", "newText": "b" });
        assert_eq!(extract_file_path("Edit", &args), Some("Cargo.toml".into()));
    }

    #[test]
    fn pi_output_extract_bash_with_file() {
        let args = json!({ "command": "cat src/main.rs" });
        assert_eq!(extract_file_path("Bash", &args), Some("src/main.rs".into()));
    }

    #[test]
    fn pi_output_extract_bash_with_path() {
        let args = json!({ "command": "ls /tmp/foo" });
        assert_eq!(extract_file_path("Bash", &args), Some("/tmp/foo".into()));
    }

    #[test]
    fn pi_output_extract_bash_no_file() {
        let args = json!({ "command": "echo hello" });
        assert_eq!(extract_file_path("Bash", &args), None);
    }

    #[test]
    fn pi_output_extract_unknown_tool() {
        let args = json!({ "path": "foo.rs" });
        assert_eq!(extract_file_path("CustomTool", &args), None);
    }

    #[test]
    fn pi_output_extract_missing_path_field() {
        let args = json!({ "content": "hello" });
        assert_eq!(extract_file_path("Read", &args), None);
    }
}
