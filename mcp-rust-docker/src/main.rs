mod config;
mod docker;
mod protocol;
mod security;
mod server;
mod transport;

use log::{info, error, warn, debug, trace};

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
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .multiple_occurrences(true)
                .help("Increases logging verbosity each use for more debug output"),
        )
        .arg(
            clap::Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppresses all output except errors"),
        )
        .get_matches();

    // Set up logging with better default configuration
    let log_level = if matches.is_present("quiet") {
        log::LevelFilter::Error
    } else {
        match matches.occurrences_of("verbose") {
            0 => log::LevelFilter::Info,
            1 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    };

    env_logger::Builder::new()
        .format_timestamp_millis()
        .format_module_path(true)
        .filter_level(log_level)
        .init();

    // Log startup banner
    info!("Starting Docker MCP Server {}", env!("CARGO_PKG_VERSION"));
    debug!("Log level set to: {:?}", log_level);

    // Load configuration
    let config_path = matches.value_of("config").map(std::path::PathBuf::from);
    info!("Loading configuration{}", if config_path.is_some() {
        format!(" from {:?}", config_path.as_ref().unwrap())
    } else {
        " from default locations".to_string()
    });
    
    let config = match crate::config::loader::load_config(config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            error!("Make sure a valid configuration file exists");
            error!("You can specify a custom path with --config");
            return Err(e);
        }
    };

    // Log startup information
    info!(
        "Configuration loaded - Server: {} ({})",
        config.server.name, config.server.version
    );
    info!("Docker host: {}", config.docker.host);
    info!("Read-only mode: {}", if config.docker.read_only { "ENABLED" } else { "DISABLED" });

    // Create and initialize server
    info!("Initializing server...");
    let server = match crate::server::McpServer::new(&config) {
        Ok(server) => server,
        Err(e) => {
            error!("Failed to initialize server: {}", e);
            return Err(anyhow::anyhow!("Server initialization failed: {}", e));
        }
    };
    
    match server.initialize().await {
        Ok(_) => info!("Server initialized successfully"),
        Err(e) => {
            error!("Failed to initialize server: {}", e);
            return Err(anyhow::anyhow!("Server initialization failed: {}", e));
        }
    }

    // Set up transport based on configuration
    match server.get_transport_type() {
        crate::config::types::TransportType::Stdio => {
            info!("Using stdio transport for JSON-RPC communication");
            let mut transport = crate::transport::stdio::StdioTransport::new(server);
            
            match transport.run().await {
                Ok(_) => info!("Transport completed normally"),
                Err(e) => {
                    error!("Transport error: {}", e);
                    return Err(anyhow::anyhow!("Transport error: {}", e));
                }
            }
        }
        crate::config::types::TransportType::Sse => {
            error!("SSE transport not implemented yet");
            return Err(anyhow::anyhow!("SSE transport not implemented yet"));
        }
    }

    info!("Server shutting down");
    Ok(())
}