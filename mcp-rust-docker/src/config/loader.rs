use anyhow::{Context, Result};
use config::{Config, Environment, File};
use std::path::Path;

use crate::config::types::ServerConfig;

pub fn load_config<P: AsRef<Path>>(path: Option<P>) -> Result<ServerConfig> {
    let mut builder = Config::builder();

    // Start with default settings
    builder = builder.add_source(File::from_str(
        include_str!("../config/default.yaml"),
        config::FileFormat::Yaml,
    ));

    // Add config file if specified
    if let Some(config_path) = path {
        builder = builder.add_source(
            File::from(config_path.as_ref())
                .required(true)
                .format(config::FileFormat::Yaml),
        );
    }

    // Add environment variables with prefix DOCKER_MCP_
    builder = builder.add_source(File::from_str(
        include_str!("../config/default.yaml"),
        config::FileFormat::Yaml,
    ));

    // Build and deserialize the config
    let config = builder
        .build()
        .context("Failed to build configuration")?
        .try_deserialize::<ServerConfig>()
        .context("Failed to deserialize configuration")?;

    Ok(config)
}