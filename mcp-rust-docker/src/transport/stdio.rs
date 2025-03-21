use std::io::{BufRead, BufReader, Write};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader};
use tokio::sync::mpsc;

use crate::protocol::error::McpError;
use crate::protocol::types::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;

pub struct StdioTransport {
    server: McpServer,
}

impl StdioTransport {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }

    pub async fn run(&mut self) -> Result<(), McpError> {
        let stdin = io::stdin();
        let mut stdin_reader = TokioBufReader::new(stdin);
        let mut stdout = io::stdout();

        let (tx, mut rx) = mpsc::channel::<String>(100);

        // Spawn a task to read from stdin
        let read_task = tokio::spawn(async move {
            let mut buffer = String::new();
            loop {
                buffer.clear();
                match stdin_reader.read_line(&mut buffer).await {
                    Ok(0) => {
                        // EOF
                        break;
                    }
                    Ok(_) => {
                        if let Err(e) = tx.send(buffer.clone()).await {
                            eprintln!("Failed to send message to channel: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from stdin: {}", e);
                        break;
                    }
                }
            }
        });

        // Process messages
        while let Some(message) = rx.recv().await {
            match serde_json::from_str::<JsonRpcRequest>(&message) {
                Ok(request) => {
                    let response = self.server.process_request(request).await;
                    
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
                Err(e) => {
                    eprintln!("Error parsing JSON-RPC request: {}", e);
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
            eprintln!("Error in read task: {}", e);
        }

        Ok(())
    }
}