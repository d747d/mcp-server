#[cfg(test)]
mod integration_tests {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_server_process() {
        // Skip this test if it's running in CI environment
        if std::env::var("CI").is_ok() {
            println!("Skipping integration test in CI environment");
            return;
        }

        // Create a temp directory for config
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let config_path = temp_dir.path().join("test_config.yaml");
        
        // Create a test config with diagnostic tool enabled
        let config = r#"
server:
  name: "integration-test-server"
  version: "0.1.0"
  transport: "stdio"
  request_timeout: "10s"

docker:
  host: "unix:///var/run/docker.sock"
  read_only: true
  operation_timeout: "5s"

security:
  rate_limiting:
    enabled: true
    requests_per_minute: 60
    burst: 10
  
  quotas:
    enabled: true
    max_containers: 20
    max_images: 50
    "#;
        
        fs::write(&config_path, config).expect("Failed to write config");
        
        // Build the path to the binary
        let cargo_manifest_dir = env!("CARGO_MANIFEST_DIR");
        let binary_path = PathBuf::from(cargo_manifest_dir)
            .join("target")
            .join("debug")
            .join("docker-mcp-server");
        
        // Check if binary exists
        if !binary_path.exists() {
            println!("Binary not found at {:?}. Run 'cargo build' first.", binary_path);
            println!("Skipping integration test");
            return;
        }
        
        println!("Starting server process with binary: {:?}", binary_path);
        
        // Start the server process
        let mut process = Command::new(&binary_path)
            .arg("--config")
            .arg(&config_path)
            .arg("--verbose")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start server process");
        
        // Get handles
        let mut stdin = process.stdin.take().expect("Failed to get stdin");
        let mut stdout = process.stdout.take().expect("Failed to get stdout");
        let mut stderr = process.stderr.take().expect("Failed to get stderr");
        
        // Give the server time to initialize
        thread::sleep(Duration::from_secs(1));
        
        // Send initialize request
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        stdin.write_all(request.as_bytes()).expect("Failed to write request");
        stdin.write_all(b"\n").expect("Failed to write newline");
        stdin.flush().expect("Failed to flush stdin");
        
        // Read response with timeout
        let mut response = String::new();
        let mut buffer = [0; 4096];
        
        let start_time = std::time::Instant::now();
        let timeout = Duration::from_secs(5);
        
        while start_time.elapsed() < timeout {
            match stdout.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = std::str::from_utf8(&buffer[..n]).expect("Invalid UTF-8");
                    response.push_str(chunk);
                    if response.contains("\n") {
                        break;
                    }
                }
                Err(e) => {
                    panic!("Error reading from stdout: {}", e);
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        
        // Check if we got a response
        if response.is_empty() {
            // If no response, read stderr to see what went wrong
            let mut stderr_output = String::new();
            stderr.read_to_string(&mut stderr_output).expect("Failed to read stderr");
            
            println!("Server stderr output:");
            println!("{}", stderr_output);
            
            panic!("Server did not respond to initialize request");
        }
        
        // Verify response format
        assert!(response.contains("\"jsonrpc\":\"2.0\""), "Response missing jsonrpc version");
        assert!(response.contains("\"id\":1"), "Response missing or incorrect id");
        
        if !response.contains("\"capabilities\"") {
            panic!("Response doesn't contain expected capabilities: {}", response);
        }
        
        // Try to send a diagnostic request
        let diagnostic_request = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"diagnostic","arguments":{"check_docker":true,"check_compose":true}}}"#;
        stdin.write_all(diagnostic_request.as_bytes()).expect("Failed to write diagnostic request");
        stdin.write_all(b"\n").expect("Failed to write newline");
        stdin.flush().expect("Failed to flush stdin");
        
        // Read diagnostic response
        response.clear();
        
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed() < timeout {
            match stdout.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = std::str::from_utf8(&buffer[..n]).expect("Invalid UTF-8");
                    response.push_str(chunk);
                    if response.contains("\n") {
                        break;
                    }
                }
                Err(e) => {
                    panic!("Error reading from stdout: {}", e);
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        
        // Check diagnostic response
        println!("Diagnostic response: {}", response);
        
        // Kill the process
        process.kill().expect("Failed to kill server process");
        
        // Read any remaining stderr output
        let mut stderr_output = String::new();
        stderr.read_to_string(&mut stderr_output).expect("Failed to read stderr");
        
        if !stderr_output.is_empty() {
            println!("Server stderr output:");
            println!("{}", stderr_output);
        }
    }
}