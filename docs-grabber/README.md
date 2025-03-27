# Anthropic Model Context Protocol (MCP) Server

An open-source implementation of the Anthropic Model Context Protocol that allows Claude to reference external documentation when developing scripts and code.

## Overview

This project provides a lightweight server that implements the Anthropic Model Context Protocol (MCP) specification. It enables Claude to access and reference documentation from various sources including PDFs, websites, and text files during conversations.

Key features:
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

### Adding Documents

#### PDF Documents

Upload a PDF document:
```bash
curl -X POST -F "file=@path/to/your/document.pdf" http://localhost:8000/documents/pdf
```

#### Web Pages

Add a web page:
```bash
curl -X POST "http://localhost:8000/documents/web?url=https://docs.anthropic.com/"
```

#### Text Documents

Add a text document:
```bash
curl -X POST -H "Content-Type: application/json" -d '{
    "source": "my-document",
    "title": "My Document",
    "content": "This is the content of my document.",
    "metadata": {"author": "Your Name"}
}' http://localhost:8000/documents/text
```

### Retrieving Context

Get context for a query:
```bash
curl -X POST -H "Content-Type: application/json" -d '{
    "query": "How does the Model Context Protocol work?",
    "max_tokens": 2000
}' http://localhost:8000/context
```

### Using with Claude

The server provides an MCP-compliant endpoint at `/mcp/v1/context` that Claude can use to retrieve relevant context during conversations.

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
