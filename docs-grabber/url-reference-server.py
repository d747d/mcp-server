#!/usr/bin/env python3
import asyncio
import os
import re
import sys
import hashlib
import tempfile
import mimetypes
import json
import time
from datetime import datetime
from typing import Dict, List, Optional, Union, Any
from urllib.parse import urlparse
import httpx
from mcp.server.fastmcp import FastMCP
from mcp.types import Resource, TextContent

# Initialize FastMCP server
mcp = FastMCP("url-reference-server")

# Configuration
MAX_FILE_SIZE = 10 * 1024 * 1024  # 10MB
ALLOWED_CONTENT_TYPES = [
    "text/html", "text/plain", "application/json", 
    "application/pdf", "text/markdown",
    "application/xml", "text/xml", "text/csv",
]
DOWNLOAD_DIR = os.path.join(tempfile.gettempdir(), "mcp_url_references")
os.makedirs(DOWNLOAD_DIR, exist_ok=True)

# Create a README file as a default resource
README_PATH = os.path.join(DOWNLOAD_DIR, "README.txt")
with open(README_PATH, "w") as f:
    f.write("""URL Reference Server
    
This server allows you to download URLs and reference them in your conversation.
Use the add_reference tool to download a URL.

Available tools:
- add_reference(url): Download a URL and add it as a reference
- list_references(): List all downloaded references
- get_reference_content(url): Get the content of a reference
- remove_reference(url): Remove a reference
- clear_references(): Clear all references
""")

# In-memory cache for URL metadata
url_cache: Dict[str, Dict[str, Any]] = {
    "README": {
        "url": "README",
        "content_type": "text/plain",
        "size": os.path.getsize(README_PATH),
        "filename": "README.txt",
        "filepath": README_PATH,
        "text_content": open(README_PATH, "r").read(),
        "timestamp": time.time(),
        "added_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    }
}

print(f"Initialized with README resource: {README_PATH}", file=sys.stderr)

# Enhanced to be more robust with better validation
def validate_url(url: str) -> bool:
    """Validate if a URL is safe and supported with enhanced validation."""
    try:
        parsed = urlparse(url)
        
        # Basic scheme validation
        if parsed.scheme not in ["http", "https"]:
            return False
        
        # Basic hostname validation
        if not parsed.netloc or parsed.netloc.startswith("localhost") or any(ip in parsed.netloc for ip in ["127.0.0.1", "0.0.0.0", "::1"]):
            return False
        
        # Check for valid domain format
        if not re.match(r'^([a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$', parsed.netloc):
            # Allow IP addresses that aren't localhost/internal
            if not re.match(r'^(\d{1,3}\.){3}\d{1,3}$', parsed.netloc):
                return False
        
        return True
    except Exception:
        return False

# Function to safely get filename from URL
def get_safe_filename(url: str) -> str:
    """Generate a safe filename from URL."""
    url_hash = hashlib.md5(url.encode()).hexdigest()
    parsed = urlparse(url)
    path = parsed.path
    
    # Extract the last part of the path if exists
    filename = os.path.basename(path) if path else "index.html"
    
    # Remove unsafe characters
    filename = re.sub(r'[^a-zA-Z0-9._-]', '_', filename)
    
    # Ensure filename isn't too long and is unique
    if len(filename) > 50:
        filename = filename[:50]
    
    # Add hash to ensure uniqueness
    base, ext = os.path.splitext(filename)
    return f"{base}_{url_hash[:8]}{ext}"

# Improved content type detection
def detect_content_type(url: str, headers_content_type: str) -> str:
    """Detect content type from URL and headers with more reliable fallbacks."""
    # First use the content-type from headers
    if headers_content_type:
        main_type = headers_content_type.split(';')[0].strip().lower()
        if main_type:
            return main_type
    
    # Then try to guess from URL
    content_type, _ = mimetypes.guess_type(url)
    if content_type:
        return content_type
    
    # If we still don't have a content type, try to determine from the URL path
    parsed = urlparse(url)
    path = parsed.path.lower()
    
    if path.endswith('.html') or path.endswith('.htm'):
        return "text/html"
    elif path.endswith('.json'):
        return "application/json"
    elif path.endswith('.xml'):
        return "application/xml"
    elif path.endswith('.txt'):
        return "text/plain"
    elif path.endswith('.md'):
        return "text/markdown"
    elif path.endswith('.csv'):
        return "text/csv"
    elif path.endswith('.pdf'):
        return "application/pdf"
    
    # Default to text/plain
    return "text/plain"

# Improved download function with better error handling and retry logic
async def download_url(url: str, retries: int = 3) -> Dict[str, Any]:
    """Download content from URL with security checks and retry logic."""
    if url in url_cache:
        print(f"Using cached version of URL: {url}", file=sys.stderr)
        return url_cache[url]
    
    if not validate_url(url):
        raise ValueError(f"Invalid or unsafe URL: {url}")
    
    attempt = 0
    last_error = None
    
    while attempt < retries:
        attempt += 1
        try:
            print(f"Downloading URL: {url} (attempt {attempt}/{retries})", file=sys.stderr)
            async with httpx.AsyncClient(follow_redirects=True, timeout=30.0) as client:
                # Download the content
                print(f"Fetching content from URL: {url}", file=sys.stderr)
                response = await client.get(url)
                response.raise_for_status()
                
                # Check actual content type and size
                actual_content_type = detect_content_type(
                    url, 
                    response.headers.get("content-type", "")
                )
                print(f"GET result - Content type: {actual_content_type}, Size: {len(response.content)} bytes", file=sys.stderr)
                
                if actual_content_type not in ALLOWED_CONTENT_TYPES and not actual_content_type.startswith("text/"):
                    raise ValueError(f"Unsupported content type: {actual_content_type}")
                
                if len(response.content) > MAX_FILE_SIZE:
                    raise ValueError(f"URL content too large: {len(response.content)} bytes")
                
                # Save to file
                filename = get_safe_filename(url)
                filepath = os.path.join(DOWNLOAD_DIR, filename)
                
                print(f"Saving to file: {filepath}", file=sys.stderr)
                with open(filepath, "wb") as f:
                    f.write(response.content)
                
                # Extract text content for text-based formats
                text_content = None
                if actual_content_type.startswith("text/") or actual_content_type in ["application/json", "application/xml"]:
                    try:
                        text_content = response.text
                        print(f"Extracted {len(text_content)} characters of text content", file=sys.stderr)
                    except Exception as e:
                        print(f"Failed to extract text content: {str(e)}", file=sys.stderr)
                        text_content = "Unable to extract text content"
                
                # Create metadata
                current_time = datetime.now()
                metadata = {
                    "url": url,
                    "content_type": actual_content_type,
                    "size": len(response.content),
                    "filename": filename,
                    "filepath": filepath,
                    "text_content": text_content,
                    "timestamp": time.time(),
                    "added_at": current_time.strftime("%Y-%m-%d %H:%M:%S")
                }
                
                # Update cache
                url_cache[url] = metadata
                print(f"Successfully cached URL: {url}", file=sys.stderr)
                return metadata
                
        except httpx.HTTPStatusError as e:
            last_error = f"HTTP error: {e.response.status_code}"
            print(f"HTTP error for URL {url}: {e.response.status_code}", file=sys.stderr)
            
            # Don't retry for client errors (4xx) except for 429 (too many requests)
            if 400 <= e.response.status_code < 500 and e.response.status_code != 429:
                break
                
        except httpx.RequestError as e:
            last_error = f"Request error: {str(e)}"
            print(f"Request error for URL {url}: {str(e)}", file=sys.stderr)
        
        except Exception as e:
            last_error = f"Error: {str(e)}"
            print(f"Failed to download URL {url}: {str(e)}", file=sys.stderr)
        
        # Wait before retrying, with exponential backoff
        if attempt < retries:
            await asyncio.sleep(2 ** attempt)  # 2, 4, 8 seconds
    
    # If we get here, all retries failed
    raise ValueError(f"Failed to download URL after {retries} attempts: {last_error}")

@mcp.tool()
async def add_reference(url: str) -> str:
    """Download a URL and add it as a reference.
    
    Args:
        url: The URL to download and reference
    """
    try:
        print(f"Tool called: add_reference({url})", file=sys.stderr)
        metadata = await download_url(url)
        return (
            f"Successfully added reference: {url}\n"
            f"Saved as: {metadata['filename']}\n"
            f"Size: {metadata['size']} bytes\n"
            f"Type: {metadata['content_type']}\n"
            f"Added at: {metadata['added_at']}"
        )
    except ValueError as e:
        print(f"Value error in add_reference: {str(e)}", file=sys.stderr)
        return f"Failed to add reference: {str(e)}"
    except Exception as e:
        print(f"Unexpected error in add_reference: {str(e)}", file=sys.stderr)
        return f"Unexpected error: {str(e)}"

@mcp.tool()
async def list_references() -> str:
    """List all currently downloaded references."""
    print(f"Tool called: list_references()", file=sys.stderr)
    if not url_cache:
        return "No references have been added yet."
    
    result = "Available References:\n\n"
    for url, metadata in url_cache.items():
        result += (
            f"- {url}\n"
            f"  File: {metadata['filename']}\n"
            f"  Type: {metadata['content_type']}\n"
            f"  Size: {metadata['size']} bytes\n"
            f"  Added: {metadata['added_at']}\n\n"
        )
    
    return result

@mcp.tool()
async def get_reference_content(url: str) -> str:
    """Get the content of a specific reference.
    
    Args:
        url: The URL of the reference to get content for
    """
    print(f"Tool called: get_reference_content({url})", file=sys.stderr)
    if url not in url_cache:
        return f"Reference not found: {url}"
    
    metadata = url_cache[url]
    
    # For text content, return directly
    if metadata.get("text_content"):
        # Limit the text size to a reasonable amount
        text = metadata["text_content"]
        if len(text) > 10000:
            text = text[:10000] + "... [content truncated]"
        return f"Content of {url}:\n\n{text}"
    
    # For binary content, just return metadata
    return (
        f"Binary content at {url}\n"
        f"Type: {metadata['content_type']}\n"
        f"Size: {metadata['size']} bytes\n"
        f"Cannot display binary content directly."
    )

@mcp.tool()
async def remove_reference(url: str) -> str:
    """Remove a specific reference.
    
    Args:
        url: The URL of the reference to remove
    """
    print(f"Tool called: remove_reference({url})", file=sys.stderr)
    if url not in url_cache:
        return f"Reference not found: {url}"
    
    metadata = url_cache[url]
    
    # Remove from cache
    del url_cache[url]
    
    # Delete file
    try:
        os.remove(metadata["filepath"])
        print(f"Deleted file: {metadata['filepath']}", file=sys.stderr)
    except Exception as e:
        print(f"Failed to delete file: {str(e)}", file=sys.stderr)
        return f"Removed reference from cache but failed to delete file: {str(e)}"
    
    return f"Successfully removed reference: {url}"

@mcp.tool()
async def clear_references() -> str:
    """Clear all downloaded references."""
    print(f"Tool called: clear_references()", file=sys.stderr)
    count = len(url_cache)
    url_cache.clear()
    
    # Also delete files
    for filename in os.listdir(DOWNLOAD_DIR):
        try:
            full_path = os.path.join(DOWNLOAD_DIR, filename)
            if os.path.isfile(full_path):
                os.remove(full_path)
                print(f"Deleted file: {filename}", file=sys.stderr)
        except Exception as e:
            print(f"Failed to delete file {filename}: {str(e)}", file=sys.stderr)
    
    # Re-add README
    with open(README_PATH, "w") as f:
        f.write("""URL Reference Server
        
This server allows you to download URLs and reference them in your conversation.
Use the add_reference tool to download a URL.

Available tools:
- add_reference(url): Download a URL and add it as a reference
- list_references(): List all downloaded references
- get_reference_content(url): Get the content of a reference
- remove_reference(url): Remove a reference
- clear_references(): Clear all references
""")
    
    # Add README to cache
    url_cache["README"] = {
        "url": "README",
        "content_type": "text/plain",
        "size": os.path.getsize(README_PATH),
        "filename": "README.txt",
        "filepath": README_PATH,
        "text_content": open(README_PATH, "r").read(),
        "timestamp": time.time(),
        "added_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    }
    
    return f"All references ({count}) have been cleared."

# Optimized resource handler
@mcp.resource("reference://{filename}")
async def handle_resources(filename=None):
    """Handle resources - both listing and reading."""
    if filename is None:
        # List resources
        print(f"Resource list requested - cache has {len(url_cache)} items", file=sys.stderr)
        resources = []
        for url, metadata in url_cache.items():
            resource_name = os.path.basename(metadata["filename"])
            resources.append(Resource(
                uri=f"reference://{metadata['filename']}",
                name=resource_name,
                description=f"Downloaded from {url} at {metadata['added_at']}",
                mimeType=metadata["content_type"]
            ))
        print(f"Returning {len(resources)} resources: {[r.name for r in resources]}", file=sys.stderr)
        return resources
    else:
        # Read specific resource
        print(f"Resource read requested: reference://{filename}", file=sys.stderr)
        
        # Find the metadata by filename
        metadata = None
        for _, meta in url_cache.items():
            if meta["filename"] == filename:
                metadata = meta
                break
        
        if not metadata:
            print(f"ERROR: Resource not found: {filename}", file=sys.stderr)
            raise ValueError(f"Reference not found: {filename}")
        
        # For text content, return directly
        if metadata.get("text_content"):
            print(f"Returning text content for {filename}", file=sys.stderr)
            return metadata["text_content"]
        
        # For binary content, read from file
        try:
            print(f"Reading binary content from {metadata['filepath']}", file=sys.stderr)
            with open(metadata["filepath"], "rb") as f:
                return f.read()
        except Exception as e:
            print(f"ERROR: Failed to read file: {str(e)}", file=sys.stderr)
            raise ValueError(f"Failed to read reference: {str(e)}")

# Main entry point
if __name__ == "__main__":
    # Print server information
    print("Starting URL Reference MCP Server", file=sys.stderr)
    print(f"Download directory: {DOWNLOAD_DIR}", file=sys.stderr)
    print(f"Initial resources: {list(url_cache.keys())}", file=sys.stderr)
    print("Server ready to accept connections", file=sys.stderr)
    
    # Run the server with specified transport
    mcp.run(transport='stdio')