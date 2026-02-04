use std::env::current_dir;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

use rust_i18n::t;

/// Log message types for backend logs
#[derive(Debug, Clone)]
pub enum BackendLogMessage {
    /// Internal info messages about backend management
    InternalInfo(String),
    /// Internal error messages about backend management
    InternalError(String),
    /// Output from the backend process (stdout)
    BackendOutput(String),
    /// Logs from the backend process (stderr)
    BackendLog(String),
}

/// Manages the lifecycle of the backend process
pub struct BackendManager {
    /// The running backend process
    backend_process: Option<Child>,
    /// Channel sender for log messages
    log_tx: Option<mpsc::Sender<BackendLogMessage>>,
    /// Channel receiver for log messages
    log_rx: Option<mpsc::Receiver<BackendLogMessage>>,
}

impl BackendManager {
    /// Creates a new BackendManager
    pub fn new() -> Self {
        let (log_tx, log_rx) = mpsc::channel();
        Self {
            backend_process: None,
            log_tx: Some(log_tx),
            log_rx: Some(log_rx),
        }
    }

    /// Processes available log messages
    pub fn process_logs(&mut self) -> Vec<BackendLogMessage> {
        let mut logs = Vec::new();
        if let Some(rx) = &self.log_rx {
            while let Ok(log_msg) = rx.try_recv() {
                logs.push(log_msg);
            }
        }
        logs
    }

    /// Starts the backend process
    /// The backend executable path can be overridden with the VRC_STT_BACKEND environment variable
    pub fn start_backend(&mut self) -> anyhow::Result<()> {
        // If there's already a running backend, kill it first
        self.stop_backend();

        // Get backend executable path from environment variable or use default
        let backend_path =
            std::env::var("VRC_STT_BACKEND").unwrap_or_else(|_| "backend".to_string());

        // Send info log
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(BackendLogMessage::InternalInfo(format!(
                "Starting backend process: {}",
                backend_path
            )));
        }
        log::info!("{}", t!("backend.starting", path = backend_path));

        let mut command = Command::new(&backend_path);

        if !std::env::var("RUST_LOG").is_ok_and(|e| !e.is_empty()) {
            command.env("RUST_LOG", "warn");
        }

        #[cfg(all(target_os = "windows", feature = "cuda"))]
        {
            use std::env::current_exe;

            command.env("CUDA_PATH", current_exe()?);
            command.env("LD_LIBRARY_PATH", current_exe()?);
        }

        // Start the backend process with captured stdout and stderr
        let mut child = command
            .current_dir(&std::env::current_dir()?)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                // Send error log
                if let Some(tx) = &self.log_tx {
                    let _ = tx.send(BackendLogMessage::InternalError(format!(
                        "Failed to start backend '{}': {}",
                        backend_path, e
                    )));
                }
                anyhow::Error::new(e).context(format!("Failed to start backend '{}'", backend_path))
            })?;

        log::info!("{}", t!("backend.started.pid", pid = child.id()));

        // Capture stdout and stderr in separate threads
        let child_stdout = child.stdout.take().expect("Failed to take stdout");
        let child_stderr = child.stderr.take().expect("Failed to take stderr");
        let log_tx_clone = self.log_tx.clone();

        // Spawn thread to read stdout
        let stdout_tx = log_tx_clone.clone();
        thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(child_stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Some(tx) = &stdout_tx {
                        let _ = tx.send(BackendLogMessage::BackendOutput(line));
                    }
                }
            }
        });

        // Spawn thread to read stderr - this contains logs from backend process
        let stderr_tx = log_tx_clone;
        thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(child_stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if let Some(tx) = &stderr_tx {
                        // Send as BackendLog to indicate these are logs from the backend process
                        let _ = tx.send(BackendLogMessage::BackendLog(line));
                    }
                }
            }
        });

        self.backend_process = Some(child);

        Ok(())
    }

    /// Stops the backend process if it's running
    pub fn stop_backend(&mut self) {
        if let Some(mut process) = self.backend_process.take() {
            if let Some(tx) = &self.log_tx {
                let _ = tx.send(BackendLogMessage::InternalInfo(format!(
                    "Stopping backend process with PID: {}",
                    process.id()
                )));
            }
            log::info!("{}", t!("backend.stopping.pid", pid = process.id()));

            // Try to terminate the process
            if let Err(e) = process.kill() {
                if let Some(tx) = &self.log_tx {
                    let _ = tx.send(BackendLogMessage::InternalError(format!(
                        "Failed to kill backend process: {}",
                        e
                    )));
                }
                log::warn!("{}", t!("backend.kill.failed", error = e));
            }

            // Wait for the process to exit
            match process.wait() {
                Ok(status) => {
                    if let Some(tx) = &self.log_tx {
                        let _ = tx.send(BackendLogMessage::InternalInfo(format!(
                            "Backend process exited with status: {:?}",
                            status
                        )));
                    }
                    log::info!("{}", t!("backend.exited.status", status = status));
                }
                Err(e) => {
                    if let Some(tx) = &self.log_tx {
                        let _ = tx.send(BackendLogMessage::InternalError(format!(
                            "Failed to wait for backend process: {}",
                            e
                        )));
                    }
                    log::warn!("{}", t!("backend.wait.failed", error = e));
                }
            }
        }
    }

    /// Restarts the backend process
    pub fn restart_backend(&mut self) -> anyhow::Result<()> {
        if let Some(tx) = &self.log_tx {
            let _ = tx.send(BackendLogMessage::InternalInfo(
                "Restarting backend process...".to_string(),
            ));
        }
        log::info!("{}", t!("backend.restart.process"));
        self.stop_backend();
        self.start_backend()
    }
}

impl Drop for BackendManager {
    /// Automatically stop the backend when the manager is dropped
    fn drop(&mut self) {
        self.stop_backend();
    }
}
