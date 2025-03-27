"""
Text processing functions for MCP Server.
Handles chunking, cleaning, and metadata extraction for documents.
"""

import logging
import os
import re
import datetime
from typing import List, Dict, Any
import pypdf

# Setup logging
logger = logging.getLogger("anthropic-mcp-server")

def chunk_text(text: str, chunk_size: int = 1000, overlap: int = 100) -> List[str]:
    """Split text into overlapping chunks of a specified size"""
    try:
        chunks = []
        if len(text) <= chunk_size:
            chunks.append(text)
            return chunks
        
        # Split text into sentences (naive approach)
        sentences = text.replace("\n", " ").split(". ")
        sentences = [s + "." for s in sentences if s]
        
        current_chunk = ""
        for sentence in sentences:
            # If adding this sentence would make the chunk too long, store the chunk and start a new one
            if len(current_chunk) + len(sentence) > chunk_size and current_chunk:
                chunks.append(current_chunk)
                # Keep some overlap for context
                words = current_chunk.split()
                if len(words) > overlap:
                    current_chunk = " ".join(words[-overlap:])
                else:
                    current_chunk = ""
            
            current_chunk += " " + sentence
        
        # Don't forget the last chunk
        if current_chunk:
            chunks.append(current_chunk)
        
        return chunks
    except Exception as e:
        logger.error(f"Error chunking text: {e}")
        raise

def clean_text(text: str) -> str:
    """Clean and normalize text for processing"""
    try:
        # Replace multiple newlines with a single one
        text = re.sub(r"\n\s*\n", "\n\n", text)
        
        # Replace multiple spaces with a single space
        text = re.sub(r" +", " ", text)
        
        # Remove control characters
        text = re.sub(r"[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]", "", text)
        
        # Trim leading/trailing whitespace
        text = text.strip()
        
        return text
    except Exception as e:
        logger.error(f"Error cleaning text: {e}")
        raise

def extract_metadata(file_path: str, file_type: str) -> Dict[str, Any]:
    """Extract metadata from various file types"""
    try:
        metadata = {
            "file_name": os.path.basename(file_path),
            "file_type": file_type,
            "extraction_date": datetime.datetime.now().isoformat()
        }
        
        if file_type.lower() == "pdf":
            with open(file_path, "rb") as f:
                pdf_reader = pypdf.PdfReader(f)
                if hasattr(pdf_reader, "metadata") and pdf_reader.metadata:
                    for key, value in pdf_reader.metadata.items():
                        # Clean up key name by removing leading "/"
                        clean_key = key.replace("/", "").lower() if isinstance(key, str) else str(key)
                        metadata[clean_key] = str(value)
                        
                metadata["page_count"] = len(pdf_reader.pages)
        
        # Add more file types as needed
        
        return metadata
    except Exception as e:
        logger.error(f"Error extracting metadata: {e}")
        # Return basic metadata if we encounter an error
        return {
            "file_name": os.path.basename(file_path),
            "file_type": file_type,
            "extraction_date": datetime.datetime.now().isoformat(),
            "error": str(e)
        }
