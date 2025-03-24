use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::types::ServerConfig;
use crate::docker::{DockerClient, DockerClientImpl};
use crate::protocol::error::McpError;
use crate::protocol::types::{
    CallToolRequest, GetPromptRequest, GetPromptResult, JsonRpcId, JsonRpcRequest,
    JsonRpcResponse, ListPromptsResult, ListResourcesResult, ListToolsResult, Prompt, ReadResourceRequest,
    ReadResourceResult, Resource, ResourceContent, ServerCapabilities, ServerInfo, Tool,
};
use crate::security::{RateLimiter, SecurityValidator};
use crate::logging::ErrorLogger;

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
    // Add this method to improve error logging
    fn log_request(&self, request: &JsonRpcRequest, response: &JsonRpcResponse) {
        let id = match &request.id {
            JsonRpcId::Null => "null".to_string(),
            JsonRpcId::String(s) => s.clone(),
            JsonRpcId::Number(n) => n.to_string(),
        };
        
        let success = response.error.is_none();
        let error_code = response.error.as_ref().map(|e| e.code);
        let error_message = response.error.as_ref().map(|e| e.message.as_str());
        
        ErrorLogger::log_request_end(&id, &request.method, success, error_code, error_message);
    }
    
    // Modify your existing process_request method to add logging
    pub async fn process_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Log request start
        let id_str = match &request.id {
            JsonRpcId::Null => "null".to_string(),
            JsonRpcId::String(s) => s.clone(),
            JsonRpcId::Number(n) => n.to_string(),
        };
        
        ErrorLogger::log_request_start(&id_str, &request.method);
        
        // Apply rate limiting
        if let Err(e) = self.rate_limiter.check() {
            let response = self.error_response(request.id, e);
            self.log_request(&request, &response);
            return response;
        }

        // Your existing match block for request.method.as_str()...
        let response = match request.method.as_str() {
            // Your existing handlers...
            _ => self.error_response(
                request.id,
                McpError::MethodNotFound(format!("Method '{}' not found", request.method)),
            ),
        };
        
        // Log request completion
        self.log_request(&request, &response);
        
        response
    }

    pub fn get_transport_type(&self) -> &crate::config::types::TransportType {
        &self.config.server.transport
    }
        // Add getter for request timeout
    pub fn get_request_timeout(&self) -> std::time::Duration {
        self.config.server.request_timeout
    }
    
    // Add a diagnostic tool to help with debugging
    async fn register_diagnostic_tool(&self, tools: &mut std::collections::HashMap<String, crate::protocol::types::Tool>) {
        tools.insert(
            "diagnostic".to_string(),
            crate::protocol::types::Tool {
                name: "diagnostic".to_string(),
                description: Some("Run diagnostic checks on the Docker MCP server".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "check_docker": {
                            "type": "boolean",
                            "description": "Check Docker connectivity"
                        },
                        "check_compose": {
                            "type": "boolean",
                            "description": "Check Docker Compose availability"
                        },
                        "list_env_vars": {
                            "type": "boolean",
                            "description": "List relevant environment variables"
                        }
                    }
                }),
            },
        );
    }

    pub async fn initialize(&self) -> Result<(), crate::protocol::error::McpError> {
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

        // Register the diagnostic tool
        self.register_diagnostic_tool(&mut tools).await;

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

    async fn handle_call_tool(&self, id: crate::protocol::types::JsonRpcId, request: crate::protocol::types::CallToolRequest) -> crate::protocol::types::JsonRpcResponse {        // Check security restrictions
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
            "diagnostic" => self.run_diagnostic(request.arguments).await,
            _ => Err(crate::protocol::error::McpError::ToolNotFound(request.name)),
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

    // Implementation of the diagnostic tool
    async fn run_diagnostic(&self, args: serde_json::Value) -> Result<crate::protocol::types::CallToolResult, crate::protocol::error::McpError> {
        let check_docker = args.get("check_docker").and_then(|v| v.as_bool()).unwrap_or(true);
        let check_compose = args.get("check_compose").and_then(|v| v.as_bool()).unwrap_or(true);
        let list_env_vars = args.get("list_env_vars").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let mut results = Vec::new();
        
        results.push("=== Docker MCP Server Diagnostics ===".to_string());
        results.push(format!("Server name: {}", self.config.server.name));
        results.push(format!("Server version: {}", self.config.server.version));
        results.push(format!("Transport type: {:?}", self.config.server.transport));
        results.push(format!("Request timeout: {:?}", self.config.server.request_timeout));
        results.push(format!("Docker host: {}", self.config.docker.host));
        results.push(format!("Read-only mode: {}", self.config.docker.read_only));
        
        if check_docker {
            results.push("\n=== Docker Connectivity ===".to_string());
            match self.docker_client.get_docker_version().await {
                Ok(version) => {
                    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&version);
                    match parsed {
                        Ok(v) => {
                            if let Some(api_version) = v.get("ApiVersion").and_then(|v| v.as_str()) {
                                results.push(format!("Docker API version: {}", api_version));
                            }
                            if let Some(engine_version) = v.get("Version").and_then(|v| v.as_str()) {
                                results.push(format!("Docker Engine version: {}", engine_version));
                            }
                            results.push("Docker connection: OK".to_string());
                        },
                        Err(_) => {
                            results.push(format!("Docker connection: OK (raw data: {})", version));
                        }
                    }
                },
                Err(e) => {
                    results.push(format!("Docker connection: FAILED - {}", e));
                    results.push("Possible causes:".to_string());
                    results.push(" - Docker daemon not running".to_string());
                    results.push(" - Incorrect Docker host configuration".to_string());
                    results.push(" - Permission issues with Docker socket".to_string());
                    
                    if self.config.docker.host.starts_with("unix://") {
                        // Check if the Docker socket exists
                        let socket_path = self.config.docker.host.trim_start_matches("unix://");
                        if let Ok(metadata) = std::fs::metadata(socket_path) {
                            results.push(format!("Docker socket exists: {}", socket_path));
                            
                            // Check if it's a socket
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::FileTypeExt;
                                if metadata.file_type().is_socket() {
                                    results.push("File is a valid socket: YES".to_string());
                                } else {
                                    results.push("File is a valid socket: NO".to_string());
                                }
                            }
                        } else {
                            results.push(format!("Docker socket not found at: {}", socket_path));
                        }
                    }
                }
            }
        }
        
        if check_compose {
            results.push("\n=== Docker Compose ===".to_string());
            
            let compose_path = &self.config.docker.compose_path;
            results.push(format!("Docker Compose path: {:?}", compose_path));
            
            // Check if the compose binary exists
            if compose_path.exists() {
                results.push("Docker Compose binary exists: YES".to_string());
                
                // Try to run docker-compose version
                let output = tokio::process::Command::new(compose_path)
                    .arg("version")
                    .output()
                    .await;
                
                match output {
                    Ok(output) => {
                        if output.status.success() {
                            let version = String::from_utf8_lossy(&output.stdout);
                            results.push(format!("Docker Compose version: {}", version.trim()));
                            results.push("Docker Compose command: OK".to_string());
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            results.push(format!("Docker Compose command failed: {}", stderr.trim()));
                        }
                    },
                    Err(e) => {
                        results.push(format!("Docker Compose command error: {}", e));
                    }
                }
            } else {
                results.push("Docker Compose binary exists: NO".to_string());
                results.push("Possible causes:".to_string());
                results.push(" - Docker Compose not installed".to_string());
                results.push(" - Incorrect path in configuration".to_string());
                results.push(format!(" - Current working directory: {:?}", std::env::current_dir().ok()));
            }
        }
        
        if list_env_vars {
            results.push("\n=== Environment Variables ===".to_string());
            for (key, value) in std::env::vars() {
                if key.starts_with("DOCKER_") || key.contains("MCP") || key.contains("RUST") {
                    results.push(format!("{}={}", key, value));
                }
            }
        }
        
        let result_text = results.join("\n");
        
        Ok(crate::protocol::types::CallToolResult {
            content: vec![crate::protocol::types::Content::Text(crate::protocol::types::TextContent {
                r#type: "text".to_string(),
                text: result_text,
            })],
            is_error: false,
        })
    }

    fn error_response(&self, id: JsonRpcId, error: McpError) -> JsonRpcResponse {
        let error_json = error.to_json_rpc_error();
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(crate::protocol::types::JsonRpcError {
                code: error_json.code,
                message: error_json.message,
                data: error_json.data,
            }),
        }
    }
}