#!/usr/bin/env python3
"""
Basic usage example for the Anthropic MCP Server.
This script demonstrates how to:
1. Start the server
2. Add documents
3. Retrieve context for a query
"""

import requests
import json
import time
import subprocess
import sys
import os
from pathlib import Path

# Configuration
SERVER_URL = "http://localhost:8000"
PDF_PATH = Path("path/to/your/document.pdf")  # Replace with your PDF file path
WEB_URL = "https://docs.anthropic.com/"  # Example URL

def start_server():
    """Start the MCP server in a subprocess"""
    print("Starting Anthropic MCP Server...")
    
    # Use Python executable from current environment
    python_exe = sys.executable
    
    # Start the server as a subprocess
    server_process = subprocess.Popen(
        [python_exe, "-m", "anthropic_mcp.server"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    # Wait for server to start
    time.sleep(2)
    
    # Check if server is running
    try:
        response = requests.get(f"{SERVER_URL}/health")
        if response.status_code == 200:
            print("Server is running!")
        else:
            print(f"Server returned status code: {response.status_code}")
            server_process.terminate()
            sys.exit(1)
    except requests.exceptions.ConnectionError:
        print("Failed to connect to server")
        server_process.terminate()
        sys.exit(1)
        
    return server_process

def add_pdf_document(pdf_path):
    """Add a PDF document to the server"""
    if not os.path.exists(pdf_path):
        print(f"PDF file not found: {pdf_path}")
        return None
        
    print(f"Adding PDF document: {pdf_path}")
    
    with open(pdf_path, "rb") as f:
        files = {"file": (os.path.basename(pdf_path), f, "application/pdf")}
        response = requests.post(f"{SERVER_URL}/documents/pdf", files=files)
        
    if response.status_code == 200:
        result = response.json()
        print(f"Added document with ID: {result['source']}")
        return result
    else:
        print(f"Failed to add PDF: {response.status_code} - {response.text}")
        return None

def add_web_document(url):
    """Add a web page document to the server"""
    print(f"Adding web page: {url}")
    
    response = requests.post(
        f"{SERVER_URL}/documents/web",
        params={"url": url}
    )
    
    if response.status_code == 200:
        result = response.json()
        print(f"Added document with ID: {result['source']}")
        return result
    else:
        print(f"Failed to add web page: {response.status_code} - {response.text}")
        return None

def get_context(query):
    """Get relevant context for a query"""
    print(f"Getting context for query: {query}")
    
    response = requests.post(
        f"{SERVER_URL}/context",
        json={
            "query": query,
            "max_tokens": 2000
        }
    )
    
    if response.status_code == 200:
        result = response.json()
        print(f"Retrieved context from {len(result['source_documents'])} sources")
        print(f"Context token count: {result['token_count']}")
        print("\nRetrieved context:")
        print(result['context'][:500] + "..." if len(result['context']) > 500 else result['context'])
        return result
    else:
        print(f"Failed to get context: {response.status_code} - {response.text}")
        return None

def list_documents():
    """List all documents in the server"""
    print("Listing documents...")
    
    response = requests.get(f"{SERVER_URL}/documents")
    
    if response.status_code == 200:
        documents = response.json()
        print(f"Found {len(documents)} documents:")
        for doc_id in documents:
            print(f"  - {doc_id}")
        return documents
    else:
        print(f"Failed to list documents: {response.status_code} - {response.text}")
        return None

def main():
    """Main function"""
    # Start the server
    server_process = start_server()
    
    try:
        # Add documents
        if PDF_PATH.exists():
            add_pdf_document(PDF_PATH)
        else:
            print(f"PDF file not found: {PDF_PATH}")
            
        add_web_document(WEB_URL)
        
        # Wait for embeddings to be generated
        print("Waiting for embeddings to be generated...")
        time.sleep(5)
        
        # List documents
        list_documents()
        
        # Get context
        get_context("How does the Model Context Protocol work?")
        
    finally:
        # Stop the server
        print("Stopping server...")
        server_process.terminate()
        server_process.wait()
        print("Server stopped")

if __name__ == "__main__":
    main()
