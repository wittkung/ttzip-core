// Copyright 2026 MetastudyLine. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! MCP Prompts catalog and template expansion.

use crate::cli::handlers::mcp::protocol::{PromptMessageContent, PromptMessageEntry};
use serde_json::{json, Value};

/// Returns registered MCP prompt templates.
pub fn get_prompt_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "ttzip_plan_extraction",
            "description": "Guides agent in formulating safe extraction strategy avoiding directory pollution and collisions",
            "arguments": [
                {
                    "name": "archive_path",
                    "description": "Path to the archive file to be extracted",
                    "required": true
                },
                {
                    "name": "target_directory",
                    "description": "Proposed extraction destination",
                    "required": false
                }
            ]
        }),
        json!({
            "name": "ttzip_plan_compression",
            "description": "Recommends optimal archive format and compression level based on workload priorities",
            "arguments": [
                {
                    "name": "source_paths",
                    "description": "Paths to files or directories to compress",
                    "required": true
                },
                {
                    "name": "priority",
                    "description": "Workload optimization priority: 'speed', 'ratio', or 'compatibility'",
                    "required": false
                }
            ]
        }),
    ]
}

/// Expands a prompt template by name with supplied arguments.
pub fn get_prompt(
    name: &str,
    arguments: Option<&Value>,
) -> Result<(Option<String>, Vec<PromptMessageEntry>), String> {
    let args = arguments.cloned().unwrap_or(Value::Object(serde_json::Map::new()));

    match name {
        "ttzip_plan_extraction" => {
            let archive_path = args.get("archive_path").and_then(|v| v.as_str()).unwrap_or("<ARCHIVE_PATH>");
            let target_dir = args.get("target_directory").and_then(|v| v.as_str()).unwrap_or("./extracted");

            let prompt_text = format!(
                "You are executing an archive extraction task for '{}'.\n\n\
                Follow this defensive extraction procedure:\n\
                1. Call `ttzip_inspect` on '{}' to check container format, encryption state, and uncompressed size.\n\
                2. If only 1-2 files are needed, use `ttzip_cat` instead of decompressing the entire archive.\n\
                3. Ensure destination is isolated: unpack into '{}' to avoid polluting the workspace (tar-bomb prevention).\n\
                4. Call `ttzip_extract` with destination and appropriate thread count.",
                archive_path, archive_path, target_dir
            );

            Ok((
                Some("Safe Archive Extraction Workflow".to_string()),
                vec![PromptMessageEntry {
                    role: "user".to_string(),
                    content: PromptMessageContent {
                        content_type: "text".to_string(),
                        text: prompt_text,
                    },
                }],
            ))
        }
        "ttzip_plan_compression" => {
            let sources = args.get("source_paths").and_then(|v| v.as_str()).unwrap_or("<SOURCES>");
            let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("compatibility");

            let (rec_format, rec_level) = match priority {
                "speed" => ("zip", 1),
                "ratio" => ("7z", 9),
                _ => ("zip", 6),
            };

            let prompt_text = format!(
                "You are planning an archive creation task for sources '{}' with priority '{}'.\n\n\
                Recommendation:\n\
                - Format: '{}'\n\
                - Compression Level: {}\n\n\
                Procedure:\n\
                1. Verify source paths exist.\n\
                2. Execute `ttzip_create` with format='{}', level={}, and source_paths=[{}].\n\
                3. Verify resulting archive using `ttzip_verify`.",
                sources, priority, rec_format, rec_level, rec_format, rec_level, sources
            );

            Ok((
                Some("Optimal Compression Strategy".to_string()),
                vec![PromptMessageEntry {
                    role: "user".to_string(),
                    content: PromptMessageContent {
                        content_type: "text".to_string(),
                        text: prompt_text,
                    },
                }],
            ))
        }
        unknown => Err(format!("Unknown prompt: '{}'", unknown)),
    }
}
