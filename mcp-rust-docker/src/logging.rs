use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;
use once_cell::sync::Lazy;

// Define structured log entry
#[derive(Debug, Serialize, Deserialize)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
    request_id: Option<String>,
    method: Option<String>,
    details: Option<String>,
    error_code: Option<i32>,
    duration_ms: Option<u64>,
}

// Define error logger
pub struct ErrorLogger {
    file: Option<Mutex<File>>,
    console_output: bool,
    log_requests: bool,
    start_time: Instant,
}

// Global instance
static ERROR_LOGGER: Lazy<Mutex<ErrorLogger>> = Lazy::new(|| {
    Mutex::new(ErrorLogger {
        file: None,
        console_output: true,
        log_requests: true,
        start_time: Instant::now(),
    })
});

impl ErrorLogger {
    pub fn init(log_file: Option<&Path>, console_output: bool, log_requests: bool) -> Result<(), String> {
        let mut logger = ERROR_LOGGER.lock().unwrap();
        
        logger.console_output = console_output;
        logger.log_requests = log_requests;
        
        if let Some(path) = log_file {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("Failed to open log file: {}", e))?;
                
            logger.file = Some(Mutex::new(file));
        }
        
        Ok(())
    }
    
    pub fn log_error(
        level: &str,
        message: &str,
        request_id: Option<&str>,
        method: Option<&str>,
        details: Option<&str>,
        error_code: Option<i32>,
    ) {
        let logger = ERROR_LOGGER.lock().unwrap();
        
        let now = Local::now();
        let entry = LogEntry {
            timestamp: now.to_rfc3339(),
            level: level.to_string(),
            message: message.to_string(),
            request_id: request_id.map(String::from),
            method: method.map(String::from),
            details: details.map(String::from),
            error_code,
            duration_ms: Some(logger.start_time.elapsed().as_millis() as u64),
        };
        
        // Log to file if configured
        if let Some(file_mutex) = &logger.file {
            if let Ok(mut file) = file_mutex.lock() {
                if let Ok(json) = serde_json::to_string(&entry) {
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
        
        // Log to console if enabled
        if logger.console_output {
            let log_prefix = match level {
                "ERROR" => "\x1b[31mERROR\x1b[0m",   // Red
                "WARN"  => "\x1b[33mWARN\x1b[0m",    // Yellow
                "INFO"  => "\x1b[32mINFO\x1b[0m",    // Green
                _       => level,
            };
            
            let id_str = request_id.map_or(String::new(), |id| format!("[req:{}] ", id));
            let method_str = method.map_or(String::new(), |m| format!("[{}] ", m));
            
            eprintln!("{} {} {}{}{}", 
                now.format("%Y-%m-%d %H:%M:%S%.3f"),
                log_prefix,
                id_str,
                method_str,
                message
            );
            
            if let Some(details) = details {
                if !details.is_empty() {
                    eprintln!("  Details: {}", details);
                }
            }
        }
    }
    
    pub fn log_request_start(id: &str, method: &str) {
        if ERROR_LOGGER.lock().unwrap().log_requests {
            Self::log_error("INFO", &format!("Request started"), Some(id), Some(method), None, None);
        }
    }
    
    pub fn log_request_end(id: &str, method: &str, success: bool, error_code: Option<i32>, error_message: Option<&str>) {
        if ERROR_LOGGER.lock().unwrap().log_requests {
            let status = if success { "succeeded" } else { "failed" };
            Self::log_error(
                if success { "INFO" } else { "ERROR" },
                &format!("Request {}", status),
                Some(id),
                Some(method),
                error_message,
                error_code
            );
        }
    }
    
    pub fn log_docker_error(message: &str, details: Option<&str>) {
        Self::log_error("ERROR", &format!("Docker error: {}", message), None, None, details, None);
    }
    
    pub fn log_security_violation(message: &str, details: Option<&str>) {
        Self::log_error("WARN", &format!("Security violation: {}", message), None, None, details, None);
    }
}

// Add this to your McpServer implementation
impl McpServer {
    // Add a method to improve error logging
    fn log_request(&self, request: &JsonRpcRequest, response: &JsonRpcResponse) {
        let id = match &request.id {
            JsonRpcId::Null => "null".to_string(),
            JsonRpcId::String(s) => s.clone(),
            JsonRpcId::Number(n) => n.to_string(),
        };
        
        let success = response.error.is_none();
        let error_code = response.error.as_ref().map(|e| e.code);
        let error_message = response.error.as_ref().map(|e| e.message.as_str());
        
        ErrorLogger::log_request_end(&id, &request.method, success, error_code, error_message);
    }
    
    // Modify your process_request method to use the logger
    pub async fn process_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Log request start
        let id_str = match &request.id {
            JsonRpcId::Null => "null".to_string(),
            JsonRpcId::String(s) => s.clone(),
            JsonRpcId::Number(n) => n.to_string(),
        };
        
        ErrorLogger::log_request_start(&id_str, &request.method);
        
        // Apply rate limiting
        if let Err(e) = self.rate_limiter.check() {
            let response = self.error_response(request.id, e);
            self.log_request(&request, &response);
            return response;
        }

        let response = match request.method.as_str() {
            // Existing method handlers...
            _ => self.error_response(
                request.id,
                McpError::MethodNotFound(format!("Method '{}' not found", request.method)),
            ),
        };
        
        // Log request completion
        self.log_request(&request, &response);
        
        response
    }
}