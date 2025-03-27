# Anthropic Model Context Protocol (MCP) Server

An open-source implementation of the Anthropic Model Context Protocol that allows Claude to reference external documentation when developing scripts and code.

## Overview

This project provides a lightweight server that implements the Anthropic Model Context Protocol (MCP) specification. It enables Claude to access and reference documentation from various sources including PDFs, websites, and text files during conversations.

Key features:
- Control directly from Claude chat with simple commands
- PDF document ingestion and processing
- Web page scraping and content extraction
- Text chunking and embedding for semantic search
- Local storage of documents and embeddings
- MCP-compliant API endpoints for Claude integration
- No authentication requirements for ease of use

## Installation

### Requirements
- Python 3.8 or higher
- pip (Python package manager)

### Setup

1. Clone the repository:
```bash
git clone https://github.com/your-username/anthropic-mcp-server.git
cd anthropic-mcp-server
```

2. Install dependencies:
```bash
pip install -r requirements.txt
```

## Usage

### Starting the Server

Run the server with default settings:
```bash
python -m main
```

This will start the server at http://127.0.0.1:8000

### Command Line Options

```bash
python -m main --host 0.0.0.0 --port 8080 --log-level debug
```

Available options:
- `--host`: Host to bind the server to (default: 127.0.0.1)
- `--port`: Port to bind the server to (default: 8000)
- `--log-level`: Logging level (choices: debug, info, warning, error, critical; default: info)
- `--data-dir`: Directory to store documents and embeddings (default: ~/.anthropic_mcp)

## Connecting to Claude Desktop

1. Start the MCP server
2. Open Claude Desktop
3. Go to Settings → Advanced → Model Context Protocol
4. Add a new MCP server with URL: `http://localhost:8000/mcp/v1/context` (no authentication needed)
5. Start a new conversation with Claude and enable the MCP server

## Controlling via Claude Chat

You can manage your documentation directly in the Claude chat interface using commands:

| Command | Description |
|---------|-------------|
| `/mcp help` | Show available commands |
| `/mcp add url https://example.com` | Add a webpage as documentation |
| `/mcp list docs` | List all documents |
| `/mcp doc info [doc_id]` | Get document details |
| `/mcp delete doc [doc_id]` | Delete a document |
| `/mcp status` | Show server status |

### Examples:

```
/mcp add url https://docs.python.org/3/tutorial/
/mcp list docs
/mcp doc info web_docs.python.org_3_tutorial_
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Server information |
| `/health` | GET | Health check |
| `/documents/pdf` | POST | Upload a PDF document |
| `/documents/web` | POST | Add a web page |
| `/documents/text` | POST | Add a text document |
| `/documents` | GET | List all documents |
| `/documents/{doc_id}` | GET | Get a specific document |
| `/documents/{doc_id}` | DELETE | Delete a document |
| `/context` | POST | Get relevant context for a query |
| `/mcp/v1/context` | POST | MCP-compliant context endpoint |

## Security Considerations

- The server is designed for local use and does not include authentication
- Web scraping is restricted to public URLs (no localhost or private IP addresses)
- Input sanitization is applied to prevent injection attacks
- Document processing is done locally with no data sent to external services

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Disclaimer

This is an unofficial implementation of the Anthropic Model Context Protocol specification. It is not affiliated with or endorsed by Anthropic.