#!/usr/bin/env python3
"""
Anthropic Model Context Protocol (MCP) Server
A lightweight server that implements the Anthropic MCP specification
for providing external context to Claude models.
"""

import argparse
import logging
import uvicorn

def main():
    parser = argparse.ArgumentParser(
        description="Anthropic Model Context Protocol (MCP) Server"
    )
    parser.add_argument(
        "--host", 
        type=str, 
        default="127.0.0.1", 
        help="Host to bind the server to (default: 127.0.0.1)"
    )
    parser.add_argument(
        "--port", 
        type=int, 
        default=8000, 
        help="Port to bind the server to (default: 8000)"
    )
    parser.add_argument(
        "--log-level", 
        type=str, 
        default="info", 
        choices=["debug", "info", "warning", "error", "critical"],
        help="Logging level (default: info)"
    )
    parser.add_argument(
        "--data-dir", 
        type=str, 
        default=None, 
        help="Directory to store documents and embeddings (default: ~/.anthropic_mcp)"
    )

    args = parser.parse_args()

    # Configure logging
    log_level = getattr(logging, args.log_level.upper())
    logging.basicConfig(
        level=log_level,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    )

    # Set data directory if provided
    if args.data_dir:
        import os
        os.environ["ANTHROPIC_MCP_DATA_DIR"] = args.data_dir

    # Start the server
    uvicorn.run(
        "anthropic_mcp.server:app",
        host=args.host,
        port=args.port,
        log_level=args.log_level
    )

if __name__ == "__main__":
    main()
