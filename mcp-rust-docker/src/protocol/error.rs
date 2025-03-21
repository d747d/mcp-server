use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Method not found: {0}")]
    MethodNotFound(String),
    
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
    
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Prompt not found: {0}")]
    PromptNotFound(String),
    
    #[error("Docker error: {0}")]
    DockerError(String),
    
    #[error("Security error: {0}")]
    SecurityError(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Resource quota exceeded: {0}")]
    ResourceQuotaExceeded(String),
    
    #[error("Operation not permitted: {0}")]
    OperationNotPermitted(String),
    
    #[error("Operation timeout")]
    OperationTimeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl McpError {
    pub fn to_json_rpc_error(&self) -> JsonRpcError {
        let (code, message) = match self {
            McpError::ParseError(msg) => (-32700, msg.clone()),
            McpError::InvalidRequest(msg) => (-32600, msg.clone()),
            McpError::MethodNotFound(msg) => (-32601, msg.clone()),
            McpError::InvalidParams(msg) => (-32602, msg.clone()),
            McpError::InternalError(msg) => (-32603, msg.clone()),
            McpError::ResourceNotFound(msg) => (1, format!("Resource not found: {}", msg)),
            McpError::ToolNotFound(msg) => (2, format!("Tool not found: {}", msg)),
            McpError::PromptNotFound(msg) => (3, format!("Prompt not found: {}", msg)),
            McpError::DockerError(msg) => (4, format!("Docker error: {}", msg)),
            McpError::SecurityError(msg) => (5, format!("Security error: {}", msg)),
            McpError::RateLimitExceeded => (6, "Rate limit exceeded".to_string()),
            McpError::ResourceQuotaExceeded(msg) => (7, format!("Resource quota exceeded: {}", msg)),
            McpError::OperationNotPermitted(msg) => (8, format!("Operation not permitted: {}", msg)),
            McpError::OperationTimeout => (9, "Operation timeout".to_string()),
        };

        JsonRpcError {
            code,
            message,
            data: None,
        }
    }
}

impl From<bollard::errors::Error> for McpError {
    fn from(error: bollard::errors::Error) -> Self {
        McpError::DockerError(error.to_string())
    }
}

impl From<std::io::Error> for McpError {
    fn from(error: std::io::Error) -> Self {
        McpError::InternalError(error.to_string())
    }
}

impl From<serde_json::Error> for McpError {
    fn from(error: serde_json::Error) -> Self {
        McpError::ParseError(error.to_string())
    }
}