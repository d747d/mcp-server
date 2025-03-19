// Cargo.toml
/*
[package]
name = "mcp-postgres-server"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.32", features = ["full"] }
tokio-postgres = "0.7"
postgres-types = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bytes = "1.5"
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
dotenv = "0.15"
*/

use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use postgres_types::Type;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_postgres::{Client, NoTls};
use tracing::{error, info, instrument};

// MCP Protocol message types
const MSG_TYPE_QUERY: u8 = 1;
const MSG_TYPE_RESPONSE: u8 = 2;
const MSG_TYPE_ERROR: u8 = 3;
const MSG_TYPE_HANDSHAKE: u8 = 4;
const MSG_TYPE_HANDSHAKE_RESPONSE: u8 = 5;

// MCP Protocol version
const PROTOCOL_VERSION: u16 = 1;

#[derive(Error, Debug)]
enum McpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Write operation attempted")]
    WriteAttempted,
    
    #[error("Authentication failed")]
    AuthFailed,
}

#[derive(Debug, Serialize, Deserialize)]
struct HandshakeRequest {
    client_name: String,
    auth_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HandshakeResponse {
    success: bool,
    server_name: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryRequest {
    query: String,
    params: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryResponse {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    row_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

struct DbConnection {
    client: Client,
}

impl DbConnection {
    async fn new() -> Result<Self> {
        dotenv::dotenv().ok();
        
        let db_host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
        let db_name = env::var("POSTGRES_DB").unwrap_or_else(|_| "postgres".to_string());
        let db_user = env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
        let db_pass = env::var("POSTGRES_PASSWORD").expect("POSTGRES_PASSWORD must be set");
        
        let connection_string = format!(
            "host={} port={} dbname={} user={} password={}",
            db_host, db_port, db_name, db_user, db_pass
        );
        
        let (client, connection) = tokio_postgres::connect(&connection_string, NoTls)
            .await
            .context("Failed to connect to PostgreSQL")?;
        
        // Spawn the connection task to the runtime
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("Connection error: {}", e);
            }
        });
        
        Ok(Self { client })
    }
    
    #[instrument(skip(self))]
    async fn execute_read_query(&self, query: &str, params: &[&(dyn tokio_postgres::types::ToSql + Sync)]) 
        -> Result<QueryResponse> {
        
        // Check if query is trying to perform a write operation
        let normalized_query = query.trim().to_lowercase();
        if normalized_query.starts_with("insert") || 
           normalized_query.starts_with("update") || 
           normalized_query.starts_with("delete") || 
           normalized_query.starts_with("drop") || 
           normalized_query.starts_with("create") || 
           normalized_query.starts_with("alter") {
            return Err(McpError::WriteAttempted.into());
        }
        
        // Execute the query
        let rows = self.client
            .query(query, params)
            .await
            .context("Failed to execute query")?;
        
        // If no rows, return empty response with column names
        if rows.is_empty() {
            let statement = self.client
                .prepare(query)
                .await
                .context("Failed to prepare statement")?;
            
            let columns = statement
                .columns()
                .iter()
                .map(|col| col.name().to_string())
                .collect();
            
            return Ok(QueryResponse {
                columns,
                rows: vec![],
                row_count: 0,
            });
        }
        
        // Get column names from the first row
        let columns = rows[0]
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();
        
        // Convert rows to JSON-compatible format
        let mut result_rows = Vec::with_capacity(rows.len());
        
        for row in &rows {
            let mut values = Vec::with_capacity(row.columns().len());
            
            for (i, column) in row.columns().iter().enumerate() {
                let value = match column.type_() {
                    &Type::BOOL => {
                        let val: Option<bool> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::INT2 | &Type::INT4 => {
                        let val: Option<i32> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::INT8 => {
                        let val: Option<i64> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::FLOAT4 => {
                        let val: Option<f32> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::FLOAT8 => {
                        let val: Option<f64> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::TEXT | &Type::VARCHAR => {
                        let val: Option<String> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::JSON | &Type::JSONB => {
                        // Fix: Convert JSON type data to string first
                        let val: Option<String> = row.get(i);
                        match val {
                            Some(json_str) => {
                                let parsed: serde_json::Value = serde_json::from_str(&json_str)?;
                                serde_json::to_value(Some(parsed))?
                            },
                            None => serde_json::to_value(None::<serde_json::Value>)?
                        }
                    },
                    &Type::TIMESTAMP | &Type::TIMESTAMPTZ => {
                        // Fix: Get timestamp as string to avoid generic parameter issues
                        let val: Option<String> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    &Type::DATE => {
                        // Fix: Get date as string to avoid generic parameter issues
                        let val: Option<String> = row.get(i);
                        serde_json::to_value(val)?
                    },
                    _ => {
                        // For other types, get as string representation
                        let val: Option<String> = row.try_get(i)
                            .unwrap_or_else(|_| Some("<binary data>".to_string()));
                        serde_json::to_value(val)?
                    }
                };
                
                values.push(value);
            }
            
            result_rows.push(values);
        }
        
        Ok(QueryResponse {
            columns,
            rows: result_rows,
            row_count: rows.len(),
        })
    }
}

struct McpSession {
    connection: Arc<DbConnection>,
    stream: TcpStream,
    buffer: BytesMut,
    authenticated: bool,
}

impl McpSession {
    fn new(connection: Arc<DbConnection>, stream: TcpStream) -> Self {
        Self {
            connection,
            stream,
            buffer: BytesMut::with_capacity(4096),
            authenticated: false,
        }
    }
    
    async fn process(&mut self) -> Result<()> {
        loop {
            // Read data from the client
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?;
            if bytes_read == 0 {
                // Client disconnected
                return Ok(());
            }
            
            // Process the message
            self.process_message().await?;
        }
    }
    
    async fn process_message(&mut self) -> Result<()> {
        // We need at least 5 bytes for a valid message (1 type + 4 length)
        if self.buffer.len() < 5 {
            return Ok(());
        }
        
        // Read message type and length
        let msg_type = self.buffer[0];
        let length = (&self.buffer[1..5]).get_u32() as usize;
        
        // Check if we have the complete message
        if self.buffer.len() < 5 + length {
            return Ok(());
        }
        
        // Extract the message payload
        let payload = self.buffer.split_to(5 + length).freeze().slice(5..);
        
        // Process based on message type
        match msg_type {
            MSG_TYPE_HANDSHAKE => {
                self.handle_handshake(payload).await?;
            },
            MSG_TYPE_QUERY => {
                if !self.authenticated {
                    self.send_error("Not authenticated", "AUTH_REQUIRED").await?;
                    return Ok(());
                }
                self.handle_query(payload).await?;
            },
            _ => {
                self.send_error("Unknown message type", "INVALID_MESSAGE").await?;
            }
        }
        
        Ok(())
    }
    
    async fn handle_handshake(&mut self, payload: bytes::Bytes) -> Result<()> {
        // Parse handshake request
        let handshake: HandshakeRequest = serde_json::from_slice(&payload)?;
        
        // In a real app, validate the auth_token
        // For this example, we use a simple environment variable token
        let expected_token = env::var("AUTH_TOKEN").unwrap_or_else(|_| "development_token".to_string());
        let success = handshake.auth_token == expected_token;
        
        if success {
            self.authenticated = true;
        }
        
        // Create response
        let response = HandshakeResponse {
            success,
            server_name: "Rust MCP PostgreSQL Server".to_string(),
            message: if success {
                "Authentication successful".to_string()
            } else {
                "Authentication failed".to_string()
            },
        };
        
        // Send response
        self.send_message(MSG_TYPE_HANDSHAKE_RESPONSE, &response).await?;
        
        if !success {
            return Err(McpError::AuthFailed.into());
        }
        
        Ok(())
    }
    
    async fn handle_query(&mut self, payload: bytes::Bytes) -> Result<()> {
        // Parse query request
        let query_req: QueryRequest = serde_json::from_slice(&payload)?;
        
        // Convert params to reference slice
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = query_req.params
            .iter()
            .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();
        
        // Execute the query
        match self.connection.execute_read_query(&query_req.query, &param_refs).await {
            Ok(response) => {
                self.send_message(MSG_TYPE_RESPONSE, &response).await?;
            },
            Err(e) => {
                let error_code = match e.downcast_ref::<McpError>() {
                    Some(McpError::WriteAttempted) => "WRITE_ATTEMPT",
                    Some(McpError::Database(_)) => "DB_ERROR",
                    _ => "QUERY_ERROR",
                };
                
                self.send_error(&e.to_string(), error_code).await?;
            }
        }
        
        Ok(())
    }
    
    async fn send_message<T: Serialize>(&mut self, msg_type: u8, data: &T) -> Result<()> {
        // Serialize the data
        let json = serde_json::to_vec(data)?;
        
        // Create the message buffer
        let mut buffer = BytesMut::with_capacity(5 + json.len());
        buffer.put_u8(msg_type);
        buffer.put_u32(json.len() as u32);
        buffer.extend_from_slice(&json);
        
        // Send the message
        self.stream.write_all(&buffer).await?;
        
        Ok(())
    }
    
    async fn send_error(&mut self, message: &str, code: &str) -> Result<()> {
        let error = ErrorResponse {
            code: code.to_string(),
            message: message.to_string(),
        };
        
        self.send_message(MSG_TYPE_ERROR, &error).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Get server configuration
    let host = env::var("MCP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("MCP_PORT").unwrap_or_else(|_| "9000".to_string());
    let address = format!("{}:{}", host, port);
    
    // Create database connection
    let db_connection = Arc::new(DbConnection::new().await?);
    
    // Create TCP listener
    let listener = TcpListener::bind(&address).await?;
    info!("MCP Server listening on {}", address);
    
    // Accept connections
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from: {}", addr);
                
                // Clone the database connection for this session
                let connection = Arc::clone(&db_connection);
                
                // Spawn a new task to handle this client
                tokio::spawn(async move {
                    let mut session = McpSession::new(connection, stream);
                    if let Err(e) = session.process().await {
                        error!("Session error: {}", e);
                    }
                });
            },
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

// Client example (can be in a separate file)
#[allow(dead_code)]
async fn client_example() -> Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:9000").await?;
    let mut buffer = BytesMut::with_capacity(4096);
    
    // Send handshake
    let handshake = HandshakeRequest {
        client_name: "Example Client".to_string(),
        auth_token: "development_token".to_string(),
    };
    
    let json = serde_json::to_vec(&handshake)?;
    let mut msg = BytesMut::with_capacity(5 + json.len());
    msg.put_u8(MSG_TYPE_HANDSHAKE);
    msg.put_u32(json.len() as u32);
    msg.extend_from_slice(&json);
    
    stream.write_all(&msg).await?;
    
    // Read response
    stream.read_buf(&mut buffer).await?;
    
    // Process response
    let msg_type = buffer[0];
    let length = (&buffer[1..5]).get_u32() as usize;
    let payload = buffer.split_to(5 + length).freeze().slice(5..);
    
    if msg_type == MSG_TYPE_HANDSHAKE_RESPONSE {
        let response: HandshakeResponse = serde_json::from_slice(&payload)?;
        println!("Handshake response: {:?}", response);
        
        if response.success {
            // Send a query
            let query = QueryRequest {
                query: "SELECT * FROM users LIMIT 10".to_string(),
                params: vec![],
            };
            
            let json = serde_json::to_vec(&query)?;
            let mut msg = BytesMut::with_capacity(5 + json.len());
            msg.put_u8(MSG_TYPE_QUERY);
            msg.put_u32(json.len() as u32);
            msg.extend_from_slice(&json);
            
            stream.write_all(&msg).await?;
            
            // Read query response
            stream.read_buf(&mut buffer).await?;
            
            // Process query response
            let msg_type = buffer[0];
            let length = (&buffer[1..5]).get_u32() as usize;
            let payload = buffer.split_to(5 + length).freeze().slice(5..);
            
            if msg_type == MSG_TYPE_RESPONSE {
                let response: QueryResponse = serde_json::from_slice(&payload)?;
                println!("Query response: {:?}", response);
            } else if msg_type == MSG_TYPE_ERROR {
                let error: ErrorResponse = serde_json::from_slice(&payload)?;
                println!("Error: {} ({})", error.message, error.code);
            }
        }
    }
    
    Ok(())
}