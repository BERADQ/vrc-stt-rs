//! Unix socket communication with frontend
//!
//! Provides a client that connects to the frontend's Unix socket server
//! for bi-directional communication.

use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(target_os = "windows")]
use uds_windows::UnixStream;

use common::SocketMessage;
use rust_i18n::t;

/// Unix socket client with automatic reconnection
pub struct SocketClient {
    socket_path: PathBuf,
    connection: Arc<Mutex<Option<UnixStream>>>,
    stop_signal: Arc<AtomicBool>,
    connection_handle: Option<JoinHandle<()>>,
}

impl SocketClient {
    /// Create a new socket client
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            connection: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            connection_handle: None,
        }
    }

    /// Start the socket client with automatic reconnection
    pub fn start(&mut self) -> anyhow::Result<()> {
        let socket_path = self.socket_path.clone();
        let stop_signal = self.stop_signal.clone();
        let connection = self.connection.clone();

        let handle = thread::spawn(move || {
            let mut delay = Duration::from_millis(500);
            let max_delay = Duration::from_secs(30);

            loop {
                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                log::info!("{}", t!("socket.connect.attempt", path = socket_path.display()));

                match UnixStream::connect(&socket_path) {
                    Ok(stream) => {
                        log::info!("{}", t!("socket.connected", path = socket_path.display()));

                        // Store connection
                        if let Ok(mut conn) = connection.lock() {
                            *conn = stream.try_clone().ok();
                        }

                        // Handle connection
                        match Self::handle_connection(&stream, stop_signal.clone()) {
                            Ok(_) => {
                                log::info!("{}", t!("socket.connection.closed"));
                            }
                            Err(e) => {
                                log::debug!("{}", t!("socket.connection.error", error = e));
                            }
                        }

                        // Clear connection
                        if let Ok(mut conn) = connection.lock() {
                            *conn = None;
                        }
                        delay = Duration::from_millis(500);
                    }
                    Err(e) => {
                        log::debug!(
                            "{}",
                            t!(
                                "socket.connection.retry",
                                error = e,
                                delay_ms = delay.as_millis()
                            )
                        );
                    }
                }

                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                // Exponential backoff with interrupt check
                let start = std::time::Instant::now();
                while start.elapsed() < delay && !stop_signal.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(50));
                }

                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                delay = std::cmp::min(delay * 2, max_delay);
            }

            log::info!("{}", t!("socket.client.stopped"));
        });

        self.connection_handle = Some(handle);
        Ok(())
    }

    /// Send message to frontend
    /// 
    /// Silently drops the message if not connected (no error spam)
    pub fn send(&self, message: &SocketMessage) -> anyhow::Result<()> {
        let mut conn = self.connection.lock().unwrap();

        if let Some(stream) = conn.as_mut() {
            let json = message.to_json()?;
            match stream.write_all(json.as_bytes()) {
                Ok(_) => {
                    if let Err(e) = stream.flush() {
                        log::debug!("{}", t!("socket.flush.error", error = e));
                        *conn = None;
                    }
                }
                Err(e) => {
                    log::debug!("{}", t!("socket.send.error", error = e));
                    *conn = None;
                }
            }
        }
        // If not connected, message is silently dropped
        Ok(())
    }

    /// Handle a connection - read incoming messages
    fn handle_connection(
        stream: &UnixStream,
        stop_signal: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;

        let reader = BufReader::new(stream);

        for line in reader.lines() {
            if stop_signal.load(Ordering::SeqCst) {
                log::info!("{}", t!("socket.disconnecting"));
                break;
            }

            match line {
                Ok(_json) => {
                    // Currently frontend doesn't send commands
                    // Commands can be handled here in the future
                }
                Err(e) => {
                    if stop_signal.load(Ordering::SeqCst) {
                        break;
                    }

                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        return Err(e.into());
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for SocketClient {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        if let Some(handle) = self.connection_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Create socket client with default path
pub fn create_default_client() -> anyhow::Result<SocketClient> {
    let socket_path = std::env::var("VRC_STT_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| common::default_socket_path());

    let mut client = SocketClient::new(socket_path);
    client.start()?;
    Ok(client)
}
