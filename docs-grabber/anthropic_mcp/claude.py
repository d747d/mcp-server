"""
Claude integration functions for MCP Server.
Handles formatting context for Claude and interacting with Claude API.
"""

import logging
import os
from typing import List, Dict, Any, Optional

# Setup logging
logger = logging.getLogger("anthropic-mcp-server")

def format_for_claude(relevant_docs: List[Dict[str, Any]], model: str, max_tokens: int) -> str:
    """Format relevant documents as context for Claude"""
    try:
        context_parts = []
        
        # Add a header
        context_parts.append("# REFERENCE DOCUMENTATION")
        context_parts.append("The following documentation snippets may be helpful:")
        context_parts.append("")
        
        # Add each relevant chunk
        for i, doc in enumerate(relevant_docs):
            # Add source information
            title = doc.get("title", doc["source"])
            context_parts.append(f"## SOURCE {i+1}: {title}")
            
            # Add the actual content
            context_parts.append(doc["chunk"].strip())
            context_parts.append("")
        
        # Add a footer
        context_parts.append("END OF REFERENCE DOCUMENTATION")
        
        # Join all parts
        context = "\n".join(context_parts)
        
        # Simple truncation to fit token limit
        # In a more sophisticated implementation, you would use a proper tokenizer
        # and handle truncation more intelligently
        approx_tokens = len(context.split()) // 3  # Very rough approximation
        if approx_tokens > max_tokens:
            # Truncate and add a warning
            words = context.split()
            context = " ".join(words[:max_tokens * 3])
            context += "\n\n[Note: The reference documentation was truncated to fit the token limit.]"
        
        return context
    except Exception as e:
        logger.error(f"Error formatting context for Claude: {e}")
        raise

def get_anthropic_api_key() -> str:
    """Get the Anthropic API key from environment variables"""
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        logger.warning("ANTHROPIC_API_KEY environment variable not set")
    return api_key

def create_completion_with_context(
    messages: List[Dict[str, Any]],
    context: str,
    model: str = "claude-3-opus-20240229",
    temperature: float = 0.7,
    max_tokens: int = 1000
) -> Dict[str, Any]:
    """Create a completion with Claude API using the provided context"""
    try:
        api_key = get_anthropic_api_key()
        if not api_key:
            raise ValueError("Anthropic API key is required")
        
        # Import here to avoid dependency if unused
        import anthropic
        
        client = anthropic.Anthropic(api_key=api_key)
        
        # Prepare system prompt with context
        system_prompt = f"""You have access to the following reference documentation. Use this information to provide accurate and helpful responses.

{context}

When you use information from the reference documentation, cite the source as [SOURCE X] where X is the source number."""
        
        # Create completion
        response = client.messages.create(
            model=model,
            system=system_prompt,
            messages=messages,
            temperature=temperature,
            max_tokens=max_tokens
        )
        
        return {
            "id": response.id,
            "model": response.model,
            "content": response.content,
            "usage": {
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens
            }
        }
    except Exception as e:
        logger.error(f"Error creating completion with context: {e}")
        raise
