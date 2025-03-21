use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::types::ServerConfig;
use crate::docker::{DockerClient, DockerClientImpl};
use crate::protocol::error::McpError;
use crate::protocol::types::{
    CallToolRequest, CallToolResult, GetPromptRequest, GetPromptResult, JsonRpcId, JsonRpcRequest,
    JsonRpcResponse, ListPromptsResult, ListResourcesResult, ListToolsResult, Prompt, ReadResourceRequest,
    ReadResourceResult, Resource, ResourceContent, ServerCapabilities, ServerInfo, Tool,
};
use crate::security::{RateLimiter, SecurityValidator};

pub struct McpServer {
    config: ServerConfig,
    docker_client: Arc<DockerClientImpl>,
    tools: Arc<RwLock<HashMap<String, Tool>>>,
    resources: Arc<RwLock<HashMap<String, Resource>>>,
    prompts: Arc<RwLock<HashMap<String, Prompt>>>,
    security_validator: Arc<SecurityValidator>,
    rate_limiter: Arc<RateLimiter>,
}

impl McpServer {
    pub fn new(config: ServerConfig) -> Self {
        let docker_client = Arc::new(DockerClientImpl::new(&config.docker));
        let security_validator = Arc::new(SecurityValidator::new(&config.security));
        let rate_limiter = Arc::new(RateLimiter::new(&config.security.rate_limiting));

        Self {
            config,
            docker_client,
            tools: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
            prompts: Arc::new(RwLock::new(HashMap::new())),
            security_validator,
            rate_limiter,
        }
    }

    pub async fn initialize(&self) -> Result<(), McpError> {
        // Register tools
        let mut tools = self.tools.write().await;
        
        // Container tools
        tools.insert(
            "list-containers".to_string(),
            Tool {
                name: "list-containers".to_string(),
                description: Some("List running Docker containers".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "all": {
                            "type": "boolean",
                            "description": "Show all containers (default shows just running)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Number of containers to show"
                        },
                        "filter": {
                            "type": "string",
                            "description": "Filter output based on conditions provided"
                        }
                    }
                }),
            },
        );

        tools.insert(
            "container-start".to_string(),
            Tool {
                name: "container-start".to_string(),
                description: Some("Start one or more stopped containers".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["container_id"],
                    "properties": {
                        "container_id": {
                            "type": "string",
                            "description": "Container ID or name to start"
                        }
                    }
                }),
            },
        );

        tools.insert(
            "container-stop".to_string(),
            Tool {
                name: "container-stop".to_string(),
                description: Some("Stop one or more running containers".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["container_id"],
                    "properties": {
                        "container_id": {
                            "type": "string",
                            "description": "Container ID or name to stop"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Seconds to wait for stop before killing it (default 10)"
                        }
                    }
                }),
            },
        );

        tools.insert(
            "container-logs".to_string(),
            Tool {
                name: "container-logs".to_string(),
                description: Some("Fetch the logs of a container".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["container_id"],
                    "properties": {
                        "container_id": {
                            "type": "string",
                            "description": "Container ID or name to get logs from"
                        },
                        "tail": {
                            "type": "string",
                            "description": "Number of lines to show from the end of the logs (default 'all')"
                        },
                        "since": {
                            "type": "string",
                            "description": "Show logs since timestamp (e.g., '2013-01-02T13:23:37Z') or relative (e.g., '42m' for 42 minutes)"
                        }
                    }
                }),
            },
        );

        // Image tools
        tools.insert(
            "list-images".to_string(),
            Tool {
                name: "list-images".to_string(),
                description: Some("List Docker images".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "all": {
                            "type": "boolean",
                            "description": "Show all images (default hides intermediate images)"
                        },
                        "filter": {
                            "type": "string",
                            "description": "Filter output based on conditions provided"
                        }
                    }
                }),
            },
        );

        // Docker Compose tools
        tools.insert(
            "compose-up".to_string(),
            Tool {
                name: "compose-up".to_string(),
                description: Some("Create and start containers defined in a Docker Compose file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["project_directory"],
                    "properties": {
                        "project_directory": {
                            "type": "string",
                            "description": "Directory containing docker-compose.yml file"
                        },
                        "detach": {
                            "type": "boolean",
                            "description": "Detached mode: Run containers in the background"
                        },
                        "services": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Specific services to start (default: all services)"
                        }
                    }
                }),
            },
        );

        tools.insert(
            "compose-down".to_string(),
            Tool {
                name: "compose-down".to_string(),
                description: Some("Stop and remove containers, networks, images, and volumes defined in a Docker Compose file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["project_directory"],
                    "properties": {
                        "project_directory": {
                            "type": "string",
                            "description": "Directory containing docker-compose.yml file"
                        },
                        "volumes": {
                            "type": "boolean",
                            "description": "Remove named volumes declared in the volumes section of the Compose file"
                        },
                        "remove_images": {
                            "type": "string",
                            "enum": ["all", "local"],
                            "description": "Remove images, 'all': remove all images, 'local': remove only images without a tag"
                        }
                    }
                }),
            },
        );

        tools.insert(
            "validate-compose".to_string(),
            Tool {
                name: "validate-compose".to_string(),
                description: Some("Validate a Docker Compose file".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "required": ["compose_content"],
                    "properties": {
                        "compose_content": {
                            "type": "string",
                            "description": "Content of the docker-compose.yml file to validate"
                        }
                    }
                }),
            },
        );

        // Register resources
        let mut resources = self.resources.write().await;

        resources.insert(
            "docker://info".to_string(),
            Resource {
                uri: "docker://info".to_string(),
                name: "Docker Info".to_string(),
                description: Some("Information about the Docker host system".to_string()),
                mime_type: Some("application/json".to_string()),
                text: None,
                blob: None,
            },
        );

        resources.insert(
            "docker://version".to_string(),
            Resource {
                uri: "docker://version".to_string(),
                name: "Docker Version".to_string(),
                description: Some("Docker version information".to_string()),
                mime_type: Some("application/json".to_string()),
                text: None,
                blob: None,
            },
        );

        // Register prompts
        let mut prompts = self.prompts.write().await;

        prompts.insert(
            "generate-dockerfile".to_string(),
            Prompt {
                name: "generate-dockerfile".to_string(),
                description: Some("Generate an optimized Dockerfile for a specific application type".to_string()),
                arguments: vec![
                    crate::protocol::types::PromptArgument {
                        name: "app_type".to_string(),
                        description: Some("Type of application (e.g., nodejs, python, go, rust)".to_string()),
                        required: true,
                    },
                    crate::protocol::types::PromptArgument {
                        name: "version".to_string(),
                        description: Some("Version of the application runtime".to_string()),
                        required: false,
                    },
                    crate::protocol::types::PromptArgument {
                        name: "production".to_string(),
                        description: Some("Whether this is for production use (yes/no)".to_string()),
                        required: false,
                    },
                ],
            },
        );

        prompts.insert(
            "generate-compose".to_string(),
            Prompt {
                name: "generate-compose".to_string(),
                description: Some("Generate a Docker Compose configuration for a specific scenario".to_string()),
                arguments: vec![
                    crate::protocol::types::PromptArgument {
                        name: "scenario".to_string(),
                        description: Some("Type of scenario (e.g., webapp, database, microservices)".to_string()),
                        required: true,
                    },
                    crate::protocol::types::PromptArgument {
                        name: "services".to_string(),
                        description: Some("Comma-separated list of services to include".to_string()),
                        required: true,
                    },
                    crate::protocol::types::PromptArgument {
                        name: "with_volumes".to_string(),
                        description: Some("Whether to include persistent volumes (yes/no)".to_string()),
                        required: false,
                    },
                ],
            },
        );

        Ok(())
    }

    pub async fn process_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Apply rate limiting
        if let Err(e) = self.rate_limiter.check() {
            return self.error_response(request.id, e);
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "tools/list" => self.handle_list_tools(request.id).await,
            "tools/call" => {
                let params = match request.params {
                    Some(params) => params,
                    None => return self.error_response(request.id, McpError::InvalidParams("Missing params".to_string())),
                };

                match serde_json::from_value::<CallToolRequest>(params) {
                    Ok(params) => self.handle_call_tool(request.id, params).await,
                    Err(e) => self.error_response(request.id, McpError::InvalidParams(e.to_string())),
                }
            }
            "resources/list" => self.handle_list_resources(request.id).await,
            "resources/read" => {
                let params = match request.params {
                    Some(params) => params,
                    None => return self.error_response(request.id, McpError::InvalidParams("Missing params".to_string())),
                };

                match serde_json::from_value::<ReadResourceRequest>(params) {
                    Ok(params) => self.handle_read_resource(request.id, params).await,
                    Err(e) => self.error_response(request.id, McpError::InvalidParams(e.to_string())),
                }
            }
            "prompts/list" => self.handle_list_prompts(request.id).await,
            "prompts/get" => {
                let params = match request.params {
                    Some(params) => params,
                    None => return self.error_response(request.id, McpError::InvalidParams("Missing params".to_string())),
                };

                match serde_json::from_value::<GetPromptRequest>(params) {
                    Ok(params) => self.handle_get_prompt(request.id, params).await,
                    Err(e) => self.error_response(request.id, McpError::InvalidParams(e.to_string())),
                }
            }
            _ => self.error_response(
                request.id,
                McpError::MethodNotFound(format!("Method '{}' not found", request.method)),
            ),
        }
    }

    async fn handle_initialize(&self, id: JsonRpcId) -> JsonRpcResponse {
        let server_info = ServerInfo {
            name: self.config.server.name.clone(),
            version: self.config.server.version.clone(),
        };

        let capabilities = ServerCapabilities {
            resources: Some(crate::protocol::types::ResourcesCapability {
                list_changed: true,
            }),
            tools: Some(crate::protocol::types::ToolsCapability {
                list_changed: true,
            }),
            prompts: Some(crate::protocol::types::PromptsCapability {
                list_changed: true,
            }),
        };

        let result = serde_json::json!({
            "server": server_info,
            "capabilities": capabilities,
        });

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    async fn handle_list_tools(&self, id: JsonRpcId) -> JsonRpcResponse {
        let tools = self.tools.read().await;
        let tools_list: Vec<Tool> = tools.values().cloned().collect();

        let result = ListToolsResult { tools: tools_list };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    async fn handle_call_tool(&self, id: JsonRpcId, request: CallToolRequest) -> JsonRpcResponse {
        // Check security restrictions
        if let Err(e) = self.security_validator.validate_tool(&request) {
            return self.error_response(id, e);
        }

        // Get the tool
        let tool_name = request.name.clone();
        let tools = self.tools.read().await;
        
        if !tools.contains_key(&tool_name) {
            return self.error_response(id, McpError::ToolNotFound(tool_name));
        }

        // Execute the tool
        let result = match tool_name.as_str() {
            "list-containers" => self.docker_client.list_containers(request.arguments).await,
            "container-start" => self.docker_client.container_start(request.arguments).await,
            "container-stop" => self.docker_client.container_stop(request.arguments).await,
            "container-logs" => self.docker_client.container_logs(request.arguments).await,
            "list-images" => self.docker_client.list_images(request.arguments).await,
            "compose-up" => self.docker_client.compose_up(request.arguments).await,
            "compose-down" => self.docker_client.compose_down(request.arguments).await,
            "validate-compose" => self.docker_client.validate_compose(request.arguments).await,
            _ => Err(McpError::ToolNotFound(tool_name)),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(result).unwrap()),
                error: None,
            },
            Err(e) => self.error_response(id, e),
        }
    }

    async fn handle_list_resources(&self, id: JsonRpcId) -> JsonRpcResponse {
        let resources = self.resources.read().await;
        let resources_list: Vec<Resource> = resources.values().cloned().collect();

        let result = ListResourcesResult {
            resources: resources_list,
            resource_templates: Some(vec![
                crate::protocol::types::ResourceTemplate {
                    uri_template: "docker://container/{container_id}".to_string(),
                    name: "Container Details".to_string(),
                    description: Some("Information about a specific container".to_string()),
                    mime_type: Some("application/json".to_string()),
                },
                crate::protocol::types::ResourceTemplate {
                    uri_template: "docker://image/{image_id}".to_string(),
                    name: "Image Details".to_string(),
                    description: Some("Information about a specific image".to_string()),
                    mime_type: Some("application/json".to_string()),
                },
                crate::protocol::types::ResourceTemplate {
                    uri_template: "docker://compose/{project_directory}".to_string(),
                    name: "Compose Project Status".to_string(),
                    description: Some("Status of a Docker Compose project".to_string()),
                    mime_type: Some("application/json".to_string()),
                },
            ]),
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    async fn handle_read_resource(&self, id: JsonRpcId, request: ReadResourceRequest) -> JsonRpcResponse {
        // Check security restrictions
        if let Err(e) = self.security_validator.validate_resource(&request) {
            return self.error_response(id, e);
        }

        // Check if it's a static resource
        let resources = self.resources.read().await;
        if let Some(resource) = resources.get(&request.uri) {
            // Fetch the resource content dynamically
            let content = match resource.uri.as_str() {
                "docker://info" => self.docker_client.get_docker_info().await,
                "docker://version" => self.docker_client.get_docker_version().await,
                _ => Err(McpError::ResourceNotFound(request.uri.clone())),
            };

            match content {
                Ok(text) => {
                    let content = ResourceContent {
                        uri: request.uri.clone(),
                        mime_type: resource.mime_type.clone(),
                        text: Some(text),
                        blob: None,
                    };
                    let result = ReadResourceResult {
                        contents: vec![content],
                    };
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::to_value(result).unwrap()),
                        error: None,
                    }
                }
                Err(e) => self.error_response(id, e),
            }
        } else {
            // Handle dynamic resources using URI templates
            if request.uri.starts_with("docker://container/") {
                let container_id = request.uri.replace("docker://container/", "");
                match self.docker_client.get_container_details(&container_id).await {
                    Ok(text) => {
                        let content = ResourceContent {
                            uri: request.uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: Some(text),
                            blob: None,
                        };
                        let result = ReadResourceResult {
                            contents: vec![content],
                        };
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(serde_json::to_value(result).unwrap()),
                            error: None,
                        }
                    }
                    Err(e) => self.error_response(id, e),
                }
            } else if request.uri.starts_with("docker://image/") {
                let image_id = request.uri.replace("docker://image/", "");
                match self.docker_client.get_image_details(&image_id).await {
                    Ok(text) => {
                        let content = ResourceContent {
                            uri: request.uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: Some(text),
                            blob: None,
                        };
                        let result = ReadResourceResult {
                            contents: vec![content],
                        };
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(serde_json::to_value(result).unwrap()),
                            error: None,
                        }
                    }
                    Err(e) => self.error_response(id, e),
                }
            } else if request.uri.starts_with("docker://compose/") {
                let project_dir = request.uri.replace("docker://compose/", "");
                match self.docker_client.get_compose_status(&project_dir).await {
                    Ok(text) => {
                        let content = ResourceContent {
                            uri: request.uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: Some(text),
                            blob: None,
                        };
                        let result = ReadResourceResult {
                            contents: vec![content],
                        };
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id,
                            result: Some(serde_json::to_value(result).unwrap()),
                            error: None,
                        }
                    }
                    Err(e) => self.error_response(id, e),
                }
            } else {
                self.error_response(id, McpError::ResourceNotFound(request.uri))
            }
        }
    }

    async fn handle_list_prompts(&self, id: JsonRpcId) -> JsonRpcResponse {
        let prompts = self.prompts.read().await;
        let prompts_list: Vec<Prompt> = prompts.values().cloned().collect();

        let result = ListPromptsResult { prompts: prompts_list };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        }
    }

    async fn handle_get_prompt(&self, id: JsonRpcId, request: GetPromptRequest) -> JsonRpcResponse {
        let prompts = self.prompts.read().await;
        
        if let Some(prompt) = prompts.get(&request.name) {
            // Validate required arguments are present
            if let Some(args) = &request.arguments {
                for arg in &prompt.arguments {
                    if arg.required && !args.contains_key(&arg.name) {
                        return self.error_response(
                            id,
                            McpError::InvalidParams(format!("Required argument '{}' is missing", arg.name)),
                        );
                    }
                }
            } else if prompt.arguments.iter().any(|arg| arg.required) {
                return self.error_response(
                    id,
                    McpError::InvalidParams("Required arguments are missing".to_string()),
                );
            }

            // Generate prompt messages based on the template type
            let result = match request.name.as_str() {
                "generate-dockerfile" => self.generate_dockerfile_prompt(request.arguments).await,
                "generate-compose" => self.generate_compose_prompt(request.arguments).await,
                _ => Err(McpError::PromptNotFound(request.name)),
            };

            match result {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::to_value(result).unwrap()),
                    error: None,
                },
                Err(e) => self.error_response(id, e),
            }
        } else {
            self.error_response(id, McpError::PromptNotFound(request.name))
        }
    }

    async fn generate_dockerfile_prompt(
        &self,
        args: Option<HashMap<String, String>>,
    ) -> Result<GetPromptResult, McpError> {
        let args = args.unwrap_or_default();
        let app_type = args
            .get("app_type")
            .ok_or_else(|| McpError::InvalidParams("Missing required argument 'app_type'".to_string()))?;
        
        let version = args.get("version").map(|s| s.as_str()).unwrap_or("latest");
        let production = args.get("production").map(|s| s.as_str()).unwrap_or("yes") == "yes";

        let mut prompt_text = format!(
            "Generate an optimized Dockerfile for a {} application",
            app_type
        );

        if version != "latest" {
            prompt_text.push_str(&format!(", using version {}", version));
        }

        if production {
            prompt_text.push_str(", optimized for production use.");
            prompt_text.push_str("\n\nThe Dockerfile should include:");
            prompt_text.push_str("\n- Multi-stage builds for smaller final image");
            prompt_text.push_str("\n- Proper security practices (non-root user, minimal permissions)");
            prompt_text.push_str("\n- Optimization for caching during builds");
            prompt_text.push_str("\n- Health checks and proper signal handling");
        } else {
            prompt_text.push_str(", configured for development.");
            prompt_text.push_str("\n\nThe Dockerfile should include:");
            prompt_text.push_str("\n- Fast rebuilds and good developer experience");
            prompt_text.push_str("\n- Volume mounting for code changes");
            prompt_text.push_str("\n- Debugging tools included");
        }

        prompt_text.push_str("\n\nPlease include comments explaining key decisions.");

        let messages = vec![crate::protocol::types::PromptMessage {
            role: "user".to_string(),
            content: crate::protocol::types::PromptContent {
                r#type: "text".to_string(),
                text: Some(prompt_text),
                resource: None,
            },
        }];

        Ok(GetPromptResult {
            description: Some(format!(
                "Optimized Dockerfile for {} {} application",
                if production { "production" } else { "development" },
                app_type
            )),
            messages,
        })
    }

    async fn generate_compose_prompt(
        &self,
        args: Option<HashMap<String, String>>,
    ) -> Result<GetPromptResult, McpError> {
        let args = args.unwrap_or_default();
        let scenario = args
            .get("scenario")
            .ok_or_else(|| McpError::InvalidParams("Missing required argument 'scenario'".to_string()))?;
        
        let services = args
            .get("services")
            .ok_or_else(|| McpError::InvalidParams("Missing required argument 'services'".to_string()))?;
        
        let with_volumes = args.get("with_volumes").map(|s| s.as_str()).unwrap_or("yes") == "yes";

        let mut prompt_text = format!(
            "Generate a Docker Compose configuration for a {} scenario",
            scenario
        );

        prompt_text.push_str(&format!(" that includes the following services: {}.", services));

        if with_volumes {
            prompt_text.push_str("\n\nInclude persistent volumes for data that should be preserved across container restarts.");
        }

        prompt_text.push_str("\n\nThe configuration should include:");
        prompt_text.push_str("\n- Proper networking between services");
        prompt_text.push_str("\n- Environment variables for configuration");
        prompt_text.push_str("\n- Health checks where appropriate");
        prompt_text.push_str("\n- Restart policies for reliability");
        prompt_text.push_str("\n\nPlease include comments explaining the purpose of each service and any important configuration details.");

        let messages = vec![crate::protocol::types::PromptMessage {
            role: "user".to_string(),
            content: crate::protocol::types::PromptContent {
                r#type: "text".to_string(),
                text: Some(prompt_text),
                resource: None,
            },
        }];

        Ok(GetPromptResult {
            description: Some(format!(
                "Docker Compose configuration for {} scenario with services: {}",
                scenario, services
            )),
            messages,
        })
    }

    fn error_response(&self, id: JsonRpcId, error: McpError) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error.to_json_rpc_error()),
        }
    }
}