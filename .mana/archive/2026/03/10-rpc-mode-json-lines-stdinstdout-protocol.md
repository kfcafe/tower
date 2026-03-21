---
id: '10'
title: RPC mode — JSON-lines stdin/stdout protocol
slug: rpc-mode-json-lines-stdinstdout-protocol
status: closed
priority: 2
created_at: '2026-03-20T17:42:26.531511Z'
updated_at: '2026-03-21T07:53:30.701677Z'
closed_at: '2026-03-21T07:53:30.701677Z'
parent: '2'
dependencies:
- '3'
verify: cd /Users/asher/tower && cargo build -p imp-cli 2>&1 | grep -q Finished && ! grep -q "not yet implemented" imp/crates/imp-cli/src/main.rs && grep -q "rpc\|json_lines\|stdin.*stdout" imp/crates/imp-cli/src/main.rs
fail_first: true
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-21T07:53:30.702485Z'
  finished_at: '2026-03-21T07:53:31.408583Z'
  duration_secs: 0.706
  result: pass
  exit_code: 0
---

## Problem
`imp --mode rpc` prints "not yet implemented" and exits.
This mode enables external UIs and orchestrators to drive imp via structured JSON.

## What to implement

In the `rpc` / `json` mode handler in `imp/crates/imp-cli/src/main.rs`:

### 1. Protocol
- **Input** (stdin): LF-delimited JSON objects, one per line
- **Output** (stdout): LF-delimited JSON objects, one per line

### 2. Input commands
```json
{"type":"prompt","content":"Fix the auth bug"}
{"type":"cancel"}
{"type":"steer","content":"Actually, also fix..."}
{"type":"followup","content":"Now run the tests"}
```

### 3. Output events
Serialize each AgentEvent as JSON with a `type` discriminator:
```json
{"type":"agent_start","model":"claude-sonnet-4-20250514","timestamp":1710000000}
{"type":"text_delta","text":"Let me look at..."}
{"type":"tool_execution_start","tool_call_id":"tc1","tool_name":"read","args":{"path":"src/auth.rs"}}
{"type":"tool_execution_end","tool_call_id":"tc1","is_error":false,"content":"..."}
{"type":"agent_end","input_tokens":1500,"output_tokens":300,"cost_total":0.05}
```

### 4. UI requests (bidirectional)
When a tool needs user input (ask tool), emit a UI request and wait:
```json
{"type":"ui_request","id":"q1","method":"confirm","params":{"title":"Delete?","message":"Sure?"}}
```
Wait for response on stdin:
```json
{"type":"ui_response","id":"q1","result":true}
```
Timeout after 60s → return None.

### 5. Implementation structure
```rust
async fn run_rpc_mode(cli: &Cli) -> Result<()> {
    // Create agent (same as TUI spawning pattern)
    let (mut agent, handle) = create_agent(&cli)?;

    // Spawn stdin reader task
    let cmd_tx = handle.command_tx.clone();
    tokio::spawn(async move {
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        // Read lines, parse JSON, send as AgentCommand
    });

    // Spawn stdout writer task
    tokio::spawn(async move {
        while let Some(event) = handle.event_rx.recv().await {
            let json = serde_json::to_string(&event).unwrap();
            println!("{json}");
        }
    });

    // Wait for first prompt command, then run agent
    // ...
}
```

## Files
- `imp/crates/imp-cli/src/main.rs` — MODIFY: implement RPC mode
- `imp/crates/imp-core/src/agent.rs` — READ: AgentEvent, AgentCommand
- `imp/crates/imp-core/src/ui.rs` — READ: UserInterface trait (consider StdioInterface)

## Do NOT
- Do not add WebSocket support — stdin/stdout only
- Do not add authentication to the protocol
- Do not change AgentEvent or AgentCommand types — serialize them as-is
