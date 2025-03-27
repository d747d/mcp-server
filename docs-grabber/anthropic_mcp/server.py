"""
Main server implementation for the Anthropic MCP Server.
Provides FastAPI endpoints for document ingestion and context retrieval.
"""

import os
import json
import logging
from typing import Dict, List, Optional, Any

from fastapi import FastAPI, HTTPException, BackgroundTasks, Request, File, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from anthropic_mcp import ingestion, processing, indexing, claude, utils

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("anthropic-mcp-server")

app = FastAPI(title="Anthropic MCP Server", 
              description="Open-source implementation of Anthropic Model Context Protocol",
              version="0.1.0")

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Allows all origins
    allow_credentials=True,
    allow_methods=["*"],  # Allows all methods
    allow_headers=["*"],  # Allows all headers
)

# Get data directory from environment or use default
DATA_DIR = os.environ.get("ANTHROPIC_MCP_DATA_DIR", os.path.expanduser("~/.anthropic_mcp"))
os.makedirs(DATA_DIR, exist_ok=True)

# In-memory storage for simplicity in this MVP
# In a production system, this would be a proper database
documents = {}
embeddings = {}

# Pydantic models for request/response
class Document(BaseModel):
    source: str
    title: Optional[str] = None
    content: str
    metadata: Dict[str, Any] = {}

class ContextRequest(BaseModel):
    query: str
    model: str = "claude-3-opus-20240229"
    max_tokens: int = 1000
    sources: List[str] = []

class ContextResponse(BaseModel):
    context: str
    source_documents: List[str]
    token_count: int

@app.get("/")
async def root():
    return {"message": "Anthropic Model Context Protocol Server"}

@app.get("/health")
async def health_check():
    return {"status": "healthy"}

@app.post("/documents/pdf", response_model=Document)
async def ingest_pdf(background_tasks: BackgroundTasks, file: UploadFile = File(...)):
    """Ingest a PDF document into the context system"""
    try:
        # Save PDF temporarily
        temp_path = f"/tmp/{file.filename}"
        with open(temp_path, "wb") as f:
            content = await file.read()
            f.write(content)
        
        # Extract text from PDF
        text = ingestion.extract_pdf_text(temp_path)
        
        # Clean the text
        cleaned_text = processing.clean_text(text)
        
        # Extract metadata
        doc_metadata = processing.extract_metadata(temp_path, "pdf")
        
        # Create document ID
        doc_id = f"pdf_{file.filename}"
        
        # Store document
        document = {
            "source": doc_id,
            "title": file.filename,
            "content": cleaned_text,
            "metadata": doc_metadata
        }
        documents[doc_id] = document
        
        # Background task to process for embedding
        background_tasks.add_task(
            process_document_for_embedding, 
            doc_id, 
            cleaned_text
        )
        
        # Cleanup
        os.remove(temp_path)
        
        return document
    except Exception as e:
        logger.error(f"Error ingesting PDF: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/documents/web", response_model=Document)
async def ingest_web(background_tasks: BackgroundTasks, url: str):
    """Ingest a web page into the context system"""
    try:
        # Check URL safety
        if not utils.is_safe_url(url):
            raise HTTPException(status_code=400, detail="URL failed security check")
            
        # Scrape the web page
        text, title = ingestion.scrape_webpage(url)
        
        # Clean the text
        cleaned_text = processing.clean_text(text)
        
        # Extract metadata
        doc_metadata = {"url": url, "title": title}
        
        # Create document ID
        doc_id = f"web_{url.replace('://', '_').replace('/', '_')}"
        
        # Store document
        document = {
            "source": doc_id,
            "title": title,
            "content": cleaned_text,
            "metadata": doc_metadata
        }
        documents[doc_id] = document
        
        # Background task to process for embedding
        background_tasks.add_task(
            process_document_for_embedding, 
            doc_id, 
            cleaned_text
        )
        
        return document
    except Exception as e:
        logger.error(f"Error ingesting web page: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/documents/text", response_model=Document)
async def ingest_text(background_tasks: BackgroundTasks, document: Document):
    """Ingest a text document into the context system"""
    try:
        # Clean the text
        cleaned_text = processing.clean_text(document.content)
        
        # Create document ID
        doc_id = f"text_{document.source}"
        
        # Store document
        stored_document = {
            "source": doc_id,
            "title": document.title or document.source,
            "content": cleaned_text,
            "metadata": document.metadata
        }
        documents[doc_id] = stored_document
        
        # Background task to process for embedding
        background_tasks.add_task(
            process_document_for_embedding, 
            doc_id, 
            cleaned_text
        )
        
        return stored_document
    except Exception as e:
        logger.error(f"Error ingesting text: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/documents", response_model=List[str])
async def list_documents():
    """List all available documents"""
    return list(documents.keys())

@app.get("/documents/{doc_id}", response_model=Document)
async def get_document(doc_id: str):
    """Get a specific document by ID"""
    if doc_id not in documents:
        raise HTTPException(status_code=404, detail="Document not found")
    
    return documents[doc_id]

@app.delete("/documents/{doc_id}")
async def delete_document(doc_id: str):
    """Delete a document by ID"""
    if doc_id not in documents:
        raise HTTPException(status_code=404, detail="Document not found")
    
    del documents[doc_id]
    if doc_id in embeddings:
        del embeddings[doc_id]
    
    return {"status": "deleted", "document_id": doc_id}

@app.post("/context", response_model=ContextResponse)
async def get_context(request: ContextRequest):
    """Get relevant context based on a query"""
    try:
        # If specific sources are provided, use only those
        source_docs = request.sources if request.sources else list(documents.keys())
        
        # Check if all specified sources exist
        for src in source_docs:
            if src not in documents:
                raise HTTPException(status_code=404, detail=f"Document source not found: {src}")
        
        # Get relevant chunks based on query
        relevant_docs = indexing.find_relevant_chunks(
            query=request.query,
            documents={k: documents[k] for k in source_docs if k in embeddings},
            embeddings=embeddings,
            top_k=5  # Number of chunks to return
        )
        
        # Format context for Claude
        formatted_context = claude.format_for_claude(
            relevant_docs,
            model=request.model,
            max_tokens=request.max_tokens
        )
        
        # Count tokens (approximate)
        token_count = len(formatted_context.split()) // 3  # Very rough approximation
        
        return {
            "context": formatted_context,
            "source_documents": [doc["source"] for doc in relevant_docs],
            "token_count": token_count
        }
    except Exception as e:
        logger.error(f"Error retrieving context: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/mcp/v1/context")
async def mcp_v1_context(request: Request):
    """MCP v1 context endpoint that follows Anthropic's MCP specification"""
    try:
        # Parse the request body
        request_data = await request.json()
        
        # Extract query from the request
        query = request_data.get("query", "")
        if not query:
            # Try to extract from messages if query is not directly provided
            messages = request_data.get("messages", [])
            if messages and messages[-1].get("role") == "user":
                query = messages[-1].get("content", "")
        
        if not query:
            raise HTTPException(status_code=400, detail="No query found in request")
        
        # Extract model information
        model = request_data.get("model", "claude-3-opus-20240229")
        
        # Get context
        context_request = ContextRequest(
            query=query,
            model=model,
            max_tokens=4000  # Default to a reasonable size
        )
        
        context_response = await get_context(context_request)
        
        # Format response according to MCP specification
        mcp_response = {
            "context": context_response.context,
            "relevant_documents": context_response.source_documents
        }
        
        return JSONResponse(content=mcp_response)
    except Exception as e:
        logger.error(f"Error in MCP context endpoint: {e}")
        raise HTTPException(status_code=500, detail=str(e))

# Background processing function
async def process_document_for_embedding(doc_id: str, text: str):
    """Process a document for embedding in the background"""
    try:
        # Chunk the document
        chunks = processing.chunk_text(text)
        
        # Create embeddings for each chunk
        doc_embeddings = indexing.create_embeddings(chunks)
        
        # Store embeddings
        embeddings[doc_id] = {
            "chunks": chunks,
            "embeddings": doc_embeddings
        }
        
        logger.info(f"Successfully processed document {doc_id} for embedding")
    except Exception as e:
        logger.error(f"Error processing document {doc_id} for embedding: {e}")
