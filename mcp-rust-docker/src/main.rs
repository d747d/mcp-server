mod config;
mod docker;
mod protocol;
mod security;
mod server;
mod transport;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up command line argument parsing
    let matches = clap::Command::new("Docker MCP Server")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Your Name <your.email@example.com>")
        .about("Model Context Protocol server for Docker operations")
        .arg(
            clap::Arg::new("config")
                .short('c')
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
    let config_path = matches.value_of("config").map(std::path::PathBuf::from);
    let config = crate::config::loader::load_config(config_path)?;

    // Log startup information
    log::info!(
        "Starting Docker MCP Server {} ({})",
        config.server.name, config.server.version
    );

    // Create and initialize server
    let server = crate::server::McpServer::new(config);
    server.initialize().await?;

    // Set up transport based on configuration
    match server.get_transport_type() {
        crate::config::types::TransportType::Stdio => {
            log::info!("Using stdio transport");
            let mut transport = crate::transport::stdio::StdioTransport::new(server);
            transport.run().await?;
        }
        crate::config::types::TransportType::Sse => {
            log::info!("SSE transport not implemented yet");
            // TODO: Implement SSE transport
        }
    }

    log::info!("Server shutting down");
    Ok(())
}