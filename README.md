# systemd-service

 A library for Generate, installing, and managing
 systemd services on Linux for Rust binary.

 This crate provides a fluent builder (`ServiceConfig`) to define a systemd
 service unit and a `SystemdService` struct to handle the generation,
 installation, and management (start, enable) of that service.

 ## ⚠️ Important: Requires Root Privileges

 Most operations in this crate (writing to `/etc/systemd/system`,
 running `systemctl` commands) **require root privileges** to execute.
 The methods will return an [`Error::Permission`] if run by a non-root user.

 ## Example Usage

 ```rust

 use systemd_service::{ServiceConfig, SystemdService, Error, is_root};

 fn setup_my_service() -> Result<(), Error> {
     // 1. Define the service configuration using the builder
     let config = ServiceConfig::new(
         "myapp",
         "/usr/local/bin/myapp --run",
         "My Application Service",
     )
     .user("myapp-user")
     .group("myapp-group")
     .working_directory("/var/lib/myapp")
     .after(vec!["network.target".to_string()])
     .environment(vec![
         ("RUST_LOG".to_string(), "info".to_string()),
         ("PORT".to_string(), "8080".to_string()),
     ])
     .restart("on-failure")
     .restart_sec(10);

     // 2. Create the service manager
     let service = SystemdService::new(config);

     // 3. Install, enable, and reload systemd
     // This requires root privileges!
     service.install_and_enable()?;

     // 4. Start the service
     // This also requires root privileges!
     service.start()?;

     println!("Service 'myapp' installed and started successfully.");
     Ok(())
 }
```
