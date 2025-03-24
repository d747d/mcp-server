use anyhow::Result;
use config::{Config, File, Environment};
use std::path::Path;
use log::{info, error};

use crate::config::types::ServerConfig;

pub fn load_config<P: AsRef<Path>>(path: Option<P>) -> Result<ServerConfig> {
    info!("Loading configuration");

    let mut builder = Config::builder();
    let mut config_sources = Vec::<String>::new();

    // Start with embedded default settings to ensure we always have a baseline config
    builder = builder.add_source(File::from_str(
        include_str!("../config/default.yaml"),
        config::FileFormat::Yaml,
    ));
    config_sources.push("embedded default config".to_string());

    // Try to load from default config file locations
    let default_locations = vec![
        "config/default.yaml",
        "/etc/docker-mcp-server/config.yaml",
        "./config.yaml",
    ];

    for location in default_locations {
        let path = std::path::Path::new(location);
        if path.exists() {
            info!("Found config file at: {}", location);
            builder = builder.add_source(
                File::from(path).required(false).format(config::FileFormat::Yaml)
            );
            config_sources.push(location.to_string());
        }
    }

    // Add config file if specified
    if let Some(config_path) = path {
        let config_path = config_path.as_ref();
        if config_path.exists() {
            info!("Using specified config file: {:?}", config_path);
            builder = builder.add_source(
                File::from(config_path)
                    .required(true)
                    .format(config::FileFormat::Yaml),
            );
            config_sources.push(config_path.to_string_lossy().to_string());
        } else {
            let err_msg = format!("Specified config file not found: {:?}", config_path);
            error!("{}", err_msg);
            return Err(anyhow::anyhow!(err_msg));
        }
    }

    // Add environment variables with prefix DOCKER_MCP_
    builder = builder.add_source(
        Environment::with_prefix("DOCKER_MCP")
            .separator("_")
            .try_parsing(true)
    );
    config_sources.push("environment variables (DOCKER_MCP_*)".to_string());

    info!("Configuration sources (in priority order): {:?}", config_sources);
    
    // Build and deserialize the config
    match builder.build() {
        Ok(config) => {
            match config.try_deserialize::<ServerConfig>() {
                Ok(config) => {
                    info!("Successfully loaded configuration");
                    Ok(config)
                },
                Err(e) => {
                    let err_msg = format!("Failed to deserialize configuration: {}", e);
                    error!("{}", err_msg);
                    Err(anyhow::anyhow!(err_msg))
                }
            }
        },
        Err(e) => {
            let err_msg = format!("Failed to build configuration: {}", e);
            error!("{}", err_msg);
            Err(anyhow::anyhow!(err_msg))
        }
    }
}