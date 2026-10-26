// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Unit tests for MCP server protocol, tool execution, resources, and prompts.

use super::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_initialize_handshake() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-agent", "version": "1.0.0" }
        })),
    };

    let resp = handle_request(req).expect("Response expected");
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!(1)));
    assert!(resp.error.is_none());

    let res_obj = resp.result.expect("Result expected");
    assert_eq!(res_obj["protocolVersion"], "2024-11-05");
    assert_eq!(res_obj["serverInfo"]["name"], "ttzip-mcp");
}

#[test]
fn test_tools_list() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/list".to_string(),
        params: None,
    };

    let resp = handle_request(req).expect("Response expected");
    let result = resp.result.expect("Result expected");
    let tools = result["tools"].as_array().expect("Tools array");
    assert_eq!(tools.len(), 8);

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"ttzip_list"));
    assert!(tool_names.contains(&"ttzip_extract"));
    assert!(tool_names.contains(&"ttzip_create"));
    assert!(tool_names.contains(&"ttzip_inspect"));
    assert!(tool_names.contains(&"ttzip_cat"));
    assert!(tool_names.contains(&"ttzip_verify"));
    assert!(tool_names.contains(&"ttzip_convert"));
    assert!(tool_names.contains(&"ttzip_repair"));
}

#[test]
fn test_resources_list_and_read() {
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "resources/list".to_string(),
        params: None,
    };
    let list_resp = handle_request(list_req).expect("Response expected");
    let res_list = list_resp.result.expect("Result expected");
    let resources = res_list["resources"].as_array().expect("Resources array");
    assert!(!resources.is_empty());

    let read_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": "ttzip://manuals/format-matrix" })),
    };
    let read_resp = handle_request(read_req).expect("Response expected");
    let read_res = read_resp.result.expect("Result expected");
    let contents = read_res["contents"].as_array().expect("Contents array");
    let first = &contents[0];
    assert_eq!(first["mimeType"], "text/markdown");
    let text = first["text"].as_str().expect("Markdown text");
    assert!(text.contains("Format Matrix"));
}

#[test]
fn test_prompts_list_and_get() {
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(5)),
        method: "prompts/list".to_string(),
        params: None,
    };
    let list_resp = handle_request(list_req).expect("Response expected");
    let prompts_arr = list_resp.result.expect("Result expected");
    let prompts = prompts_arr["prompts"].as_array().expect("Prompts array");
    assert_eq!(prompts.len(), 2);

    let get_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(6)),
        method: "prompts/get".to_string(),
        params: Some(json!({
            "name": "ttzip_plan_extraction",
            "arguments": { "archive_path": "/tmp/test.zip", "target_directory": "/tmp/out" }
        })),
    };
    let get_resp = handle_request(get_req).expect("Response expected");
    let prompt_res = get_resp.result.expect("Result expected");
    let messages = prompt_res["messages"].as_array().expect("Messages");
    assert_eq!(messages.len(), 1);
    let msg_text = messages[0]["content"]["text"].as_str().expect("Prompt text");
    assert!(msg_text.contains("/tmp/test.zip"));
    assert!(msg_text.contains("/tmp/out"));
}

#[test]
fn test_e2e_archive_workflow() {
    let dir = tempdir().expect("tempdir");
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("create src dir");
    fs::write(src_dir.join("hello.txt"), b"Hello TTZip MCP!").expect("write file");
    fs::write(src_dir.join("data.json"), b"{\"status\": \"ok\"}").expect("write json");

    let archive_path = dir.path().join("bundle.zip");

    // 1. Create Archive via ttzip_create
    let create_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(10)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_create",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap(),
                "source_paths": [src_dir.to_str().unwrap()],
                "level": 6
            }
        })),
    };
    let create_resp = handle_request(create_req).expect("Response");
    assert!(create_resp.error.is_none());
    assert!(archive_path.exists());

    // 2. Inspect via ttzip_inspect
    let inspect_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(11)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_inspect",
            "arguments": { "archive_path": archive_path.to_str().unwrap() }
        })),
    };
    let inspect_resp = handle_request(inspect_req).expect("Response");
    assert!(inspect_resp.error.is_none());

    // 3. List via ttzip_list
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(12)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_list",
            "arguments": { "archive_path": archive_path.to_str().unwrap() }
        })),
    };
    let list_resp = handle_request(list_req).expect("Response");
    assert!(list_resp.error.is_none());

    // 4. Cat single file via ttzip_cat
    let cat_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(13)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_cat",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap(),
                "entry_path": "hello.txt"
            }
        })),
    };
    let cat_resp = handle_request(cat_req).expect("Response");
    assert!(cat_resp.error.is_none());

    // 5. Verify archive via ttzip_verify
    let verify_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(14)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_verify",
            "arguments": { "archive_path": archive_path.to_str().unwrap() }
        })),
    };
    let verify_resp = handle_request(verify_req).expect("Response");
    assert!(verify_resp.error.is_none());

    // 6. Extract via ttzip_extract
    let out_dir = dir.path().join("extracted");
    let extract_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(15)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_extract",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap(),
                "destination": out_dir.to_str().unwrap()
            }
        })),
    };
    let extract_resp = handle_request(extract_req).expect("Response");
    assert!(extract_resp.error.is_none());
}
