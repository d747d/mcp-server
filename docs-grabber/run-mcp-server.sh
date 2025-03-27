#!/bin/bash
# run-mcp-reference-server.sh - Launcher for URL Reference MCP server with proper environment

# Configuration - adjust these values for your system
PYTHON_PATH="/home/user/repos/mcp-server/docs-grabber/.venv/bin/python"  # Default to system Python, will check if it exists
SCRIPT_PATH="/home/user/repos/mcp-server/docs-grabber/url-reference-server.py"  # Default to script in same directory

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --python)
      PYTHON_PATH="$2"
      shift 2
      ;;
    --script)
      SCRIPT_PATH="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      echo "Usage: $0 [--python python_path] [--script script_path]"
      exit 1
      ;;
  esac
done

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

# Check if python exists in PATH if not specified
if [ "$PYTHON_PATH" = "python3" ]; then
  if ! command -v python3 &> /dev/null; then
    echo "WARNING: python3 not found in PATH, trying python" | tee -a "$LOG_FILE"
    if ! command -v python &> /dev/null; then
      echo "ERROR: Neither python3 nor python found in PATH" | tee -a "$LOG_FILE"
      exit 1
    fi
    PYTHON_PATH="python"
  fi
# Otherwise check if specified Python exists
elif [ ! -f "$PYTHON_PATH" ] && ! command -v "$PYTHON_PATH" &> /dev/null; then
  echo "ERROR: Python interpreter not found at $PYTHON_PATH" | tee -a "$LOG_FILE"
  exit 1
fi

# Resolve script path to absolute path if it's relative
if [[ ! "$SCRIPT_PATH" = /* ]]; then
  SCRIPT_PATH="$(pwd)/$SCRIPT_PATH"
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
  "$PYTHON_PATH" -c "import sys; print(f'Python executable: {sys.executable}')"
} >> "$LOG_FILE" 2>&1

# Check and install dependencies if needed
check_and_install_package() {
  local package=$1
  if ! "$PYTHON_PATH" -m pip show "$package" >> "$LOG_FILE" 2>&1; then
    echo "Installing $package package..." | tee -a "$LOG_FILE"
    "$PYTHON_PATH" -m pip install "$package" >> "$LOG_FILE" 2>&1
    
    if [ $? -ne 0 ]; then
      echo "ERROR: Failed to install $package. Please install it manually: pip install $package" | tee -a "$LOG_FILE"
      exit 1
    fi
  fi
}

check_and_install_package "mcp[cli]"
check_and_install_package "httpx"

# Log server startup
echo "===== STARTING MCP SERVER =====" | tee -a "$LOG_FILE"
echo "Running script: $SCRIPT_PATH" | tee -a "$LOG_FILE"
echo "Log file: $LOG_FILE"

# Set up environment for MCP
export PYTHONUNBUFFERED=1
export MCP_DEBUG=1

# Check if the script is executable, make it executable if not
if [ ! -x "$SCRIPT_PATH" ]; then
  echo "Making script executable..." | tee -a "$LOG_FILE"
  chmod +x "$SCRIPT_PATH"
fi

# Run the server
"$PYTHON_PATH" "$SCRIPT_PATH" 2>&1 | tee -a "$LOG_FILE"

# Check exit status
exit_status=$?
if [ $exit_status -ne 0 ]; then
  echo "MCP server exited with error code: $exit_status" | tee -a "$LOG_FILE"
else
  echo "MCP server exited normally" | tee -a "$LOG_FILE"
fi

exit $exit_status