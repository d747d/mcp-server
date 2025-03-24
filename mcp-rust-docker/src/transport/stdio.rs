use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use std::sync::Arc;

use crate::protocol::error::McpError;
use crate::protocol::types::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;

pub struct StdioTransport {
    server: Arc<McpServer>,
    request_timeout: Duration,
}

impl StdioTransport {
    pub fn new(server: McpServer) -> Self {
        let request_timeout = server.get_request_timeout();
        Self { 
            server: Arc::new(server),
            request_timeout,
        }
    }

    pub async fn run(&mut self) -> Result<(), McpError> {
        let stdin = io::stdin();
        let mut stdin_reader = TokioBufReader::new(stdin);
        let mut stdout = io::stdout();

        let (tx, mut rx) = mpsc::channel::<String>(100);

        let server = self.server.clone();
        let request_timeout = self.request_timeout;

        // Spawn a task to read from stdin
        let read_task = tokio::spawn(async move {
            let mut buffer = String::new();
            loop {
                buffer.clear();
                match stdin_reader.read_line(&mut buffer).await {
                    Ok(0) => {
                        // EOF
                        log::debug!("Reached EOF on stdin");
                        break;
                    }
                    Ok(n) => {
                        log::debug!("Read {} bytes from stdin", n);
                        // Skip empty lines
                        if buffer.trim().is_empty() {
                            continue;
                        }
                        
                        if let Err(e) = tx.send(buffer.clone()).await {
                            log::error!("Failed to send message to channel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Error reading from stdin: {}", e);
                        break;
                    }
                }
            }
        });

        // Process messages
        while let Some(message) = rx.recv().await {
            let trimmed = message.trim();
            if trimmed.is_empty() {
                continue;
            }
            
            log::debug!("Processing message: {}", trimmed);
            let server = self.server.clone();
            
            match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                Ok(request) => {
                    log::info!("Received request: method={} id={:?}", request.method, request.id);
                    
                    // Process the request with a timeout
                    let request_clone = request.clone();
                    match timeout(request_timeout, server.process_request(request)).await {
                        Ok(response) => {
                            let response_json = match serde_json::to_string(&response) {
                                Ok(json) => json,
                                Err(e) => {
                                    log::error!("Failed to serialize response: {}", e);
                                    let error_response = JsonRpcResponse {
                                        jsonrpc: "2.0".to_string(),
                                        id: request_clone.id,
                                        result: None,
                                        error: Some(crate::protocol::types::JsonRpcError {
                                            code: -32603,
                                            message: format!("Internal error: Failed to serialize response: {}", e),
                                            data: None,
                                        }),
                                    };
                                    serde_json::to_string(&error_response).unwrap_or_else(|_| {
                                        r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Critical error: Failed to serialize error response"}}"#.to_string()
                                    })
                                }
                            };
                            
                            log::debug!("Sending response: {}", response_json);
                            if let Err(e) = stdout.write_all(response_json.as_bytes()).await {
                                log::error!("Failed to write response: {}", e);
                                break;
                            }
                            
                            if let Err(e) = stdout.write_all(b"\n").await {
                                log::error!("Failed to write newline: {}", e);
                                break;
                            }
                            
                            if let Err(e) = stdout.flush().await {
                                log::error!("Failed to flush stdout: {}", e);
                                break;
                            }
                            
                            log::info!("Sent response for method={} id={:?}", request_clone.method, request_clone.id);
                        }
                        Err(_) => {
                            // Request processing timed out
                            log::error!("Request timed out: method={} id={:?}", request_clone.method, request_clone.id);
                            let error_response = JsonRpcResponse {
                                jsonrpc: "2.0".to_string(),
                                id: request_clone.id,
                                result: None,
                                error: Some(crate::protocol::types::JsonRpcError {
                                    code: -32603,
                                    message: "Request processing timed out".to_string(),
                                    data: None,
                                }),
                            };
                            
                            let response_json = serde_json::to_string(&error_response)
                                .unwrap_or_else(|_| {
                                    r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Request timed out"}}"#.to_string()
                                });
                            
                            stdout.write_all(response_json.as_bytes()).await?;
                            stdout.write_all(b"\n").await?;
                            stdout.flush().await?;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error parsing JSON-RPC request: {}", e);
                    // Send error response
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: crate::protocol::types::JsonRpcId::Null,
                        result: None,
                        error: Some(crate::protocol::types::JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
        }

        // Wait for read task to complete
        if let Err(e) = read_task.await {
            log::error!("Error in read task: {}", e);
        }

        Ok(())
    }
}