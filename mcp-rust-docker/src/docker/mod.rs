use bollard::container::{ListContainersOptions, LogsOptions, StartContainerOptions, StopContainerOptions};
use bollard::image::ListImagesOptions;
use bollard::Docker;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

use crate::config::types::DockerSettings;
use crate::protocol::error::McpError;
use crate::protocol::types::{CallToolResult, Content, TextContent};
use futures::stream::TryStreamExt;


pub trait DockerClient {
    // Container operations
    async fn list_containers(&self, args: Value) -> Result<CallToolResult, McpError>;
    async fn container_start(&self, args: Value) -> Result<CallToolResult, McpError>;
    async fn container_stop(&self, args: Value) -> Result<CallToolResult, McpError>;
    async fn container_logs(&self, args: Value) -> Result<CallToolResult, McpError>;
    
    // Image operations
    async fn list_images(&self, args: Value) -> Result<CallToolResult, McpError>;
    
    // Compose operations
    async fn compose_up(&self, args: Value) -> Result<CallToolResult, McpError>;
    async fn compose_down(&self, args: Value) -> Result<CallToolResult, McpError>;
    async fn validate_compose(&self, args: Value) -> Result<CallToolResult, McpError>;
    
    // Resource operations
    async fn get_docker_info(&self) -> Result<String, McpError>;
    async fn get_docker_version(&self) -> Result<String, McpError>;
    async fn get_container_details(&self, container_id: &str) -> Result<String, McpError>;
    async fn get_image_details(&self, image_id: &str) -> Result<String, McpError>;
    async fn get_compose_status(&self, project_directory: &str) -> Result<String, McpError>;
}

pub struct DockerClientImpl {
    client: Docker,
    settings: DockerSettings,
}

impl DockerClientImpl {
    // Add getter for compose path
    pub fn get_compose_path(&self) -> &std::path::Path {
        &self.settings.compose_path
    }
    // Enhance the Docker client connection handling
    pub fn new(settings: &DockerSettings) -> Result<Self, McpError> {
        let client = match settings.host.as_str() {
            host if host.starts_with("unix://") => {
                match Docker::connect_with_unix_defaults() {
                    Ok(client) => client,
                    Err(e) => return Err(McpError::DockerError(format!(
                        "Failed to connect to Docker daemon at {}: {}", host, e
                    ))),
                }
            }
            host if host.starts_with("npipe://") => {
                match Docker::connect_with_local_defaults() {
                    Ok(client) => client,
                    Err(e) => return Err(McpError::DockerError(format!(
                        "Failed to connect to Docker daemon at {}: {}", host, e
                    ))),
                }
            }
            host => {
                match Docker::connect_with_http_defaults() {
                    Ok(client) => client,
                    Err(e) => return Err(McpError::DockerError(format!(
                        "Failed to connect to Docker daemon at {}: {}", host, e
                    ))),
                }
            },
        };
    
        Ok(Self {
            client,
            settings: settings.clone(),
        })
    }

    fn is_read_only_operation(&self, operation: &str) -> bool {
        match operation {
            "list_containers" | "container_logs" | "list_images" |
            "get_docker_info" | "get_docker_version" | "get_container_details" |
            "get_image_details" | "get_compose_status" | "validate_compose" => true,
            _ => false,
        }
    }

    fn check_read_only(&self, operation: &str) -> Result<(), McpError> {
        if self.settings.read_only && !self.is_read_only_operation(operation) {
            return Err(McpError::OperationNotPermitted(
                "Server is in read-only mode".to_string(),
            ));
        }
        Ok(())
    }
}

// Improve Docker operation with timeouts
impl DockerClient for DockerClientImpl {
    async fn list_containers(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("list_containers")?;

        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(0);
        let filter = args.get("filter").and_then(|v| v.as_str());

        let mut options = ListContainersOptions::<String>::default();
        options.all = all;
        if limit > 0 {
            options.limit = Some(limit as isize);
        }
        
        if let Some(filter_str) = filter {
            let mut filters = HashMap::new();
            // Parse filter string like "status=running"
            let parts: Vec<&str> = filter_str.split('=').collect();
            if parts.len() == 2 {
                filters.insert(parts[0].to_string(), vec![parts[1].to_string()]);
                options.filters = filters;
            }
        }

        // Add timeout to Docker API call
        match tokio::time::timeout(
            self.settings.operation_timeout,
            self.client.list_containers(Some(options))
        ).await {
            Ok(result) => {
                match result {
                    Ok(containers) => {
                        let json_result = serde_json::to_string_pretty(&containers)?;
                        
                        Ok(CallToolResult {
                            content: vec![Content::Text(TextContent {
                                r#type: "text".to_string(),
                                text: json_result,
                            })],
                            is_error: false,
                        })
                    },
                    Err(e) => Err(McpError::DockerError(format!("Failed to list containers: {}", e))),
                }
            },
            Err(_) => Err(McpError::OperationTimeout),
        }
    }

    async fn container_start(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("container_start")?;

        let container_id = args
            .get("container_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing container_id parameter".to_string()))?;

        let options = StartContainerOptions::<String>::default();
        self.client.start_container(container_id, Some(options)).await?;

        Ok(CallToolResult {
            content: vec![Content::Text(TextContent {
                r#type: "text".to_string(),
                text: format!("Container {} started successfully", container_id),
            })],
            is_error: false,
        })
    }

    async fn container_stop(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("container_stop")?;

        let container_id = args
            .get("container_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing container_id parameter".to_string()))?;

        let timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(10);
        
        let options = StopContainerOptions {
            t: timeout as i64,
        };

        self.client.stop_container(container_id, Some(options)).await?;

        Ok(CallToolResult {
            content: vec![Content::Text(TextContent {
                r#type: "text".to_string(),
                text: format!("Container {} stopped successfully", container_id),
            })],
            is_error: false,
        })
    }

    async fn container_logs(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("container_logs")?;

        let container_id = args
            .get("container_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing container_id parameter".to_string()))?;

        let tail = args.get("tail").and_then(|v| v.as_str()).unwrap_or("all");
        let since = args.get("since").and_then(|v| v.as_str());

        let mut options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        if tail != "all" {
            options.tail = tail.to_string();
        }

        if let Some(since_str) = since {
            // Handle relative time (e.g., "42m" for 42 minutes)
            if since_str.ends_with('m') {
                if let Ok(minutes) = since_str.trim_end_matches('m').parse::<i64>() {
                    let since_timestamp = chrono::Utc::now() - chrono::Duration::minutes(minutes);
                    options.since = since_timestamp.timestamp();
                }
            } else if since_str.ends_with('h') {
                if let Ok(hours) = since_str.trim_end_matches('h').parse::<i64>() {
                    let since_timestamp = chrono::Utc::now() - chrono::Duration::hours(hours);
                    options.since = since_timestamp.timestamp();
                }
            } else if let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(since_str) {
                options.since = timestamp.timestamp();
            }
        }

        let max_log_size = self.settings.max_log_size;
        
        // Use timeout for logs collection
        match tokio::time::timeout(
            self.settings.operation_timeout,
            self.client.logs(container_id, Some(options)).try_collect::<Vec<_>>()
        ).await {
            Ok(result) => {
                match result {
                    Ok(logs) => {
                        let mut log_text = String::new();
                        for log in logs {
                            match log {
                                bollard::container::LogOutput::StdOut { message } => {
                                    if let Ok(text) = String::from_utf8(message.to_vec()) {
                                        log_text.push_str(&format!("[STDOUT] {}\n", text));
                                    }
                                }
                                bollard::container::LogOutput::StdErr { message } => {
                                    if let Ok(text) = String::from_utf8(message.to_vec()) {
                                        log_text.push_str(&format!("[STDERR] {}\n", text));
                                    }
                                }
                                _ => {}
                            }
                            
                            // Check if we've exceeded the maximum log size
                            if log_text.len() > max_log_size {
                                log_text.truncate(max_log_size);
                                log_text.push_str("\n... (log truncated due to size limit)");
                                break;
                            }
                        }

                        Ok(CallToolResult {
                            content: vec![Content::Text(TextContent {
                                r#type: "text".to_string(),
                                text: log_text,
                            })],
                            is_error: false,
                        })
                    },
                    Err(e) => Err(McpError::DockerError(format!("Failed to get container logs: {}", e))),
                }
            },
            Err(_) => Err(McpError::OperationTimeout),
        }
    }

    async fn list_images(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("list_images")?;

        let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
        let filter = args.get("filter").and_then(|v| v.as_str());

        let mut options = ListImagesOptions::<String>::default();
        options.all = all;
        
        if let Some(filter_str) = filter {
            let mut filters = HashMap::new();
            // Parse filter string like "reference=alpine"
            let parts: Vec<&str> = filter_str.split('=').collect();
            if parts.len() == 2 {
                filters.insert(parts[0].to_string(), vec![parts[1].to_string()]);
                options.filters = filters;
            }
        }

        let images = self.client.list_images(Some(options)).await?;
        
        let json_result = serde_json::to_string_pretty(&images)?;
        
        Ok(CallToolResult {
            content: vec![Content::Text(TextContent {
                r#type: "text".to_string(),
                text: json_result,
            })],
            is_error: false,
        })
    }

    async fn compose_up(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("compose_up")?;

        let project_directory = args
            .get("project_directory")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing project_directory parameter".to_string()))?;

        // Security check for project directory
        if let Some(allowed_projects) = &self.settings.allowed_compose_projects {
            if !allowed_projects.contains(project_directory) {
                return Err(McpError::OperationNotPermitted(format!(
                    "Project directory '{}' is not in the allowed list",
                    project_directory
                )));
            }
        }

        let detach = args.get("detach").and_then(|v| v.as_bool()).unwrap_or(true);
        let services: Vec<String> = args
            .get("services")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut command = Command::new(&self.settings.compose_path);
        command.current_dir(project_directory);
        command.arg("up");
        
        if detach {
            command.arg("-d");
        }
        
        for service in services {
            command.arg(&service);
        }

        let output = tokio::process::Command::from(command)
            .output()
            .await
            .map_err(|e| McpError::DockerError(format!("Failed to execute docker-compose: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{}", stdout));
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("STDERR:\n{}", stderr));
        }

        if output.status.success() {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose up successful for {}:\n{}", project_directory, result),
                })],
                is_error: false,
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose up failed for {}:\n{}", project_directory, result),
                })],
                is_error: true,
            })
        }
    }

    async fn compose_down(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("compose_down")?;

        let project_directory = args
            .get("project_directory")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing project_directory parameter".to_string()))?;

        // Security check for project directory
        if let Some(allowed_projects) = &self.settings.allowed_compose_projects {
            if !allowed_projects.contains(project_directory) {
                return Err(McpError::OperationNotPermitted(format!(
                    "Project directory '{}' is not in the allowed list",
                    project_directory
                )));
            }
        }

        let volumes = args.get("volumes").and_then(|v| v.as_bool()).unwrap_or(false);
        let remove_images = args.get("remove_images").and_then(|v| v.as_str());

        let mut command = Command::new(&self.settings.compose_path);
        command.current_dir(project_directory);
        command.arg("down");
        
        if volumes {
            command.arg("-v");
        }
        
        if let Some(images) = remove_images {
            match images {
                "all" => {
                    command.arg("--rmi").arg("all");
                }
                "local" => {
                    command.arg("--rmi").arg("local");
                }
                _ => {}
            }
        }

        let output = tokio::process::Command::from(command)
            .output()
            .await
            .map_err(|e| McpError::DockerError(format!("Failed to execute docker-compose: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{}", stdout));
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("STDERR:\n{}", stderr));
        }

        if output.status.success() {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose down successful for {}:\n{}", project_directory, result),
                })],
                is_error: false,
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose down failed for {}:\n{}", project_directory, result),
                })],
                is_error: true,
            })
        }
    }

    async fn validate_compose(&self, args: Value) -> Result<CallToolResult, McpError> {
        self.check_read_only("validate_compose")?;

        let compose_content = args
            .get("compose_content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::InvalidParams("Missing compose_content parameter".to_string()))?;

        // Create a temporary file with the compose content
        let temp_dir = tempfile::tempdir()
            .map_err(|e| McpError::InternalError(format!("Failed to create temporary directory: {}", e)))?;
        
        let temp_file_path = temp_dir.path().join("docker-compose.yml");
        
        tokio::fs::write(&temp_file_path, compose_content)
            .await
            .map_err(|e| McpError::InternalError(format!("Failed to write temporary file: {}", e)))?;

        let mut command = Command::new(&self.settings.compose_path);
        command.current_dir(temp_dir.path());
        command.arg("config");

        let output = tokio::process::Command::from(command)
            .output()
            .await
            .map_err(|e| McpError::DockerError(format!("Failed to execute docker-compose: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&format!("STDOUT:\n{}", stdout));
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("STDERR:\n{}", stderr));
        }

        if output.status.success() {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose configuration is valid.\n{}", result),
                })],
                is_error: false,
            })
        } else {
            Ok(CallToolResult {
                content: vec![Content::Text(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Docker Compose configuration is invalid.\n{}", result),
                })],
                is_error: true,
            })
        }
    }

    async fn get_docker_info(&self) -> Result<String, McpError> {
        self.check_read_only("get_docker_info")?;

        let info = self.client.info().await?;
        Ok(serde_json::to_string_pretty(&info)?)
    }

    async fn get_docker_version(&self) -> Result<String, McpError> {
        self.check_read_only("get_docker_version")?;

        let version = self.client.version().await?;
        Ok(serde_json::to_string_pretty(&version)?)
    }

    async fn get_container_details(&self, container_id: &str) -> Result<String, McpError> {
        self.check_read_only("get_container_details")?;

        let details = self.client.inspect_container(container_id, None).await?;
        Ok(serde_json::to_string_pretty(&details)?)
    }

    async fn get_image_details(&self, image_id: &str) -> Result<String, McpError> {
        self.check_read_only("get_image_details")?;

        let details = self.client.inspect_image(image_id).await?;
        Ok(serde_json::to_string_pretty(&details)?)
    }

    async fn get_compose_status(&self, project_directory: &str) -> Result<String, McpError> {
        self.check_read_only("get_compose_status")?;

        // Security check for project directory
        if let Some(allowed_projects) = &self.settings.allowed_compose_projects {
            if !allowed_projects.contains(project_directory) {
                return Err(McpError::OperationNotPermitted(format!(
                    "Project directory '{}' is not in the allowed list",
                    project_directory
                )));
            }
        }

        let mut command = Command::new(&self.settings.compose_path);
        command.current_dir(project_directory);
        command.arg("ps");
        command.arg("--format").arg("json");

        let output = tokio::process::Command::from(command)
            .output()
            .await
            .map_err(|e| McpError::DockerError(format!("Failed to execute docker-compose: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Err(McpError::DockerError(format!("Failed to get compose status: {}", stderr)))
        }
    }
}