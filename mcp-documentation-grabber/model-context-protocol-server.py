import os
import re
import requests
import tempfile
from typing import List, Dict, Optional, Union
from urllib.parse import urlparse, urljoin
from fastapi import FastAPI, Request, Form, UploadFile, File, HTTPException, BackgroundTasks
from fastapi.responses import HTMLResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from pydantic import BaseModel, HttpUrl
from bs4 import BeautifulSoup
import PyPDF2
import anthropic
from anthropic import Anthropic
import uvicorn
import logging
from pathlib import Path
import httpx
import time
from fastapi.middleware.cors import CORSMiddleware
from concurrent.futures import ThreadPoolExecutor

# Set up logging
logging.basicConfig(level=logging.INFO, 
                    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

# Initialize FastAPI app
app = FastAPI(title="Model Context Protocol Server", 
              description="A server that allows users to provide documentation links for Claude to ingest")

# CORS middleware to allow cross-origin requests
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Allows all origins
    allow_credentials=True,
    allow_methods=["*"],  # Allows all methods
    allow_headers=["*"],  # Allows all headers
)

# Set up templates and static files
templates = Jinja2Templates(directory="templates")
os.makedirs("templates", exist_ok=True)
with open("templates/index.html", "w") as f:
    f.write("""
<!DOCTYPE html>
<html>
<head>
    <title>Model Context Protocol Server</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
        }
        .container {
            border: 1px solid #ddd;
            padding: 20px;
            border-radius: 5px;
        }
        .form-group {
            margin-bottom: 15px;
        }
        label {
            display: block;
            margin-bottom: 5px;
        }
        input[type="text"], input[type="file"], textarea {
            width: 100%;
            padding: 8px;
            box-sizing: border-box;
            border: 1px solid #ddd;
            border-radius: 4px;
        }
        button {
            background-color: #4CAF50;
            color: white;
            padding: 10px 15px;
            border: none;
            border-radius: 4px;
            cursor: pointer;
        }
        button:hover {
            background-color: #45a049;
        }
        .response {
            margin-top: 20px;
            padding: 15px;
            border: 1px solid #ddd;
            border-radius: 5px;
            background-color: #f9f9f9;
            white-space: pre-wrap;
        }
        .tabs {
            overflow: hidden;
            border: 1px solid #ddd;
            background-color: #f1f1f1;
            border-radius: 5px 5px 0 0;
        }
        .tab {
            background-color: inherit;
            float: left;
            border: none;
            outline: none;
            cursor: pointer;
            padding: 14px 16px;
            transition: 0.3s;
        }
        .tab:hover {
            background-color: #ddd;
        }
        .tab.active {
            background-color: #ccc;
        }
        .tabcontent {
            display: none;
            padding: 20px;
            border: 1px solid #ddd;
            border-top: none;
            border-radius: 0 0 5px 5px;
        }
        .progress-container {
            margin-top: 20px;
            display: none;
        }
        .progress-bar {
            width: 100%;
            background-color: #ddd;
            border-radius: 4px;
            padding: 3px;
        }
        .progress {
            width: 0%;
            height: 20px;
            background-color: #4CAF50;
            border-radius: 4px;
            transition: width 0.3s;
            text-align: center;
            line-height: 20px;
            color: white;
        }
        .status {
            margin-top: 10px;
            font-style: italic;
        }
    </style>
</head>
<body>
    <h1>Model Context Protocol Server</h1>
    <p>Provide a documentation link or upload a PDF file to ingest into Claude's context.</p>
    
    <div class="tabs">
        <button class="tab active" onclick="openTab(event, 'urlTab')">URL</button>
        <button class="tab" onclick="openTab(event, 'pdfTab')">PDF Upload</button>
        <button class="tab" onclick="openTab(event, 'promptTab')">Custom Prompt</button>
    </div>
    
    <div id="urlTab" class="tabcontent" style="display: block;">
        <form id="urlForm" action="/process_url" method="post">
            <div class="form-group">
                <label for="url">Documentation URL:</label>
                <input type="text" id="url" name="url" required placeholder="https://example.com/docs">
            </div>
            <div class="form-group">
                <label for="max_pages">Maximum pages to crawl (leave empty for unlimited):</label>
                <input type="number" id="max_pages" name="max_pages" placeholder="10">
            </div>
            <div class="form-group">
                <label for="crawl_depth">Crawl depth:</label>
                <input type="number" id="crawl_depth" name="crawl_depth" value="1" min="0" max="3">
            </div>
            <button type="submit">Process URL</button>
        </form>
    </div>
    
    <div id="pdfTab" class="tabcontent">
        <form id="pdfForm" action="/process_pdf" method="post" enctype="multipart/form-data">
            <div class="form-group">
                <label for="pdf_file">Upload PDF:</label>
                <input type="file" id="pdf_file" name="pdf_file" accept=".pdf" required>
            </div>
            <button type="submit">Process PDF</button>
        </form>
    </div>
    
    <div id="promptTab" class="tabcontent">
        <form id="promptForm" action="/custom_prompt" method="post">
            <div class="form-group">
                <label for="prompt">Custom prompt for Claude:</label>
                <textarea id="prompt" name="prompt" rows="5" required placeholder="Enter your prompt here..."></textarea>
            </div>
            <div class="form-group">
                <label for="context">Optional context (will be added to Claude's system prompt):</label>
                <textarea id="context" name="context" rows="3" placeholder="Enter additional context here..."></textarea>
            </div>
            <button type="submit">Send to Claude</button>
        </form>
    </div>
    
    <div class="progress-container" id="progressContainer">
        <div class="progress-bar">
            <div class="progress" id="progressBar">0%</div>
        </div>
        <div class="status" id="statusText">Processing...</div>
    </div>
    
    <div class="response" id="responseContainer" style="display: none;">
        <h3>Claude's Response:</h3>
        <div id="responseContent"></div>
    </div>
    
    <script>
        function openTab(evt, tabName) {
            var i, tabcontent, tablinks;
            tabcontent = document.getElementsByClassName("tabcontent");
            for (i = 0; i < tabcontent.length; i++) {
                tabcontent[i].style.display = "none";
            }
            tablinks = document.getElementsByClassName("tab");
            for (i = 0; i < tablinks.length; i++) {
                tablinks[i].className = tablinks[i].className.replace(" active", "");
            }
            document.getElementById(tabName).style.display = "block";
            evt.currentTarget.className += " active";
        }
        
        document.getElementById('urlForm').addEventListener('submit', function(e) {
            e.preventDefault();
            submitForm('urlForm', '/process_url');
        });
        
        document.getElementById('pdfForm').addEventListener('submit', function(e) {
            e.preventDefault();
            submitFormWithFile('pdfForm', '/process_pdf');
        });
        
        document.getElementById('promptForm').addEventListener('submit', function(e) {
            e.preventDefault();
            submitForm('promptForm', '/custom_prompt');
        });
        
        function submitForm(formId, endpoint) {
            showProgress();
            const form = document.getElementById(formId);
            const formData = new FormData(form);
            
            fetch(endpoint, {
                method: 'POST',
                body: formData
            })
            .then(response => response.json())
            .then(data => {
                hideProgress();
                showResponse(data);
            })
            .catch(error => {
                hideProgress();
                showResponse({error: 'An error occurred: ' + error});
            });
        }
        
        function submitFormWithFile(formId, endpoint) {
            showProgress();
            const form = document.getElementById(formId);
            const formData = new FormData(form);
            
            fetch(endpoint, {
                method: 'POST',
                body: formData
            })
            .then(response => response.json())
            .then(data => {
                if (data.task_id) {
                    checkTaskStatus(data.task_id);
                } else {
                    hideProgress();
                    showResponse(data);
                }
            })
            .catch(error => {
                hideProgress();
                showResponse({error: 'An error occurred: ' + error});
            });
        }
        
        function checkTaskStatus(taskId) {
            fetch(`/task_status/${taskId}`)
            .then(response => response.json())
            .then(data => {
                if (data.status === 'completed') {
                    hideProgress();
                    showResponse(data);
                } else if (data.status === 'failed') {
                    hideProgress();
                    showResponse({error: data.error || 'Task failed'});
                } else {
                    updateProgress(data.progress || 0, data.status_message || 'Processing...');
                    setTimeout(() => checkTaskStatus(taskId), 2000);
                }
            })
            .catch(error => {
                hideProgress();
                showResponse({error: 'Error checking task status: ' + error});
            });
        }
        
        function showProgress() {
            document.getElementById('progressContainer').style.display = 'block';
            document.getElementById('responseContainer').style.display = 'none';
        }
        
        function hideProgress() {
            document.getElementById('progressContainer').style.display = 'none';
        }
        
        function updateProgress(percent, message) {
            document.getElementById('progressBar').style.width = percent + '%';
            document.getElementById('progressBar').textContent = percent + '%';
            document.getElementById('statusText').textContent = message;
        }
        
        function showResponse(data) {
            const container = document.getElementById('responseContainer');
            const content = document.getElementById('responseContent');
            
            container.style.display = 'block';
            
            if (data.error) {
                content.innerHTML = `<div style="color: red;">${data.error}</div>`;
            } else if (data.response) {
                content.innerHTML = data.response.replace(/\\n/g, '<br>');
            } else {
                content.innerHTML = '<pre>' + JSON.stringify(data, null, 2) + '</pre>';
            }
        }
    </script>
</body>
</html>
    """)

# Pydantic models for validation
class URLInput(BaseModel):
    url: HttpUrl
    max_pages: Optional[int] = None
    crawl_depth: Optional[int] = 1

class PDFInput(BaseModel):
    file_path: str

class PromptInput(BaseModel):
    prompt: str
    context: Optional[str] = None

class TaskStatus(BaseModel):
    task_id: str
    status: str
    progress: Optional[float] = None
    status_message: Optional[str] = None
    response: Optional[str] = None
    error: Optional[str] = None

# Dictionary to store task statuses
tasks = {}

# Anthropic API client
def get_anthropic_client():
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        raise ValueError("ANTHROPIC_API_KEY environment variable not set")
    return Anthropic(api_key=api_key)

# Helper functions for web crawling
def is_valid_url(url: str, base_url: str) -> bool:
    """Check if a URL is valid and within the same domain."""
    parsed_url = urlparse(url)
    parsed_base = urlparse(base_url)
    
    # Check if it's a valid URL and not a fragment
    if not parsed_url.netloc and not parsed_url.path:
        return False
    
    # Check if it's within the same domain
    if parsed_url.netloc and parsed_url.netloc != parsed_base.netloc:
        return False
    
    # Ignore common non-documentation file types
    ignored_extensions = ['.jpg', '.jpeg', '.png', '.gif', '.css', '.js', '.svg', '.ico']
    if any(url.lower().endswith(ext) for ext in ignored_extensions):
        return False
    
    return True

def extract_text_from_html(html_content: str) -> str:
    """Extract readable text from HTML content."""
    soup = BeautifulSoup(html_content, 'html.parser')
    
    # Remove script and style elements
    for script_or_style in soup(['script', 'style', 'header', 'footer', 'nav']):
        script_or_style.decompose()
    
    # Get text
    text = soup.get_text()
    
    # Remove extra whitespace
    lines = (line.strip() for line in text.splitlines())
    chunks = (phrase.strip() for line in lines for phrase in line.split("  "))
    text = '\n'.join(chunk for chunk in chunks if chunk)
    
    return text

def crawl_website(base_url: str, max_pages: Optional[int] = None, depth: int = 1, task_id: str = None) -> List[Dict[str, str]]:
    """Crawl a website and extract content from its pages."""
    visited_urls = set()
    to_visit = [(base_url, 0)]  # (url, depth)
    documents = []
    
    # Update task status
    if task_id:
        tasks[task_id] = {
            "status": "in_progress",
            "progress": 0,
            "status_message": f"Starting to crawl {base_url}"
        }
    
    page_count = 0
    while to_visit and (max_pages is None or page_count < max_pages):
        current_url, current_depth = to_visit.pop(0)
        
        # Skip if already visited or exceeds depth
        if current_url in visited_urls or current_depth > depth:
            continue
        
        visited_urls.add(current_url)
        
        logger.info(f"Crawling: {current_url}")
        if task_id:
            tasks[task_id]["status_message"] = f"Crawling: {current_url}"
        
        try:
            response = requests.get(current_url, timeout=10)
            response.raise_for_status()
            
            # Extract text from the page
            page_text = extract_text_from_html(response.text)
            documents.append({
                "url": current_url,
                "content": page_text
            })
            
            page_count += 1
            
            # Update progress
            if task_id and max_pages:
                progress = min(100, (page_count / max_pages) * 100)
                tasks[task_id]["progress"] = progress
            
            # Continue crawling if depth limit not reached
            if current_depth < depth:
                soup = BeautifulSoup(response.text, 'html.parser')
                for link in soup.find_all('a', href=True):
                    href = link['href']
                    absolute_url = urljoin(current_url, href)
                    
                    if is_valid_url(absolute_url, base_url) and absolute_url not in visited_urls:
                        to_visit.append((absolute_url, current_depth + 1))
        
        except Exception as e:
            logger.error(f"Error crawling {current_url}: {str(e)}")
    
    # Update task status
    if task_id:
        tasks[task_id]["status_message"] = f"Completed crawling {page_count} pages"
        if max_pages:
            tasks[task_id]["progress"] = 100
    
    return documents

def extract_text_from_pdf(file_path: str) -> str:
    """Extract text from a PDF file."""
    with open(file_path, 'rb') as file:
        reader = PyPDF2.PdfReader(file)
        text = ""
        total_pages = len(reader.pages)
        
        for page_num in range(total_pages):
            text += reader.pages[page_num].extract_text() + "\n\n"
    
    return text

def chunk_text(text: str, max_chunk_size: int = 8000) -> List[str]:
    """Split text into manageable chunks."""
    chunks = []
    current_chunk = ""
    
    for paragraph in text.split("\n\n"):
        if len(current_chunk) + len(paragraph) < max_chunk_size:
            current_chunk += paragraph + "\n\n"
        else:
            if current_chunk:
                chunks.append(current_chunk.strip())
            current_chunk = paragraph + "\n\n"
    
    if current_chunk:
        chunks.append(current_chunk.strip())
    
    return chunks

def format_document_for_claude(documents: Union[List[Dict[str, str]], str], source_type: str, source_url: str = None) -> str:
    """Format document content for Claude's context."""
    if isinstance(documents, str):
        # Single text document (like PDF)
        context = f"### Documentation from {source_type}"
        if source_url:
            context += f": {source_url}"
        context += "\n\n"
        context += documents
    else:
        # Multiple web pages
        context = f"### Documentation crawled from {source_type}"
        if source_url:
            context += f": {source_url}"
        context += "\n\n"
        
        for doc in documents:
            context += f"## Page: {doc['url']}\n\n"
            context += doc['content']
            context += "\n\n---\n\n"
    
    return context

def process_with_claude(context: str, prompt: str = None, task_id: str = None) -> str:
    """Process the documentation with Claude and return the response."""
    try:
        client = get_anthropic_client()
        
        if prompt:
            user_message = prompt
        else:
            user_message = "Please summarize the key information from this documentation. Focus on the main concepts, functions, and usage patterns."
        
        if task_id:
            tasks[task_id]["status_message"] = "Sending to Claude API..."
        
        message = client.messages.create(
            model="claude-3-7-sonnet-20250219",
            max_tokens=4000,
            system=f"You are a helpful assistant that specializes in understanding and explaining documentation. Please help the user understand the provided documentation context. Here is the documentation you should reference:\n\n{context}",
            messages=[
                {"role": "user", "content": user_message}
            ]
        )
        
        return message.content[0].text
    
    except Exception as e:
        logger.error(f"Error processing with Claude: {str(e)}")
        raise HTTPException(status_code=500, detail=f"Error processing with Claude: {str(e)}")

# Routes
@app.get("/", response_class=HTMLResponse)
async def get_index(request: Request):
    return templates.TemplateResponse("index.html", {"request": request})

@app.post("/process_url")
async def process_url(
    background_tasks: BackgroundTasks,
    url: str = Form(...),
    max_pages: Optional[int] = Form(None),
    crawl_depth: Optional[int] = Form(1)
):
    # Generate a task ID
    task_id = f"url_{int(time.time())}"
    tasks[task_id] = {
        "status": "in_progress",
        "progress": 0,
        "status_message": "Starting task..."
    }
    
    # Start task in background
    background_tasks.add_task(process_url_task, url, max_pages, crawl_depth, task_id)
    
    return {"task_id": task_id}

async def process_url_task(url: str, max_pages: Optional[int], crawl_depth: int, task_id: str):
    try:
        # Crawl the website
        documents = crawl_website(url, max_pages, crawl_depth, task_id)
        
        # Format documents for Claude
        context = format_document_for_claude(documents, "website", url)
        
        # Process with Claude
        response = process_with_claude(context, task_id=task_id)
        
        # Update task status
        tasks[task_id] = {
            "status": "completed",
            "progress": 100,
            "response": response
        }
    
    except Exception as e:
        tasks[task_id] = {
            "status": "failed",
            "error": str(e)
        }

@app.post("/process_pdf")
async def process_pdf(
    background_tasks: BackgroundTasks,
    pdf_file: UploadFile = File(...)
):
    # Generate a task ID
    task_id = f"pdf_{int(time.time())}"
    tasks[task_id] = {
        "status": "in_progress",
        "progress": 0,
        "status_message": "Uploading PDF..."
    }
    
    # Start task in background
    background_tasks.add_task(process_pdf_task, pdf_file, task_id)
    
    return {"task_id": task_id}

async def process_pdf_task(pdf_file: UploadFile, task_id: str):
    try:
        # Save uploaded file to temp directory
        with tempfile.NamedTemporaryFile(delete=False, suffix=".pdf") as temp_file:
            content = await pdf_file.read()
            temp_file.write(content)
            temp_file_path = temp_file.name
        
        tasks[task_id]["status_message"] = "Extracting text from PDF..."
        tasks[task_id]["progress"] = 25
        
        # Extract text from PDF
        pdf_text = extract_text_from_pdf(temp_file_path)
        
        # Clean up temp file
        os.unlink(temp_file_path)
        
        tasks[task_id]["status_message"] = "Formatting text for Claude..."
        tasks[task_id]["progress"] = 50
        
        # Format text for Claude
        context = format_document_for_claude(pdf_text, "PDF", pdf_file.filename)
        
        tasks[task_id]["status_message"] = "Processing with Claude..."
        tasks[task_id]["progress"] = 75
        
        # Process with Claude
        response = process_with_claude(context)
        
        # Update task status
        tasks[task_id] = {
            "status": "completed",
            "progress": 100,
            "response": response
        }
    
    except Exception as e:
        tasks[task_id] = {
            "status": "failed",
            "error": str(e)
        }

@app.post("/custom_prompt")
async def custom_prompt(
    prompt: str = Form(...),
    context: Optional[str] = Form(None)
):
    try:
        # Process with Claude
        if context:
            response = process_with_claude(context, prompt)
        else:
            client = get_anthropic_client()
            message = client.messages.create(
                model="claude-3-7-sonnet-20250219",
                max_tokens=4000,
                messages=[
                    {"role": "user", "content": prompt}
                ]
            )
            response = message.content[0].text
        
        return {"response": response}
    
    except Exception as e:
        logger.error(f"Error processing custom prompt: {str(e)}")
        return {"error": str(e)}

@app.get("/task_status/{task_id}")
async def task_status(task_id: str):
    if task_id not in tasks:
        raise HTTPException(status_code=404, detail="Task not found")
    
    return tasks[task_id]

if __name__ == "__main__":
    uvicorn.run("app:app", host="0.0.0.0", port=8000, reload=True)
