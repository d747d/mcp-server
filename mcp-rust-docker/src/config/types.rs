use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSettings,
    pub docker: DockerSettings,
    pub security: SecuritySettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub transport: TransportType,
    #[serde(with = "humantime_serde", default = "default_request_timeout")]
    pub request_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    Stdio,
    Sse,
}

impl Default for TransportType {
    fn default() -> Self {
        TransportType::Stdio
    }
}

impl Default for DockerSettings {
    fn default() -> Self {
        Self {
            host: default_docker_host(),
            api_version: None,
            allowed_compose_projects: None,
            compose_path: default_compose_path(),
            operation_timeout: default_operation_timeout(),
            read_only: false,
            max_log_size: default_max_log_size(),
        }
    }
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            rate_limiting: RateLimitSettings {
                enabled: default_true(),
                requests_per_minute: default_rate_limit(),
                burst: default_burst_limit(),
            },
            quotas: QuotaSettings {
                enabled: default_true(),
                max_containers: default_max_containers(),
                max_images: default_max_images(),
                max_build_time: default_max_build_time(),
                max_log_size: default_max_log_size(),
            },
            registries: RegistrySettings {
                allowed_registries: None,
                denied_registries: std::collections::HashSet::new(),
                allowed_base_images: None,
                denied_base_images: std::collections::HashSet::new(),
            },
            volumes: VolumeSettings {
                allowed_mounts: None,
                denied_mounts: std::collections::HashSet::new(),
            },
            networks: NetworkSettings {
                allowed_networks: None,
                denied_networks: std::collections::HashSet::new(),
            },
            commands: CommandSettings {
                allowed_commands: None,
                denied_commands: std::collections::HashSet::new(),
            },
        }
    }
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            log_requests: default_true(),
            audit_logging: default_true(),
            audit_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerSettings {
    /// Docker socket path or TCP endpoint
    #[serde(default = "default_docker_host")]
    pub host: String,
    /// Docker API version to use
    pub api_version: Option<String>,
    /// Specific Docker Compose projects allowed
    #[serde(default)]
    pub allowed_compose_projects: Option<HashSet<String>>,
    /// Path to docker-compose binary
    #[serde(default = "default_compose_path")]
    pub compose_path: PathBuf,
    /// Default timeout for Docker operations
    #[serde(with = "humantime_serde", default = "default_operation_timeout")]
    pub operation_timeout: Duration,
    /// Whether to enable read-only mode (prevents modifications)
    #[serde(default)]
    pub read_only: bool,
    /// Maximum log size to return in bytes
    #[serde(default = "default_max_log_size")]
    pub max_log_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// Rate limiting settings
    pub rate_limiting: RateLimitSettings,
    /// Resource quota settings
    pub quotas: QuotaSettings,
    /// Image registry controls
    pub registries: RegistrySettings,
    /// Volume mount restrictions
    pub volumes: VolumeSettings,
    /// Network access restrictions
    pub networks: NetworkSettings,
    /// Command execution restrictions
    pub commands: CommandSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitSettings {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of requests per minute
    #[serde(default = "default_rate_limit")]
    pub requests_per_minute: u32,
    /// Burst allowance above the base rate
    #[serde(default = "default_burst_limit")]
    pub burst: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaSettings {
    /// Enable resource quotas
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of containers
    #[serde(default = "default_max_containers")]
    pub max_containers: u32,
    /// Maximum number of images
    #[serde(default = "default_max_images")]
    pub max_images: u32,
    /// Maximum build time in seconds
    #[serde(with = "humantime_serde", default = "default_max_build_time")]
    pub max_build_time: Duration,
    /// Maximum log size to return in bytes
    #[serde(default = "default_max_log_size")]
    pub max_log_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySettings {
    /// List of allowed image registries (if empty, all are allowed)
    #[serde(default)]
    pub allowed_registries: Option<HashSet<String>>,
    /// List of denied image registries
    #[serde(default)]
    pub denied_registries: HashSet<String>,
    /// List of allowed base images (if empty, all are allowed)
    #[serde(default)]
    pub allowed_base_images: Option<HashSet<String>>,
    /// List of denied base images
    #[serde(default)]
    pub denied_base_images: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSettings {
    /// List of allowed volume mount paths
    #[serde(default)]
    pub allowed_mounts: Option<HashSet<String>>,
    /// List of denied volume mount paths
    #[serde(default)]
    pub denied_mounts: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    /// List of allowed networks
    #[serde(default)]
    pub allowed_networks: Option<HashSet<String>>,
    /// List of denied networks
    #[serde(default)]
    pub denied_networks: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSettings {
    /// List of allowed Docker commands
    #[serde(default)]
    pub allowed_commands: Option<HashSet<String>>,
    /// List of denied Docker commands
    #[serde(default)]
    pub denied_commands: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Log format (json or text)
    #[serde(default = "default_log_format")]
    pub format: String,
    /// Path to log file (if None, logs to stderr)
    pub file: Option<PathBuf>,
    /// Whether to enable request logging
    #[serde(default = "default_true")]
    pub log_requests: bool,
    /// Whether to enable audit logging for security events
    #[serde(default = "default_true")]
    pub audit_logging: bool,
    /// Path to audit log file (if None, audit logs go to regular log)
    pub audit_file: Option<PathBuf>,
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_request_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_operation_timeout() -> Duration {
    Duration::from_secs(60)
}

fn default_docker_host() -> String {
    if cfg!(unix) {
        "unix:///var/run/docker.sock".to_string()
    } else {
        "npipe:////./pipe/docker_engine".to_string()
    }
}

fn default_compose_path() -> PathBuf {
    "docker-compose".into()
}

fn default_rate_limit() -> u32 {
    60
}

fn default_burst_limit() -> u32 {
    10
}

fn default_max_containers() -> u32 {
    20
}

fn default_max_images() -> u32 {
    50
}

fn default_max_build_time() -> Duration {
    Duration::from_secs(300) // 5 minutes
}

fn default_max_log_size() -> usize {
    1024 * 1024 // 1 MB
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}