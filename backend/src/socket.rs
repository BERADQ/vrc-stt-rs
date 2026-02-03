use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::SocketMessage;
use rust_i18n::t;

/// Unix socket client that connects to the frontend with automatic reconnection
pub struct SocketClient {
    socket_path: String,
    connection: Arc<Mutex<Option<UnixStream>>>,
    stop_signal: Arc<AtomicBool>,
    connection_handle: Option<JoinHandle<()>>,
}

impl SocketClient {
    /// Create a new socket client
    pub fn new(socket_path: String) -> Self {
        Self {
            socket_path,
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

                log::info!("{}", t!("socket.connect.attempt", path = socket_path));

                match UnixStream::connect(&socket_path) {
                    Ok(stream) => {
                        log::info!("{}", t!("socket.connected", path = socket_path));

                        // Store connection
                        *connection.lock().unwrap() = Some(stream.try_clone().unwrap());

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
                        *connection.lock().unwrap() = None;
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

                // Exponential backoff
                let start = std::time::Instant::now();
                while start.elapsed() < delay && !stop_signal.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(50));
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

    /// Send message to frontend - returns Ok(()) if not connected (message is dropped)
    pub fn send(&self, message: &SocketMessage) -> anyhow::Result<()> {
        let mut conn = self.connection.lock().unwrap();

        if let Some(stream) = conn.as_mut() {
            let json = message.to_json()?;
            match stream.write_all(json.as_bytes()) {
                Ok(_) => {
                    if let Err(e) = stream.flush() {
                        log::debug!("{}", t!("socket.flush.error", error = e));
                        // Connection might be broken, clear it
                        *conn = None;
                    }
                }
                Err(e) => {
                    log::debug!("{}", t!("socket.send.error", error = e));
                    // Connection is broken, clear it
                    *conn = None;
                }
            }
        }
        // If not connected, message is silently dropped - this is expected behavior
        Ok(())
    }

    /// Handle a connection - read responses (currently just keeps connection alive)
    fn handle_connection(stream: &UnixStream, stop_signal: Arc<AtomicBool>) -> anyhow::Result<()> {
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;

        let reader = BufReader::new(stream);

        // Read incoming messages (frontend might send commands in the future)
        for line in reader.lines() {
            if stop_signal.load(Ordering::SeqCst) {
                log::info!("{}", t!("socket.disconnecting"));
                break;
            }

            match line {
                Ok(_json) => {
                    // Currently frontend doesn't send messages, but we could handle them here
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

/// Legacy wrapper for backward compatibility with old code
pub struct SocketServer {
    client: SocketClient,
}

impl SocketServer {
    /// Create a new socket "server" (now a client that connects to frontend)
    pub fn new() -> anyhow::Result<(Self, Receiver<SocketMessage>)> {
        let socket_path = std::env::var("VRC_STT_SOCKET")
            .unwrap_or_else(|_| common::default_socket_path().to_string());

        let mut client = SocketClient::new(socket_path);

        // Start the client immediately
        client.start()?;

        // Create a dummy receiver for compatibility (old API expected this)
        let (_, broadcast_rx) = channel::<SocketMessage>();

        let server = Self { client };
        Ok((server, broadcast_rx))
    }

    /// Start method for compatibility (does nothing, client already started)
    pub fn start(&self) -> anyhow::Result<JoinHandle<()>> {
        // Client is already started in new(), return a dummy handle
        let handle = thread::spawn(|| {});
        Ok(handle)
    }

    /// Send message to frontend - renamed from broadcast for clarity
    /// Silently drops message if not connected (no error spam)
    pub fn broadcast(&self, message: SocketMessage) -> anyhow::Result<()> {
        self.client.send(&message)?;
        Ok(())
    }

    /// Run broadcaster - kept for compatibility but not needed
    pub fn run_broadcaster(&self, _rx: Receiver<SocketMessage>) -> JoinHandle<()> {
        thread::spawn(|| {})
    }
}
