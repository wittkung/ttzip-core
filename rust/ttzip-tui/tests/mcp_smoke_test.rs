// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Integration smoke test for TTZip Model Context Protocol (MCP) server.

use serde_json::json;
use std::fs;
use tempfile::tempdir;
use ttzip_tui::cli::handlers::mcp::{handle_request, protocol::JsonRpcRequest};

#[test]
fn test_mcp_full_lifecycle_smoke() {
    // 1. Initialize Handshake
    let init_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "integration-tester", "version": "1.0.0" }
        })),
    };
    let init_resp = handle_request(init_req).expect("Response");
    assert_eq!(init_resp.jsonrpc, "2.0");
    assert_eq!(init_resp.id, Some(json!(1)));
    let init_result = init_resp.result.expect("Result");
    assert_eq!(init_result["protocolVersion"], "2024-11-05");
    assert_eq!(init_result["serverInfo"]["name"], "ttzip-mcp");

    // 2. Notification Initialized
    let notif_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "notifications/initialized".to_string(),
        params: None,
    };
    let notif_resp = handle_request(notif_req);
    assert!(notif_resp.is_none());

    // 3. Ping
    let ping_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "ping".to_string(),
        params: None,
    };
    let ping_resp = handle_request(ping_req).expect("Response");
    assert!(ping_resp.error.is_none());

    // 4. Tools List
    let tools_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "tools/list".to_string(),
        params: None,
    };
    let tools_resp = handle_request(tools_req).expect("Response");
    let tools_result = tools_resp.result.expect("Result");
    let tools = tools_result["tools"].as_array().expect("Tools");
    assert_eq!(tools.len(), 8);

    // 5. End-to-End File Compression & Extraction
    let dir = tempdir().expect("tempdir");
    let source_dir = dir.path().join("payload");
    fs::create_dir_all(&source_dir).expect("create source dir");
    fs::write(source_dir.join("sample.txt"), b"Smoke test data for MCP server").expect("write file");

    let archive_path = dir.path().join("smoke_output.zip");

    // Call ttzip_create
    let create_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(4)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_create",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap(),
                "source_paths": [source_dir.to_str().unwrap()],
                "format": "zip",
                "level": 6
            }
        })),
    };
    let create_resp = handle_request(create_req).expect("Response");
    assert!(create_resp.error.is_none());
    assert!(archive_path.exists());

    // Call ttzip_inspect
    let inspect_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(5)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_inspect",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap()
            }
        })),
    };
    let inspect_resp = handle_request(inspect_req).expect("Response");
    assert!(inspect_resp.error.is_none());

    // Call ttzip_extract
    let dest_dir = dir.path().join("extracted_target");
    let extract_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(6)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "ttzip_extract",
            "arguments": {
                "archive_path": archive_path.to_str().unwrap(),
                "destination": dest_dir.to_str().unwrap()
            }
        })),
    };
    let extract_resp = handle_request(extract_req).expect("Response");
    assert!(extract_resp.error.is_none());
    assert!(dest_dir.join("sample.txt").exists() || dest_dir.join("payload").join("sample.txt").exists());
}
