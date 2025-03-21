use std::sync::Arc;
use governor::{Quota, RateLimiter as Governor};
use governor::clock::{DefaultClock, ReasonablyRealtime};
use std::num::NonZeroU32;
use url::Url;

use crate::config::types::{RateLimitSettings, SecuritySettings};
use crate::protocol::error::McpError;
use crate::protocol::types::{CallToolRequest, ReadResourceRequest};

// Rate limiter implementation using Governor crate
pub struct RateLimiter {
    settings: RateLimitSettings,
    governor: Option<Arc<Governor<String, DefaultClock>>>,
}

impl RateLimiter {
    pub fn new(settings: &RateLimitSettings) -> Self {
        let governor = if settings.enabled {
            let quota = Quota::per_minute(NonZeroU32::new(settings.requests_per_minute).unwrap())
                .allow_burst(NonZeroU32::new(settings.burst).unwrap());
            
            Some(Arc::new(Governor::keyed(quota)))
        } else {
            None
        };

        Self {
            settings: settings.clone(),
            governor,
        }
    }

    pub fn check(&self) -> Result<(), McpError> {
        if let Some(governor) = &self.governor {
            // Use a constant key for now - in a real implementation you might use client IP or token
            let key = "default".to_string();
            
            match governor.check_key(&key) {
                Ok(_) => Ok(()),
                Err(_) => Err(McpError::RateLimitExceeded),
            }
        } else {
            Ok(())
        }
    }
}

// Security validator for Docker operations
pub struct SecurityValidator {
    settings: SecuritySettings,
}

impl SecurityValidator {
    pub fn new(settings: &SecuritySettings) -> Self {
        Self {
            settings: settings.clone(),
        }
    }

    pub fn validate_tool(&self, request: &CallToolRequest) -> Result<(), McpError> {
        // Check if command is allowed
        if let Some(allowed) = &self.settings.commands.allowed_commands {
            if !allowed.contains(&request.name) {
                return Err(McpError::OperationNotPermitted(format!(
                    "Tool '{}' is not in the allowed list",
                    request.name
                )));
            }
        } else if self.settings.commands.denied_commands.contains(&request.name) {
            return Err(McpError::OperationNotPermitted(format!(
                "Tool '{}' is in the denied list",
                request.name
            )));
        }

        // Additional validation for specific tools
        match request.name.as_str() {
            "compose-up" | "compose-down" => {
                if let Some(project_dir) = request.arguments.get("project_directory").and_then(|v| v.as_str()) {
                    // Check if project directory is allowed
                    if let Some(allowed_projects) = &self.settings.networks.allowed_networks {
                        if !allowed_projects.contains(project_dir) {
                            return Err(McpError::OperationNotPermitted(format!(
                                "Project directory '{}' is not in the allowed list",
                                project_dir
                            )));
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn validate_resource(&self, request: &ReadResourceRequest) -> Result<(), McpError> {
        // Validate container resources
        if request.uri.starts_with("docker://container/") {
            // Nothing to validate for now
        }
        // Validate image resources
        else if request.uri.starts_with("docker://image/") {
            let image_id = request.uri.replace("docker://image/", "");
            
            // Check if image name contains a registry that's denied
            for denied in &self.settings.registries.denied_registries {
                if image_id.starts_with(&format!("{}/", denied)) {
                    return Err(McpError::OperationNotPermitted(format!(
                        "Image from registry '{}' is not allowed",
                        denied
                    )));
                }
            }
            
            // Check if it's in the denied base images list
            for denied in &self.settings.registries.denied_base_images {
                if image_id == *denied {
                    return Err(McpError::OperationNotPermitted(format!(
                        "Base image '{}' is not allowed",
                        denied
                    )));
                }
            }
        }
        // Validate compose resources
        else if request.uri.starts_with("docker://compose/") {
            let project_dir = request.uri.replace("docker://compose/", "");
            
            // Check if project directory is allowed
            if let Some(allowed_projects) = &self.settings.networks.allowed_networks {
                if !allowed_projects.contains(&project_dir) {
                    return Err(McpError::OperationNotPermitted(format!(
                        "Project directory '{}' is not in the allowed list",
                        project_dir
                    )));
                }
            }
        }

        Ok(())
    }
}