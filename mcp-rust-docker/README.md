# Docker MCP Server

A Model Context Protocol (MCP) server that provides Docker and Docker Compose operations to Claude and other MCP clients. This server allows LLMs to help users build, manage, and deploy Docker Compose applications, with strong security controls and resource limitations.

## Features

- **Docker Operations**:
  - List, start, stop and restart containers
  - View container logs
  - List images
  - Build images from Dockerfiles
  - Execute Docker Compose commands

- **Resources**:
  - Docker system information
  - Container details
  - Image details
  - Docker Compose project status

- **Prompts**:
  - Generate optimized Dockerfiles
  - Create Docker Compose configurations
  - Troubleshoot Docker issues

- **Security**:
  - Comprehensive configuration system
  - Rate limiting
  - Resource quotas
  - Access control for Docker commands
  - Registry and image restrictions
  - Network and volume mount restrictions

## Installation

### Prerequisites

- Rust 1.65 or higher
- Docker Engine
- Docker Compose V2

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/docker-mcp-server.git
cd docker-mcp-server

# Build the project
cargo build --release

# Run the server with default configuration
./target/release/docker-mcp-server

# Run with custom configuration
./target/release/docker-mcp-server --config /path/to/config.yaml


Using with Claude for Desktop
To use this server with Claude for Desktop:

Build the server using the instructions above
Edit your Claude for Desktop configuration at:

macOS: ~/Library/Application Support/Claude/claude_desktop_config.json
Windows: %APPDATA%\Claude\claude_desktop_config.json


Add the Docker MCP Server to your configuration:

json

Security Considerations

Docker Access: This server requires access to the Docker socket, which provides significant control over your system. Configure security settings appropriately.
Read-Only Mode: For safer operation, consider enabling read-only mode to prevent modifications to containers/images.
Container Privileges: Be cautious when allowing container operations - they could affect other services on your system.
Resource Limits: Configure appropriate resource quotas to prevent abuse.



Run the server:

./target/release/docker-mcp-server