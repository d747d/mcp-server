#!/bin/bash
# run-mcp-reference-server.sh - Launcher for URL Reference MCP server with proper environment

# Configuration - adjust these values for your system
PYTHON_PATH="/home/user/repos/mcp-server/docs-grabber/.venv/bin/python3"  # Change to your preferred Python path
SCRIPT_PATH="/home/user/repos/mcp-server/docs-grabber/url-reference-server.py"  # Change to your server script path

# Create log directory
LOG_DIR="$HOME/.mcp-logs"
mkdir -p "$LOG_DIR"

# Log file with timestamp
TIMESTAMP=$(date +"%Y%m%d-%H%M%S")
LOG_FILE="$LOG_DIR/mcp-server-$TIMESTAMP.log"

# Log execution details
{
  echo "===== MCP SERVER LAUNCH ====="
  echo "Date and time: $(date)"
  echo "Python path: $PYTHON_PATH"
  echo "Script path: $SCRIPT_PATH"
  echo "Working directory: $(pwd)"
  echo "Environment variables:"
  env | grep -E '^(PYTHON|MCP)'
} > "$LOG_FILE"

# Check if Python exists
if [ ! -f "$PYTHON_PATH" ]; then
  echo "ERROR: Python interpreter not found at $PYTHON_PATH" | tee -a "$LOG_FILE"
  exit 1
fi

# Check if the script exists
if [ ! -f "$SCRIPT_PATH" ]; then
  echo "ERROR: Server script not found at $SCRIPT_PATH" | tee -a "$LOG_FILE"
  exit 1
fi

# Log Python version
{
  echo "===== PYTHON ENVIRONMENT ====="
  "$PYTHON_PATH" --version
} >> "$LOG_FILE" 2>&1

# Check and install dependencies if needed
check_and_install_package() {
  local package=$1
  if ! "$PYTHON_PATH" -m pip show "$package" >> "$LOG_FILE" 2>&1; then
    echo "Installing $package package..." | tee -a "$LOG_FILE"
    "$PYTHON_PATH" -m pip install "$package" >> "$LOG_FILE" 2>&1
  fi
}

check_and_install_package "mcp[cli]"
check_and_install_package "httpx"

# Log server startup
echo "===== STARTING MCP SERVER =====" | tee -a "$LOG_FILE"
echo "Running script: $SCRIPT_PATH" | tee -a "$LOG_FILE"
echo "Log file: $LOG_FILE"

# Export environment variables for MCP
export PYTHONUNBUFFERED=1
export MCP_DEBUG=1

# Run the server
"$PYTHON_PATH" "$SCRIPT_PATH" 2>&1 | tee -a "$LOG_FILE"