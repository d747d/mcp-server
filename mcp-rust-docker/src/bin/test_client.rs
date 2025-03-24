use clap::{App, Arg, SubCommand};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

static REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

struct McpClient {
    server_process: Child,
    stdin: std::process::ChildStdin,
    stdout_reader: BufReader<std::process::ChildStdout>,
}

impl McpClient {
    fn new(server_path: &str, config_path: Option<&str>) -> Result<Self, String> {
        let mut cmd = Command::new(server_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Pass stderr through for debugging

        if let Some(config) = config_path {
            cmd.arg("--config").arg(config);
        }

        cmd.arg("--verbose"); // Enable verbose logging

        println!("Starting server: {:?}", cmd);

        let mut server_process = cmd.spawn()
            .map_err(|e| format!("Failed to start server: {}", e))?;

        let stdin = server_process.stdin.take()
            .ok_or_else(|| "Failed to open stdin".to_string())?;

        let stdout = server_process.stdout.take()
            .ok_or_else(|| "Failed to open stdout".to_string())?;

        let stdout_reader = BufReader::new(stdout);

        // Small delay to let the server initialize
        std::thread::sleep(Duration::from_millis(500));

        Ok(McpClient {
            server_process,
            stdin,
            stdout_reader,
        })
    }

    fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
        
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let request_str = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        println!("Sending request: {}", request_str);

        // Write request to server stdin
        self.stdin.write_all(request_str.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Failed to send request: {}", e))?;

        // Read response
        let mut response_line = String::new();
        self.stdout_reader.read_line(&mut response_line)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        println!("Received response: {}", response_line);

        let response: Value = serde_json::from_str(&response_line)
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        // Check for error
        if let Some(error) = response.get("error") {
            return Err(format!("Server returned error: {}", error));
        }

        Ok(response)
    }

    fn initialize(&mut self) -> Result<Value, String> {
        self.send_request("initialize", None)
    }

    fn list_tools(&mut self) -> Result<Value, String> {
        self.send_request("tools/list", None)
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.send_request("tools/call", Some(json!({
            "name": name,
            "arguments": arguments
        })))
    }

    fn list_resources(&mut self) -> Result<Value, String> {
        self.send_request("resources/list", None)
    }

    fn read_resource(&mut self, uri: &str) -> Result<Value, String> {
        self.send_request("resources/read", Some(json!({
            "uri": uri
        })))
    }

    fn list_prompts(&mut self) -> Result<Value, String> {
        self.send_request("prompts/list", None)
    }

    fn get_prompt(&mut self, name: &str, arguments: Option<Value>) -> Result<Value, String> {
        let mut params = json!({
            "name": name
        });

        if let Some(args) = arguments {
            params["arguments"] = args;
        }

        self.send_request("prompts/get", Some(params))
    }

    fn run_diagnostics(&mut self) -> Result<Value, String> {
        self.call_tool("diagnostic", json!({
            "check_docker": true,
            "check_compose": true,
            "list_env_vars": true
        }))
    }

    fn close(mut self) -> Result<(), String> {
        match self.server_process.kill() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to kill server process: {}", e)),
        }
    }
}

fn main() -> Result<(), String> {
    let matches = App::new("Docker MCP Test Client")
    .version("0.1.0")
    .author("Your Name <your.email@example.com>")
    .about("Test client for Docker MCP server")
    .arg(Arg::with_name("server")
        .short('s')  // Changed from .short("s")
        .long("server")
        .value_name("PATH")
        .help("Path to the server executable")
        .required(true))
    .arg(Arg::with_name("config")
        .short('c')  // Changed from .short("c")
        .long("config")
        .value_name("PATH")
        .help("Path to the server config file"))
    .subcommand(SubCommand::with_name("initialize")
        .about("Initialize the server"))
    .subcommand(SubCommand::with_name("list-tools")
        .about("List available tools"))
    .subcommand(SubCommand::with_name("list-resources")
        .about("List available resources"))
    .subcommand(SubCommand::with_name("list-prompts")
        .about("List available prompts"))
    .subcommand(SubCommand::with_name("diagnostics")
        .about("Run server diagnostics"))
    .subcommand(SubCommand::with_name("containers")
        .about("List containers")
        .arg(Arg::with_name("all")
            .short('a')  // Changed from .short("a")
            .long("all")
            .help("Show all containers (default shows just running)")))
    .subcommand(SubCommand::with_name("images")
        .about("List images")
        .arg(Arg::with_name("all")
            .short('a')  // Changed from .short("a")
            .long("all")
            .help("Show all images (default hides intermediate images)")))
    .subcommand(SubCommand::with_name("logs")
        .about("Show container logs")
        .arg(Arg::with_name("container")
            .help("Container ID or name")
            .required(true))
        .arg(Arg::with_name("tail")
            .short('t')  // Changed from .short("t")
            .long("tail")
            .takes_value(true)
            .help("Number of lines to show from the end of the logs")))
    .subcommand(SubCommand::with_name("start")
        .about("Start container")
        .arg(Arg::with_name("container")
            .help("Container ID or name")
            .required(true)))
    .subcommand(SubCommand::with_name("stop")
        .about("Stop container")
        .arg(Arg::with_name("container")
            .help("Container ID or name")
            .required(true)))
    .get_matches();

    let server_path = matches.value_of("server").unwrap();
    let config_path = matches.value_of("config");

    // Create client and initialize the server
    let mut client = McpClient::new(server_path, config_path)?;
    
    // Handle specific subcommands
    if let Some(_) = matches.subcommand_matches("initialize") {
        let response = client.initialize()?;
        println!("Server initialized: {}", response);
    } else if let Some(_) = matches.subcommand_matches("list-tools") {
        let response = client.list_tools()?;
        println!("Available tools: {}", response);
    } else if let Some(_) = matches.subcommand_matches("list-resources") {
        let response = client.list_resources()?;
        println!("Available resources: {}", response);
    } else if let Some(_) = matches.subcommand_matches("list-prompts") {
        let response = client.list_prompts()?;
        println!("Available prompts: {}", response);
    } else if let Some(_) = matches.subcommand_matches("diagnostics") {
        let response = client.run_diagnostics()?;
        println!("Diagnostics: {}", response);
    } else if let Some(cmd) = matches.subcommand_matches("containers") {
        let all = cmd.is_present("all");
        let response = client.call_tool("list-containers", json!({
            "all": all
        }))?;
        println!("Containers: {}", response);
    } else if let Some(cmd) = matches.subcommand_matches("images") {
        let all = cmd.is_present("all");
        let response = client.call_tool("list-images", json!({
            "all": all
        }))?;
        println!("Images: {}", response);
    } else if let Some(cmd) = matches.subcommand_matches("logs") {
        let container = cmd.value_of("container").unwrap();
        let tail = cmd.value_of("tail").unwrap_or("all");
        let response = client.call_tool("container-logs", json!({
            "container_id": container,
            "tail": tail
        }))?;
        println!("Logs: {}", response);
    } else if let Some(cmd) = matches.subcommand_matches("start") {
        let container = cmd.value_of("container").unwrap();
        let response = client.call_tool("container-start", json!({
            "container_id": container
        }))?;
        println!("Start result: {}", response);
    } else if let Some(cmd) = matches.subcommand_matches("stop") {
        let container = cmd.value_of("container").unwrap();
        let response = client.call_tool("container-stop", json!({
            "container_id": container
        }))?;
        println!("Stop result: {}", response);
    } else {
        // Default: run initialization and diagnostics
        client.initialize()?;
        let response = client.run_diagnostics()?;
        println!("Diagnostics: {}", response);
    }

    client.close()?;
    
    Ok(())
}