//! MCP stdio server: reads JSON-RPC 2.0 from stdin, writes responses to stdout.

use std::io::{self, BufRead, Write};
use std::path::Path;

use serde_json::{json, Value};

use crate::mcp::protocol::{
    JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND, PARSE_ERROR,
};
use crate::mcp::resources;
use crate::mcp::tools;

/// Protocol version we support.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Run the MCP server loop on stdin/stdout.
///
/// Reads newline-delimited JSON-RPC 2.0 messages from stdin,
/// dispatches to the appropriate handler, and writes responses
/// to stdout. Notifications (no `id`) do not get responses.
pub fn run(mana_dir: &Path) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    eprintln!("units MCP server started (protocol {})", PROTOCOL_VERSION);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("stdin read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                // Parse error — respond with error if we can extract an id
                let error_response =
                    JsonRpcResponse::error(Value::Null, PARSE_ERROR, format!("Parse error: {}", e));
                write_response(&mut writer, &error_response)?;
                continue;
            }
        };

        // Notifications (no id) don't get responses
        let id = match request.id {
            Some(id) => id,
            None => {
                // Handle notification silently
                handle_notification(&request.method);
                continue;
            }
        };

        let response = dispatch(&request.method, &request.params, id.clone(), mana_dir);
        write_response(&mut writer, &response)?;
    }

    eprintln!("units MCP server shutting down");
    Ok(())
}

/// Dispatch a JSON-RPC request to the appropriate handler.
fn dispatch(method: &str, params: &Option<Value>, id: Value, mana_dir: &Path) -> JsonRpcResponse {
    match method {
        "initialize" => handle_initialize(params, id),
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(params, id, mana_dir),
        "resources/list" => handle_resources_list(id),
        "resources/read" => handle_resources_read(params, id, mana_dir),
        "ping" => JsonRpcResponse::success(id, json!({})),
        _ => JsonRpcResponse::error(id, METHOD_NOT_FOUND, format!("Unknown method: {}", method)),
    }
}

/// Handle notifications (no response needed).
fn handle_notification(method: &str) {
    match method {
        "notifications/initialized" => {
            eprintln!("Client initialized");
        }
        "notifications/cancelled" => {
            eprintln!("Request cancelled by client");
        }
        _ => {
            eprintln!("Unknown notification: {}", method);
        }
    }
}

/// Write a JSON-RPC response as a single line to stdout.
fn write_response(writer: &mut impl Write, response: &JsonRpcResponse) -> anyhow::Result<()> {
    let json = serde_json::to_string(response)?;
    writeln!(writer, "{}", json)?;
    writer.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP Method Handlers
// ---------------------------------------------------------------------------

fn handle_initialize(params: &Option<Value>, id: Value) -> JsonRpcResponse {
    let _client_info = params
        .as_ref()
        .and_then(|p| p.get("clientInfo"))
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown");

    eprintln!("Initializing with client: {}", _client_info);

    JsonRpcResponse::success(
        id,
        json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "units",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tool_defs = tools::tool_definitions();
    let tools_json: Vec<Value> = tool_defs
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            })
        })
        .collect();

    JsonRpcResponse::success(id, json!({ "tools": tools_json }))
}

fn handle_tools_call(params: &Option<Value>, id: Value, mana_dir: &Path) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing params");
        }
    };

    let name = match params.get("name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing tool name");
        }
    };

    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    eprintln!("Tool call: {}", name);
    let result = tools::handle_tool_call(name, &args, mana_dir);
    JsonRpcResponse::success(id, result)
}

fn handle_resources_list(id: Value) -> JsonRpcResponse {
    let resource_defs = resources::resource_definitions();
    let resources_json: Vec<Value> = resource_defs
        .iter()
        .map(|r| {
            let mut obj = json!({
                "uri": r.uri,
                "name": r.name,
            });
            if let Some(ref desc) = r.description {
                obj["description"] = json!(desc);
            }
            if let Some(ref mime) = r.mime_type {
                obj["mimeType"] = json!(mime);
            }
            obj
        })
        .collect();

    JsonRpcResponse::success(id, json!({ "resources": resources_json }))
}

fn handle_resources_read(params: &Option<Value>, id: Value, mana_dir: &Path) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing params");
        }
    };

    let uri = match params.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "Missing resource URI");
        }
    };

    match resources::handle_resource_read(uri, mana_dir) {
        Ok(contents) => {
            let contents_json: Vec<Value> = contents
                .iter()
                .map(|c| {
                    let mut obj = json!({
                        "uri": c.uri,
                        "text": c.text,
                    });
                    if let Some(ref mime) = c.mime_type {
                        obj["mimeType"] = json!(mime);
                    }
                    obj
                })
                .collect();
            JsonRpcResponse::success(id, json!({ "contents": contents_json }))
        }
        Err(e) => JsonRpcResponse::error(id, INTERNAL_ERROR, format!("Resource error: {}", e)),
    }
}
