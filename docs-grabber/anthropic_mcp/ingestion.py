"""
Document ingestion functions for MCP Server.
Handles extracting text from various sources including PDFs and web pages.
"""

import logging
import os
from typing import Tuple, Dict, Any
import requests
from bs4 import BeautifulSoup
import pypdf

# Setup logging
logger = logging.getLogger("anthropic-mcp-server")

def extract_pdf_text(pdf_path: str) -> str:
    """Extract text content from a PDF file"""
    try:
        text = ""
        with open(pdf_path, "rb") as f:
            pdf_reader = pypdf.PdfReader(f)
            for page_num in range(len(pdf_reader.pages)):
                page = pdf_reader.pages[page_num]
                text += page.extract_text() + "\n\n"
        return text
    except Exception as e:
        logger.error(f"Error extracting text from PDF: {e}")
        raise

def scrape_webpage(url: str) -> Tuple[str, str]:
    """Scrape text content from a webpage"""
    try:
        headers = {
            "User-Agent": "AnthropicMCPServer/0.1 (Open Source Project)"
        }
        response = requests.get(url, headers=headers, timeout=10)
        response.raise_for_status()
        
        soup = BeautifulSoup(response.text, "html.parser")
        
        # Get page title
        title = soup.title.string if soup.title else url
        
        # Remove script and style elements
        for script in soup(["script", "style"]):
            script.extract()
        
        # Get text content
        text = soup.get_text(separator="\n")
        
        # Clean up whitespace
        lines = (line.strip() for line in text.splitlines())
        text = "\n".join(line for line in lines if line)
        
        return text, title
    except Exception as e:
        logger.error(f"Error scraping webpage {url}: {e}")
        raise

def read_text_file(file_path: str) -> str:
    """Read text from a file"""
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            return f.read()
    except Exception as e:
        logger.error(f"Error reading text file: {e}")
        raise

def save_uploaded_file(file_content: bytes, file_name: str, upload_dir: str = "/tmp") -> str:
    """Save an uploaded file to disk"""
    try:
        os.makedirs(upload_dir, exist_ok=True)
        file_path = os.path.join(upload_dir, file_name)
        
        with open(file_path, "wb") as f:
            f.write(file_content)
            
        return file_path
    except Exception as e:
        logger.error(f"Error saving uploaded file: {e}")
        raise
