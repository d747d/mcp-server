"""
Indexing and retrieval functions for MCP Server.
Handles creating embeddings and finding relevant content.
"""

import logging
import numpy as np
from typing import List, Dict, Any

# Setup logging
logger = logging.getLogger("anthropic-mcp-server")

# Using a global variable to lazy-load the embedding model
_embedding_model = None

def get_embedding_model():
    """Get the embedding model, loading it if necessary"""
    global _embedding_model
    if _embedding_model is None:
        try:
            from sentence_transformers import SentenceTransformer
            # Using a small model for efficiency, but you can use a larger one for better quality
            _embedding_model = SentenceTransformer("all-MiniLM-L6-v2")
            logger.info("Loaded embedding model: all-MiniLM-L6-v2")
        except Exception as e:
            logger.error(f"Error loading embedding model: {e}")
            raise
    return _embedding_model

def create_embeddings(text_chunks: List[str]) -> List[List[float]]:
    """Create embeddings for a list of text chunks"""
    try:
        model = get_embedding_model()
        embeddings = model.encode(text_chunks, convert_to_numpy=True)
        return embeddings.tolist()  # Convert to list for JSON serialization
    except Exception as e:
        logger.error(f"Error creating embeddings: {e}")
        raise

def calculate_similarity(query_embedding: List[float], chunk_embedding: List[float]) -> float:
    """Calculate cosine similarity between two embeddings"""
    try:
        query_vec = np.array(query_embedding)
        chunk_vec = np.array(chunk_embedding)
        
        # Normalize vectors
        query_vec = query_vec / np.linalg.norm(query_vec)
        chunk_vec = chunk_vec / np.linalg.norm(chunk_vec)
        
        # Calculate cosine similarity
        similarity = np.dot(query_vec, chunk_vec)
        
        return float(similarity)
    except Exception as e:
        logger.error(f"Error calculating similarity: {e}")
        raise

def find_relevant_chunks(query: str, documents: Dict[str, Any], embeddings: Dict[str, Any], top_k: int = 5) -> List[Dict[str, Any]]:
    """Find the most relevant chunks based on a query"""
    try:
        # Get embedding for the query
        query_embedding = create_embeddings([query])[0]
        
        # Calculate similarity for each chunk
        similarities = []
        
        for doc_id, doc in documents.items():
            if doc_id not in embeddings:
                continue
                
            doc_chunks = embeddings[doc_id]["chunks"]
            doc_embeddings = embeddings[doc_id]["embeddings"]
            
            for i, (chunk, chunk_embedding) in enumerate(zip(doc_chunks, doc_embeddings)):
                similarity = calculate_similarity(query_embedding, chunk_embedding)
                
                similarities.append({
                    "doc_id": doc_id,
                    "chunk_index": i,
                    "chunk": chunk,
                    "similarity": similarity,
                    "source": doc["source"],
                    "title": doc.get("title", doc_id)
                })
        
        # Sort by similarity (descending)
        similarities.sort(key=lambda x: x["similarity"], reverse=True)
        
        # Return top_k results
        return similarities[:top_k]
    except Exception as e:
        logger.error(f"Error finding relevant chunks: {e}")
        raise
