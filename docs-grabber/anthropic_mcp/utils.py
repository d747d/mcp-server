"""
Utility functions for MCP Server.
Includes security checks, input sanitization, and helper functions.
"""

import logging
import re
import json
import os
from typing import Dict, Any

# Setup logging
logger = logging.getLogger("anthropic-mcp-server")

def sanitize_input(text: str) -> str:
    """Sanitize input to prevent injection attacks"""
    try:
        # Remove any potentially harmful control characters
        text = re.sub(r"[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]", "", text)
        
        # Remove any script tags (for web content)
        text = re.sub(r"<script.*?>.*?</script>", "", text, flags=re.DOTALL)
        
        return text
    except Exception as e:
        logger.error(f"Error sanitizing input: {e}")
        return ""  # Return empty string on error

def is_safe_url(url: str) -> bool:
    """Check if a URL is potentially safe to scrape"""
    try:
        # Basic check for proper URL format
        if not re.match(r"^https?://.+", url):
            return False
            
        # Check for localhost and private IPs
        local_patterns = [
            r"^https?://localhost",
            r"^https?://127\.",
            r"^https?://10\.",
            r"^https?://172\.(1[6-9]|2[0-9]|3[0-1])\.",
            r"^https?://192\.168\."
        ]
        
        for pattern in local_patterns:
            if re.match(pattern, url):
                return False
                
        return True
    except Exception as e:
        logger.error(f"Error checking URL safety: {e}")
        return False  # Default to unsafe

def save_json(data: Dict[str, Any], file_path: str) -> None:
    """Save data as JSON to a file"""
    try:
        os.makedirs(os.path.dirname(file_path), exist_ok=True)
        with open(file_path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
    except Exception as e:
        logger.error(f"Error saving JSON to {file_path}: {e}")
        raise

def load_json(file_path: str) -> Dict[str, Any]:
    """Load JSON data from a file"""
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except FileNotFoundError:
        logger.warning(f"File not found: {file_path}")
        return {}
    except json.JSONDecodeError:
        logger.error(f"Invalid JSON in file: {file_path}")
        return {}
    except Exception as e:
        logger.error(f"Error loading JSON from {file_path}: {e}")
        raise
