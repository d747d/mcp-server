# Compact Project Structure

```
anthropic-mcp-server/
├── README.md
├── LICENSE
├── requirements.txt
├── main.py                # Entry point
├── anthropic_mcp/
│   ├── __init__.py
│   ├── server.py          # Main server with FastAPI endpoints
│   ├── ingestion.py       # All document ingestion (PDF, web, file)
│   ├── processing.py      # Text chunking and cleaning
│   ├── indexing.py        # Vector embeddings and retrieval
│   ├── claude.py          # Claude integration and context formatting
│   └── utils.py           # Utility functions
└── examples/
    └── basic_usage.py     # Example usage
```
