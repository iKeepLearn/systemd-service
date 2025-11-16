//! A library for Generate, installing, and managing
//! systemd services on Linux for Rust binary.
//!
//! This crate provides a fluent builder (`ServiceConfig`) to define a systemd
//! service unit and a `SystemdService` struct to handle the generation,
//! installation, and management (start, enable) of that service.
//!
//! ## ⚠️ Important: Requires Root Privileges
//!
//! Most operations in this crate (writing to `/etc/systemd/system`,
//! running `systemctl` commands) **require root privileges** to execute.
//! The methods will return an [`Error::Permission`] if run by a non-root user.
//!
//! ## Example Usage
//!
//! ```no_run
//!
//! use systemd_service::{ServiceConfig, SystemdService, Error, is_root};
//!
//! fn setup_my_service() -> Result<(), Error> {
//!     // 1. Define the service configuration using the builder
//!     let config = ServiceConfig::new(
//!         "myapp",
//!         "/usr/local/bin/myapp --run",
//!         "My Application Service",
//!     )
//!     .user("myapp-user")
//!     .group("myapp-group")
//!     .working_directory("/var/lib/myapp")
//!     .after(vec!["network.target".to_string()])
//!     .environment(vec![
//!         ("RUST_LOG".to_string(), "info".to_string()),
//!         ("PORT".to_string(), "8080".to_string()),
//!     ])
//!     .restart("on-failure")
//!     .restart_sec(10);
//!
//!     // 2. Create the service manager
//!     let service = SystemdService::new(config);
//!
//!     // 3. Install, enable, and reload systemd
//!     // This requires root privileges!
//!     service.install_and_enable()?;
//!
//!     // 4. Start the service
//!     // This also requires root privileges!
//!     service.start()?;
//!
//!     println!("Service 'myapp' installed and started successfully.");
//!     Ok(())
//! }
//! ```
//!

mod error;
mod utils;

pub use error::Error;
pub use utils::is_root;

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Represents the configuration for a systemd service.
///
/// Use [`ServiceConfig::new`] to create a basic configuration
/// and chain builder methods to set optional parameters.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// The name of the service (e.g., "myapp"). This is used for the .service filename.
    pub name: String,
    /// A brief description of the service (e.g., "My Application Service").
    pub description: String,
    /// The command to execute to start the service (e.g., "/usr/local/bin/myapp --daemon").
    pub exec_start: String,
    /// The working directory for the service process.
    pub working_directory: Option<String>,
    /// The user to run the service as.
    pub user: Option<String>,
    /// The group to run the service as.
    pub group: Option<String>,
    /// Restart policy (e.g., "no", "on-success", "on-failure", "always").
    pub restart: Option<String>,
    /// Delay (in seconds) before restarting the service.
    pub restart_sec: Option<u32>,
    /// The target to install this service under (usually "multi-user.target").
    pub wanted_by: Option<String>,
    /// Environment variables to set for the service (e.g., `vec![("RUST_LOG".to_string(), "info".to_string())]`).
    pub environment: Option<Vec<(String, String)>>,
    /// Services that must be started before this one (e.g., `vec!["network.target".to_string()]`).
    pub after: Option<Vec<String>>,
    /// File path to redirect `StandardOutput` to. `StandardError` is set to inherit.
    pub log_file: Option<String>,
}

/// Provides default values for `ServiceConfig`.
///
/// - `restart`: "always"
/// - `restart_sec`: 5
/// - `wanted_by`: "multi-user.target"
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            exec_start: String::new(),
            working_directory: None,
            user: None,
            group: None,
            restart: Some("always".to_string()),
            restart_sec: Some(5),
            wanted_by: Some("multi-user.target".to_string()),
            environment: None,
            after: None,
            log_file: None,
        }
    }
}

impl ServiceConfig {
    /// Creates a new `ServiceConfig` with the essential fields.
    ///
    /// All other fields are set to their default values.
    pub fn new(name: &str, exec_start: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            exec_start: exec_start.to_string(),
            ..Default::default()
        }
    }

    /// Sets the working directory for the service (builder method).
    pub fn working_directory(mut self, dir: &str) -> Self {
        self.working_directory = Some(dir.to_string());
        self
    }

    /// Sets the user for the service (builder method).
    pub fn user(mut self, user: &str) -> Self {
        self.user = Some(user.to_string());
        self
    }

    /// Sets the group for the service (builder method).
    pub fn group(mut self, group: &str) -> Self {
        self.group = Some(group.to_string());
        self
    }

    /// Sets the restart policy for the service (builder method).
    pub fn restart(mut self, restart: &str) -> Self {
        self.restart = Some(restart.to_string());
        self
    }

    /// Sets the restart delay (in seconds) for the service (builder method).
    pub fn restart_sec(mut self, sec: u32) -> Self {
        self.restart_sec = Some(sec);
        self
    }

    /// Sets the `WantedBy` target for the service (builder method).
    pub fn wanted_by(mut self, target: &str) -> Self {
        self.wanted_by = Some(target.to_string());
        self
    }

    /// Sets environment variables for the service (builder method).
    pub fn environment(mut self, env: Vec<(String, String)>) -> Self {
        self.environment = Some(env);
        self
    }

    /// Sets service dependencies (`After=`) (builder method).
    pub fn after(mut self, after: Vec<String>) -> Self {
        self.after = Some(after);
        self
    }

    /// Sets the log file path for `StandardOutput` (builder method).
    pub fn log_file(mut self, file_path: &str) -> Self {
        self.log_file = Some(file_path.to_string());
        self
    }
}

/// Manages a systemd service based on a [`ServiceConfig`].
///
/// This struct provides methods to generate the service file content,
/// write it to disk, and interact with `systemctl` to manage the service.
pub struct SystemdService {
    config: ServiceConfig,
}

impl SystemdService {
    /// Creates a new `SystemdService` from a given configuration.
    pub fn new(config: ServiceConfig) -> Self {
        SystemdService { config }
    }

    /// Generates the content of the .service unit file as a string.
    pub fn generate(&self) -> String {
        let mut content = String::new();

        // [Unit] section
        content.push_str("[Unit]\n");
        content.push_str(&format!("Description={}\n", self.config.description));

        if let Some(after) = &self.config.after
            && !after.is_empty()
        {
            content.push_str(&format!("After={}\n", after.join(" ")));
        }

        content.push('\n');

        // [Service] section
        content.push_str("[Service]\n");

        if let Some(working_directory) = &self.config.working_directory {
            content.push_str(&format!("WorkingDirectory={}\n", working_directory));
        }

        if let Some(user) = &self.config.user {
            content.push_str(&format!("User={}\n", user));
        }

        if let Some(group) = &self.config.group {
            content.push_str(&format!("Group={}\n", group));
        }

        if let Some(restart) = &self.config.restart {
            content.push_str(&format!("Restart={}\n", restart));
        }

        if let Some(restart_sec) = self.config.restart_sec {
            content.push_str(&format!("RestartSec={}\n", restart_sec));
        }

        content.push_str(&format!("ExecStart={}\n", self.config.exec_start));

        if let Some(log_file) = &self.config.log_file {
            content.push_str(&format!("StandardOutput=append:{}\n", log_file));
            content.push_str("StandardError=inherit\n");
        }

        if let Some(environment) = &self.config.environment
            && !environment.is_empty()
        {
            for (key, value) in environment {
                content.push_str(&format!("Environment=\"{}={}\"\n", key, value));
            }
        }
        content.push('\n');

        // [Install] section
        content.push_str("[Install]\n");
        if let Some(wanted_by) = &self.config.wanted_by {
            content.push_str(&format!("WantedBy={}\n", wanted_by));
        }

        content
    }

    /// Writes the generated service file content to the specified path.
    ///
    /// # Errors
    /// - [`Error::Permission`] if not run with root privileges.
    /// - [`Error::Io`] on file creation or write failures.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        validate_root_privileges()?;
        let content = self.generate();
        write_service_file(&content, path)
    }

    /// Installs, enables, and reloads the systemd daemon.
    ///
    /// This is the primary method for setting up a new service. It performs:
    /// 1. Writes the service file to `/etc/systemd/system/`.
    /// 2. Reloads the systemd daemon (`systemctl daemon-reload`).
    /// 3. Enables the service (`systemctl enable`).
    ///
    /// # Errors
    /// - [`Error::Permission`] if not run with root privileges.
    /// - [`Error::Io`] if the service file already exists or on write failures.
    /// - [`Error::Command`] if `systemctl` commands fail.
    pub fn install_and_enable(&self) -> Result<(), Error> {
        let path = self.get_service_file_path()?;
        let service_path = Path::new(&path);

        // 1. Write the file
        self.write(service_path)?;

        // 2. Reload systemd
        Self::reload_systemd()?;

        // 3. Enable the service
        self.enable()?;

        println!("Service '{}' 已安装并启用", self.config.name); // "Service '...' installed and enabled"
        Ok(())
    }

    /// Enables the service using `systemctl enable`.
    ///
    /// Assumes the service file already exists and systemd has been reloaded.
    /// Requires root privileges.
    fn enable(&self) -> Result<(), Error> {
        validate_root_privileges()?;
        let status = Command::new("systemctl")
            .arg("enable")
            .arg(&self.config.name)
            .status()?;

        if !status.success() {
            return Err(Error::Command(format!(
                "enable '{}' failed",
                self.config.name
            )));
        }

        println!("Service '{}' enabled", self.config.name);
        Ok(())
    }

    /// Starts the service using `systemctl start`.
    ///
    /// # Errors
    /// - [`Error::Permission`] if not run with root privileges.
    /// - [`Error::Command`] if the `systemctl start` command fails.
    pub fn start(&self) -> Result<(), Error> {
        validate_root_privileges()?;
        let status = Command::new("systemctl")
            .arg("start")
            .arg(&self.config.name)
            .status()?;

        if !status.success() {
            return Err(Error::Command(format!(
                "start '{}' failed",
                self.config.name
            )));
        }

        println!("Service '{}' start", self.config.name);
        Ok(())
    }

    /// Gets the conventional path for the service file (e.g., `/etc/systemd/system/myapp.service`).
    ///
    /// # Errors
    /// - [`Error::Io`] if the service file already exists at this path.
    fn get_service_file_path(&self) -> Result<String, Error> {
        let path = format!("/etc/systemd/system/{}.service", self.config.name);
        if Path::new(&path).exists() {
            // Fails if the file already exists to avoid overwriting.
            return Err(Error::Io("Service file exists".to_string()));
        }
        Ok(path)
    }

    /// Reloads the systemd daemon (`systemctl daemon-reload`).
    ///
    /// Requires root privileges.
    fn reload_systemd() -> Result<(), Error> {
        validate_root_privileges()?;
        let status = Command::new("systemctl").arg("daemon-reload").status()?;

        if !status.success() {
            return Err(Error::Command("systemctl daemon-reload failed".to_string()));
        }

        println!("systemd 已重新加载"); // "systemd has been reloaded"
        Ok(())
    }
}

/// Checks for root privileges and returns an error if not root.
///
/// # Errors
/// - [`Error::Permission`] if the check fails (i.e., not root).
pub fn validate_root_privileges() -> Result<(), Error> {
    if !is_root() {
        return Err(Error::Permission("need root privileges".to_string()));
    }
    Ok(())
}

/// Helper function to write the service file content to the specified path.
///
/// This function does *not* check for root privileges; it assumes
/// the caller (e.g., `SystemdService::write`) has already done so.
fn write_service_file(content: &str, path: &Path) -> Result<(), Error> {
    File::create(path)?.write_all(content.as_bytes())?;

    println!("Service file created: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_service_file() {
        let config = ServiceConfig::new(
            "myapp",
            "/usr/local/bin/myapp --daemon",
            "My Application Service",
        )
        .working_directory("/var/lib/myapp")
        .user("myapp")
        .group("myapp")
        .after(vec![
            "network.target".to_string(),
            "postgresql.service".to_string(),
        ])
        .environment(vec![
            ("RUST_LOG".to_string(), "info".to_string()),
            (
                "DATABASE_URL".to_string(),
                "postgresql://localhost/myapp".to_string(),
            ),
        ]);
        let systemd = SystemdService::new(config);
        let service_content = systemd.generate();
        println!("{}", service_content);

        assert!(service_content.contains("Description=My Application Service"));
        assert!(service_content.contains("ExecStart=/usr/local/bin/myapp --daemon"));
        assert!(service_content.contains("User=myapp"));
        assert!(service_content.contains("After=network.target postgresql.service"));
        assert!(service_content.contains("Environment=\"RUST_LOG=info\""));
    }

    #[test]
    fn test_minimal_service() {
        let config = ServiceConfig::new("minimal", "Minimal Service", "/usr/bin/sleep infinity");

        let systemd = SystemdService::new(config);
        let service_content = systemd.generate();
        println!("{}", service_content);

        assert!(service_content.contains("Description=Minimal Service"));
        assert!(service_content.contains("ExecStart=/usr/bin/sleep infinity"));
        assert!(service_content.contains("Restart=always")); // default value
        assert!(service_content.contains("WantedBy=multi-user.target")); // default value
    }

    #[test]
    fn test_root_check() {
        let result = is_root();
        eprintln!("is root:{}", result);
    }
}
