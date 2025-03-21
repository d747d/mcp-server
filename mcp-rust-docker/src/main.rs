mod config;
mod docker;
mod protocol;
mod security;
mod server;
mod transport;

use clap::{App, Arg};
use std::path::PathBuf;
use log::{info, error};

use crate::config::loader::load_config;
use crate::server::McpServer;
use crate::transport::stdio::StdioTransport;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up command line argument parsing
    let matches = App::new("Docker MCP Server")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Your Name <your.email@example.com>")
        .about("Model Context Protocol server for Docker operations")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .help("Path to configuration file")
                .takes_value(true),
        )
        .get_matches();

    // Set up logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // Load configuration
    let config_path = matches.value_of("config").map(PathBuf::from);
    let config = match load_config(config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(Box::new(e));
        }
    };

    // Log startup information
    info!(
        "Starting Docker MCP Server {} ({})",
        config.server.name, config.server.version
    );

    // Create and initialize server
    let server = McpServer::new(config);
    server.initialize().await?;

    // Set up transport based on configuration
    match server.config.server.transport {
        config::types::TransportType::Stdio => {
            info!("Using stdio transport");
            let mut transport = StdioTransport::new(server);
            transport.run().await?;
        }
        config::types::TransportType::Sse => {
            info!("SSE transport not implemented yet");
            // TODO: Implement SSE transport
        }
    }

    info!("Server shutting down");
    Ok(())
}