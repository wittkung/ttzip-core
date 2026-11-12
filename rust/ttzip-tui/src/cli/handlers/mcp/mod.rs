// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

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

fn handle_tools_call(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
    let Some(tool_name) = params_val.get("name").and_then(|v| v.as_str()) else {
        return JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing 'name' field in tools/call parameters",
        );
    };
    let tool_args = params_val.get("arguments");
    let tool_res = tools::execute_tool_call(tool_name, tool_args);

    match serde_json::to_value(&tool_res) {
        Ok(val) => JsonRpcResponse::success(id, val),
        Err(e) => JsonRpcResponse::error(
            id,
            INTERNAL_ERROR,
            format!("Failed to serialize tool result: {}", e),
        ),
    }
}

fn handle_resources_read(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
    let Some(uri) = params_val.get("uri").and_then(|v| v.as_str()) else {
        return JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing 'uri' field in resources/read parameters",
        );
    };

    match resources::read_resource(uri) {
        Ok(entry) => JsonRpcResponse::success(id, json!({ "contents": [entry] })),
        Err(e) => JsonRpcResponse::error(id, INVALID_PARAMS, e),
    }
}

fn handle_prompts_get(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params_val = params.unwrap_or(Value::Object(serde_json::Map::new()));
    let Some(prompt_name) = params_val.get("name").and_then(|v| v.as_str()) else {
        return JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing 'name' field in prompts/get parameters",
        );
    };
    let prompt_args = params_val.get("arguments");

    match prompts::get_prompt(prompt_name, prompt_args) {
        Ok((desc, msgs)) => JsonRpcResponse::success(
            id,
            json!({ "description": desc, "messages": msgs }),
        ),
        Err(e) => JsonRpcResponse::error(id, INVALID_PARAMS, e),
    }
}

/// Dispatches an incoming JSON-RPC request to the appropriate MCP subsystem.
pub fn handle_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = req.id;
    let params = req.params;

    match req.method.as_str() {
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
        "notifications/initialized" => None,
        "ping" => Some(JsonRpcResponse::success(id, json!({}))),
        "tools/list" => {
            let tools_list = tools::get_tool_definitions();
            Some(JsonRpcResponse::success(id, json!({ "tools": tools_list })))
        }
        "tools/call" => Some(handle_tools_call(id, params)),
        "resources/list" => {
            let resources_list = resources::get_resource_definitions();
            Some(JsonRpcResponse::success(
                id,
                json!({ "resources": resources_list }),
            ))
        }
        "resources/read" => Some(handle_resources_read(id, params)),
        "prompts/list" => {
            let prompts_list = prompts::get_prompt_definitions();
            Some(JsonRpcResponse::success(
                id,
                json!({ "prompts": prompts_list }),
            ))
        }
        "prompts/get" => Some(handle_prompts_get(id, params)),
        unknown => id.map(|req_id| {
            JsonRpcResponse::error(
                Some(req_id),
                METHOD_NOT_FOUND,
                format!("Method '{}' not found", unknown),
            )
        }),
    }
}
