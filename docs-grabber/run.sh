#!/bin/bash
#
# MCP Server Launcher Script
# This script launches the MCP server with proper environment setup

# Set default values
PORT=8000
HOST="0.0.0.0"
LOG_LEVEL="info"
DATA_DIR="$HOME/.anthropic_mcp"
VENV_PATH="/home/user/repos/mcp-server/docs-grabber/.venv"  # Updated with your specific path

# Set up environment variables
export PYTHONPATH="$(pwd):$PYTHONPATH"
export ANTHROPIC_MCP_DATA_DIR="$DATA_DIR"

# Check if we're in a piped context (like in oterm)
if [ -t 0 ]; then
  INTERACTIVE=true
else
  INTERACTIVE=false
fi

# Parse command line arguments if running in interactive mode
if [ "$INTERACTIVE" = true ]; then
  while [[ $# -gt 0 ]]; do
    case $1 in
      --port)
        PORT="$2"
        shift 2
        ;;
      --host)
        HOST="$2"
        shift 2
        ;;
      --log-level)
        LOG_LEVEL="$2"
        shift 2
        ;;
      --data-dir)
        DATA_DIR="$2"
        export ANTHROPIC_MCP_DATA_DIR="$DATA_DIR"
        shift 2
        ;;
      --venv)
        VENV_PATH="$2"
        shift 2
        ;;
      *)
        echo "Unknown option: $1"
        echo "Usage: $0 [--port PORT] [--host HOST] [--log-level LEVEL] [--data-dir DIR] [--venv PATH]"
        exit 1
        ;;
    esac
  done
fi

# Activate virtual environment
if [ -f "$VENV_PATH/bin/activate" ]; then
  source "$VENV_PATH/bin/activate"
  echo "Activated virtual environment at $VENV_PATH"
elif [ -f "$VENV_PATH/Scripts/activate" ]; then
  # Windows path
  source "$VENV_PATH/Scripts/activate"
  echo "Activated virtual environment at $VENV_PATH"
else
  echo "No virtual environment found at $VENV_PATH, using system Python"
fi

# Check if server is running in interactive mode or as part of oterm
if [ "$INTERACTIVE" = true ]; then
  # Interactive mode - start server directly
  echo "Starting MCP server on $HOST:$PORT..."
  echo "Data directory: $DATA_DIR"
  echo "Log level: $LOG_LEVEL"
  
  # Start the server
  python main.py --host "$HOST" --port "$PORT" --log-level "$LOG_LEVEL" --data-dir "$DATA_DIR"
else
  # Non-interactive mode (oterm) - bridge mode
  # Start the server in the background
  python main.py --host "$HOST" --port "$PORT" --log-level "$LOG_LEVEL" --data-dir "$DATA_DIR" > /dev/null 2>&1 &
  SERVER_PID=$!
  
  # Wait for server to start
  MAX_RETRIES=10
  for i in $(seq 1 $MAX_RETRIES); do
    if curl -s "http://localhost:$PORT/health" > /dev/null; then
      break
    fi
    
    if [ $i -eq $MAX_RETRIES ]; then
      echo "{\"error\": \"Failed to start MCP server after $MAX_RETRIES attempts\"}" >&2
      kill $SERVER_PID 2>/dev/null
      exit 1
    fi
    
    sleep 1
  done
  
  # Process input from stdin and forward to server
  while read -r line; do
    if [ -z "$line" ]; then
      continue
    fi
    
    # Try to parse as JSON
    if echo "$line" | jq -e . >/dev/null 2>&1; then
      # Extract query from JSON
      QUERY=$(echo "$line" | jq -r '.query // ""')
    else
      # Treat as plain text
      QUERY="$line"
    fi
    
    # If this is a command
    if [[ "$QUERY" == /mcp* ]]; then
      # Create a mock message to send to the MCP server
      JSON_DATA="{\"messages\": [{\"role\": \"user\", \"content\": \"$QUERY\"}]}"
      
      # Send to MCP server
      RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$JSON_DATA" "http://localhost:$PORT/mcp/v1/context")
      
      # Forward response
      echo "$RESPONSE"
    else
      # For regular queries
      JSON_DATA="{\"query\": \"$QUERY\", \"max_tokens\": 4000}"
      
      # Send to context endpoint
      RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" -d "$JSON_DATA" "http://localhost:$PORT/context")
      
      # Check if we got a valid response
      if [ $? -eq 0 ] && [ ! -z "$RESPONSE" ]; then
        # Extract context
        CONTEXT=$(echo "$RESPONSE" | jq -r '.context // ""')
        
        # Create MCP-compatible response
        echo "{\"context\": \"$CONTEXT\"}"
      else
        # Return empty context
        echo "{\"context\": \"\"}"
      fi
    fi
  done
  
  # When stdin closes, kill the server
  kill $SERVER_PID 2>/dev/null
fi