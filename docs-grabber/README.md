# Model Context Protocol Server Setup

This server allows you to provide documentation links or PDF files for Claude to ingest and process. It can crawl websites, extract text from PDFs, and send the formatted content to Claude's API.

## Prerequisites

- Python 3.8 or higher
- An Anthropic API key

## Setup Instructions

1. Clone the repository or download the code files.

2. Install the required dependencies:
   ```bash
   pip install -r requirements.txt
   ```

3. Set up your Anthropic API key as an environment variable:
   ```bash
   # For Linux/MacOS
   export ANTHROPIC_API_KEY=your_api_key_here
   
   # For Windows
   set ANTHROPIC_API_KEY=your_api_key_here
   ```

4. Run the server:
   ```bash
   python app.py
   ```

5. Access the web interface by navigating to `http://localhost:8000` in your browser.

## Usage

### Processing a Documentation Website

1. Open the web interface and ensure you're on the "URL" tab.
2. Enter the documentation URL in the input field.
3. Optionally, set the maximum number of pages to crawl and the crawl depth.
4. Click "Process URL" to start crawling and processing the documentation.
5. Wait for the process to complete and view Claude's response.

### Processing a PDF

1. Switch to the "PDF Upload" tab.
2. Click "Choose File" and select a PDF document.
3. Click "Process PDF" to upload and process the file.
4. Wait for the process to complete and view Claude's response.

### Using a Custom Prompt

1. Switch to the "Custom Prompt" tab.
2. Enter your custom prompt for Claude.
3. Optionally, provide additional context to include in Claude's system prompt.
4. Click "Send to Claude" to process your prompt.

## Features

- **Web Crawling**: Crawls documentation websites and extracts text content.
- **PDF Processing**: Extracts text from uploaded PDF files.
- **Claude Integration**: Sends processed documentation to Claude for summarization and explanation.
- **Custom Prompts**: Allows sending custom prompts to Claude with or without additional context.
- **Asynchronous Processing**: Handles long-running tasks asynchronously with progress tracking.

## Limitations

- The server crawls only within the same domain as the provided URL.
- PDF processing may not perfectly extract all formatting and special characters.
- There are rate limits on the Anthropic API that may affect processing large amounts of documentation.
- The server doesn't currently handle authentication for accessing protected documentation sites.

## Extending the Server

You can extend this server by:

1. Adding support for other document formats (e.g., DOCX, Markdown).
2. Implementing authentication for protected documentation sites.
3. Adding a caching mechanism to avoid re-processing the same documentation.
4. Implementing advanced text processing techniques for better context preparation.