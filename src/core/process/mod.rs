//! Process management module
//!
//! Track and manage running Minecraft instances.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};

/// A running Minecraft instance
pub struct RunningInstance {
    /// Instance name
    pub instance_name: String,
    /// Child process handle
    pub child: Child,
    /// When the process started
    pub started_at: DateTime<Utc>,
    /// Log buffer (limited to max lines)
    pub log_buffer: Vec<String>,
    /// Log file path
    pub log_file: Option<PathBuf>,
}

impl RunningInstance {
    /// Maximum lines to keep in memory
    const MAX_LOG_LINES: usize = 10_000;

    /// Create a new running instance
    pub fn new(instance_name: String, child: Child, log_file: Option<PathBuf>) -> Self {
        Self {
            instance_name,
            child,
            started_at: Utc::now(),
            log_buffer: Vec::new(),
            log_file,
        }
    }

    /// Add a log line
    pub fn add_log(&mut self, line: String) {
        // Write to log file if configured
        if let Some(ref log_path) = self.log_file {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = writeln!(file, "{}", line);
            }
        }

        // Add to buffer
        self.log_buffer.push(line);

        // Trim if too large
        if self.log_buffer.len() > Self::MAX_LOG_LINES {
            let drain_count = self.log_buffer.len() - Self::MAX_LOG_LINES;
            self.log_buffer.drain(0..drain_count);
        }
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Kill the process
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }

    /// Get process ID
    pub fn pid(&self) -> Option<u32> {
        Some(self.child.id())
    }
}

/// Manager for tracking running instances
#[derive(Default)]
pub struct ProcessManager {
    /// Running instances by name
    running: HashMap<String, RunningInstance>,
}

impl ProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a running instance
    pub fn register(&mut self, instance: RunningInstance) {
        self.running
            .insert(instance.instance_name.clone(), instance);
    }

    /// Kill a running instance by name
    pub fn kill(&mut self, name: &str) -> Result<()> {
        if let Some(instance) = self.running.get_mut(name) {
            instance.kill()?;
            self.running.remove(name);
            Ok(())
        } else {
            anyhow::bail!("Instance '{}' is not running", name)
        }
    }

    /// Check if an instance is running
    pub fn is_running(&mut self, name: &str) -> bool {
        if let Some(instance) = self.running.get_mut(name) {
            if instance.is_running() {
                return true;
            } else {
                // Process ended, remove it
                self.running.remove(name);
            }
        }
        false
    }

    /// Get logs for an instance
    pub fn get_logs(&self, name: &str) -> Option<&[String]> {
        self.running.get(name).map(|i| i.log_buffer.as_slice())
    }

    /// Get all running instance names
    pub fn list_running(&self) -> Vec<&str> {
        self.running.keys().map(|s| s.as_str()).collect()
    }

    /// Get instance info
    pub fn get_instance(&self, name: &str) -> Option<&RunningInstance> {
        self.running.get(name)
    }

    /// Get mutable instance
    pub fn get_instance_mut(&mut self, name: &str) -> Option<&mut RunningInstance> {
        self.running.get_mut(name)
    }

    /// Clean up finished processes
    pub fn cleanup(&mut self) {
        let mut finished: Vec<String> = Vec::new();

        for (name, inst) in self.running.iter_mut() {
            if !inst.is_running() {
                finished.push(name.clone());
            }
        }

        for name in finished {
            self.running.remove(&name);
        }
    }
}

/// Thread-safe process manager
pub type SharedProcessManager = Arc<Mutex<ProcessManager>>;

/// Create a shared process manager
pub fn create_shared_manager() -> SharedProcessManager {
    Arc::new(Mutex::new(ProcessManager::new()))
}

/// Log message types
#[derive(Debug, Clone)]
pub enum LogMessage {
    /// Standard output line
    Stdout(String),
    /// Standard error line
    Stderr(String),
    /// System message (from launcher)
    System(String),
    /// Process exited
    Exited(Option<i32>),
}

/// Start log capture thread for a child process
pub fn start_log_capture(instance_name: String, child: &mut Child, manager: SharedProcessManager) {
    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let name = instance_name.clone();
        let mgr = Arc::clone(&manager);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut m) = mgr.lock() {
                    if let Some(inst) = m.get_instance_mut(&name) {
                        inst.add_log(line);
                    }
                }
            }
        });
    }

    // Capture stderr
    if let Some(stderr) = child.stderr.take() {
        let name = instance_name.clone();
        let mgr = Arc::clone(&manager);
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut m) = mgr.lock() {
                    if let Some(inst) = m.get_instance_mut(&name) {
                        inst.add_log(format!("[ERR] {}", line));
                    }
                }
            }
        });
    }
}
