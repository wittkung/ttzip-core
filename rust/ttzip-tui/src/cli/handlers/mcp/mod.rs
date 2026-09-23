// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! TTZip Model Context Protocol (MCP) server implementation over stdio.

pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod tools;

#[cfg(test)]
mod tests;

use crate::cli::handlers::mcp::protocol::*;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

/// Main entrypoint for `ttzip mcp` subcommand. Runs the stdio event loop.
pub fn execute_mcp() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();

    for line_res in stdin.lock().lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[ttzip-mcp] Stdin read error: {}", e);
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(req) => req,
            Err(err) => {
                let err_resp = JsonRpcResponse::error(None, PARSE_ERROR, format!("Parse error: {}", err));
                if let Ok(serialized) = serde_json::to_string(&err_resp) {
                    let _ = writeln!(stdout, "{}", serialized);
                    let _ = stdout.flush();
                }
                continue;
            }
        };

        let response = handle_request(request);
        if let Some(resp) = response {
            if let Ok(serialized) = serde_json::to_string(&resp) {
                let _ = writeln!(stdout, "{}", serialized);
                let _ = stdout.flush();
            }
        }
    }

    Ok(())
}

/// Dispatches an incoming JSON-RPC request to the appropriate MCP subsystem.
pub fn handle_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = req.id;
    let method = req.method.as_str();
    let params = req.params;

    match method {
        "initialize" => {
            let res = json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false },
                    "prompts": { "listChanged": false },
                    "logging": {}
                },
                "serverInfo": {
                    "name": "ttzip-mcp",
                    "version": "1.0.0"
                }
            });
            Some(JsonRpcResponse::success(id, res))
        }
        "notifications/initialized" => {
            // Notification has no response
            None
        }
        "ping" => Some(JsonRpcResponse::success(id, json!({}))),
        "tools/list" => {
            let tools_list = tools::get_tool_definitions();
            Some(JsonRpcResponse::success(id, json!({ "tools": tools_list })))
        }
        "tools/call" => {
            let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
            let tool_name = match params_val.get("name").and_then(|v| v.as_str()) {
                Some(name) => name,
                None => {
                    return Some(JsonRpcResponse::error(
                        id,
                        INVALID_PARAMS,
                        "Missing 'name' field in tools/call parameters",
                    ));
                }
            };
            let tool_args = params_val.get("arguments");
            let tool_res = tools::execute_tool_call(tool_name, tool_args);

            match serde_json::to_value(&tool_res) {
                Ok(val) => Some(JsonRpcResponse::success(id, val)),
                Err(e) => Some(JsonRpcResponse::error(
                    id,
                    INTERNAL_ERROR,
                    format!("Failed to serialize tool result: {}", e),
                )),
            }
        }
        "resources/list" => {
            let resources_list = resources::get_resource_definitions();
            Some(JsonRpcResponse::success(
                id,
                json!({ "resources": resources_list }),
            ))
        }
        "resources/read" => {
            let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
            let uri = match params_val.get("uri").and_then(|v| v.as_str()) {
                Some(u) => u,
                None => {
                    return Some(JsonRpcResponse::error(
                        id,
                        INVALID_PARAMS,
                        "Missing 'uri' field in resources/read parameters",
                    ));
                }
            };

            match resources::read_resource(uri) {
                Ok(entry) => Some(JsonRpcResponse::success(id, json!({ "contents": [entry] }))),
                Err(e) => Some(JsonRpcResponse::error(id, INVALID_PARAMS, e)),
            }
        }
        "prompts/list" => {
            let prompts_list = prompts::get_prompt_definitions();
            Some(JsonRpcResponse::success(
                id,
                json!({ "prompts": prompts_list }),
            ))
        }
        "prompts/get" => {
            let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
            let prompt_name = match params_val.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return Some(JsonRpcResponse::error(
                        id,
                        INVALID_PARAMS,
                        "Missing 'name' field in prompts/get parameters",
                    ));
                }
            };
            let prompt_args = params_val.get("arguments");

            match prompts::get_prompt(prompt_name, prompt_args) {
                Ok((desc, msgs)) => Some(JsonRpcResponse::success(
                    id,
                    json!({ "description": desc, "messages": msgs }),
                )),
                Err(e) => Some(JsonRpcResponse::error(id, INVALID_PARAMS, e)),
            }
        }
        unknown => {
            if id.is_none() {
                None
            } else {
                Some(JsonRpcResponse::error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Method '{}' not found", unknown),
                ))
            }
        }
    }
}
