// Copyright 2026 MetastudyLine. All rights reserved.
// Proprietary and confidential.

//! JSON-RPC 2.0 & Model Context Protocol (MCP) data structures.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

/// Incoming JSON-RPC 2.0 message request or notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// Outgoing JSON-RPC 2.0 response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Standard JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tool execution content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl McpContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".to_string(),
            text: text.into(),
        }
    }
}

/// Structured MCP Tool Execution Result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

impl McpToolResult {
    pub fn ok_json<T: Serialize>(data: &T) -> Result<Self, String> {
        let text = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize tool response: {}", e))?;
        Ok(Self {
            content: vec![McpContent::text(text)],
            is_error: false,
        })
    }

    pub fn ok_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::text(text)],
            is_error: false,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::text(message)],
            is_error: true,
        }
    }

    pub fn err_structured(error_code: &str, message: impl Into<String>, details: Option<Value>) -> Self {
        let msg_str = message.into();
        let payload = serde_json::json!({
            "isError": true,
            "errorCode": error_code,
            "message": msg_str,
            "details": details.unwrap_or(Value::Null)
        });
        let text = serde_json::to_string_pretty(&payload).unwrap_or(msg_str);
        Self {
            content: vec![McpContent::text(text)],
            is_error: true,
        }
    }
}

/// Resource read content entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContentEntry {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Prompt message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Prompt message entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessageEntry {
    pub role: String,
    pub content: PromptMessageContent,
}
