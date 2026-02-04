use std::io::{BufRead, BufReader, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(target_os = "windows")]
use uds_windows::{UnixListener, UnixStream};

use common::SocketMessage;
use rust_i18n::t;

/// Messages that can be sent from backend to frontend
#[derive(Clone, Debug)]
pub enum FrontendMessage {
    /// Backend connection established
    BackendConnected,
    /// Backend connection error/disconnected
    BackendDisconnected,
    /// STT recording started
    STTRecordStart,
    /// STT recording ended, processing transcribed text
    STTRecordProcessing,
    /// STT recording ended successfully with transcribed text
    STTRecordEndWithText(String),
    /// STT recording ended with an error
    STTRecordEndWithError(String),
}

/// Socket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Listening,
}

/// Unix socket server that listens for a single backend connection
pub struct SocketServer {
    socket_path: PathBuf,
    msg_tx: Sender<FrontendMessage>,
    connection_state: Arc<Mutex<ConnectionState>>,
    stop_signal: Arc<AtomicBool>,
    listener_handle: Option<JoinHandle<()>>,
}

impl SocketServer {
    /// Create a new socket server
    pub fn new(socket_path: impl Into<PathBuf>, msg_tx: Sender<FrontendMessage>) -> Self {
        Self {
            socket_path: socket_path.into(),
            msg_tx,
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            listener_handle: None,
        }
    }

    /// Start the socket server (blocking until server is ready)
    pub fn start(&mut self) -> anyhow::Result<()> {
        // Remove old socket file if it exists
        if std::path::Path::new(&self.socket_path).exists() {
            // Try to connect to existing socket to see if it's still active
            match UnixStream::connect(&self.socket_path) {
                Ok(_) => {
                    // Socket is still active, don't delete it
                    return Err(anyhow::anyhow!(
                        "Socket file {} is already in use by another process",
                        self.socket_path.display()
                    ));
                }
                Err(_) => {
                    // Socket is not active, safe to delete
                    log::info!("{}", t!("socket.remove.stale", path = self.socket_path.display()));
                    if let Err(e) = std::fs::remove_file(&self.socket_path) {
                        log::warn!("{}", t!("socket.remove.failed", error = e));
                    }
                }
            }
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        log::info!("{}", t!("socket.server.listening", path = self.socket_path.display()));

        let msg_tx = self.msg_tx.clone();
        let stop_signal = self.stop_signal.clone();
        let connection_state = self.connection_state.clone();
        let socket_path_clone = self.socket_path.clone();

        let handle = thread::spawn(move || {
            *connection_state.lock().unwrap() = ConnectionState::Listening;

            // Accept only one connection (single backend)
            for stream in listener.incoming() {
                if stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                match stream {
                    Ok(stream) => {
                        log::info!("{}", t!("socket.backend.connected"));
                        *connection_state.lock().unwrap() = ConnectionState::Connected;

                        if let Err(e) = msg_tx.send(FrontendMessage::BackendConnected) {
                            log::error!("{}", t!("socket.send.message.error", error = e));
                            break;
                        }

                        // Handle this connection until it closes
                        if let Err(e) =
                            Self::handle_connection(stream, msg_tx.clone(), stop_signal.clone())
                        {
                            log::debug!("{}", t!("socket.handler.ended", error = e));
                        }

                        *connection_state.lock().unwrap() = ConnectionState::Disconnected;

                        if let Err(e) = msg_tx.send(FrontendMessage::BackendDisconnected) {
                            log::error!("{}", t!("socket.send.message.error", error = e));
                        }

                        // Wait a bit before accepting new connection to avoid busy looping
                        std::thread::sleep(Duration::from_millis(100));
                        log::info!("{}", t!("socket.waiting.new.connection"));
                    }
                    Err(e) => {
                        log::error!("{}", t!("socket.connection.error.general", error = e));
                    }
                }
            }

            log::info!("{}", t!("socket.server.stopped"));

            // Clean up socket file
            if socket_path_clone.exists() {
                log::info!("{}", t!("socket.remove.file", path = socket_path_clone.display()));
                if let Err(e) = std::fs::remove_file(&socket_path_clone) {
                    log::warn!("{}", t!("socket.remove.failed", error = e));
                }
            }
        });

        self.listener_handle = Some(handle);
        Ok(())
    }

    /// Stop the server
    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);

        // Connect to ourselves to wake up the listener
        let _ = UnixStream::connect(&self.socket_path);

        // Wait for the listener thread to finish
        if let Some(handle) = self.listener_handle.take() {
            if let Err(_) = handle.join() {
                log::warn!("{}", t!("socket.thread.join.failed"));
            }
        }

        // Clean up socket file
        if std::path::Path::new(&self.socket_path).exists() {
            log::info!("{}", t!("socket.remove.file", path = self.socket_path.display()));
            if let Err(e) = std::fs::remove_file(&self.socket_path) {
                log::warn!("{}", t!("socket.remove.failed", error = e));
            }
        }
    }

    /// Handle a single connection from backend
    fn handle_connection(
        stream: UnixStream,
        msg_tx: Sender<FrontendMessage>,
        stop_signal: Arc<AtomicBool>,
    ) -> anyhow::Result<()> {
        // Set read timeout to allow periodic checking of stop signal
        stream.set_read_timeout(Some(Duration::from_millis(100)))?;
        stream.set_nonblocking(false)?;

        let reader = BufReader::new(&stream);

        // Spawn a thread to handle outgoing messages
        let writer_stop_signal = stop_signal.clone();
        let writer_handle = thread::spawn(move || {
            loop {
                // Check stop signal periodically
                if writer_stop_signal.load(Ordering::SeqCst) {
                    break;
                }

                // In this simplified version, we just sleep and check for stop signal
                // Messages would need to be passed through a channel for a full implementation
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        // Read incoming messages
        for line in reader.lines() {
            if stop_signal.load(Ordering::SeqCst) {
                log::info!("{}", t!("socket.stop.received"));
                break;
            }

            match line {
                Ok(json) => {
                    match SocketMessage::from_json(&json) {
                        Ok(SocketMessage::STTRecordStart) => {
                            if let Err(e) = msg_tx.send(FrontendMessage::STTRecordStart) {
                                log::error!("{}", t!("socket.message.send.error", error = e));
                                break;
                            }
                        }
                        Ok(SocketMessage::STTRecordProcessing) => {
                            if let Err(e) = msg_tx.send(FrontendMessage::STTRecordProcessing) {
                                log::error!("{}", t!("socket.message.send.error", error = e));
                                break;
                            }
                        }
                        Ok(SocketMessage::STTRecordEndWithText(text)) => {
                            if let Err(e) = msg_tx.send(FrontendMessage::STTRecordEndWithText(text))
                            {
                                log::error!("{}", t!("socket.message.send.error", error = e));
                                break;
                            }
                        }
                        Ok(SocketMessage::STTRecordEndWithError(error)) => {
                            if let Err(e) =
                                msg_tx.send(FrontendMessage::STTRecordEndWithError(error))
                            {
                                log::error!("{}", t!("socket.message.send.error", error = e));
                                break;
                            }
                        }
                        Ok(SocketMessage::Ping) => {
                            // Ignore ping messages
                        }
                        Ok(SocketMessage::Pong) => {
                            // Ignore pong messages
                        }
                        Err(e) => {
                            log::warn!("{}", t!("socket.message.parse.error", error = e));
                        }
                    }
                }
                Err(e) => {
                    if stop_signal.load(Ordering::SeqCst) {
                        log::info!("{}", t!("socket.read.graceful.exit"));
                        break;
                    }

                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        log::debug!("{}", t!("socket.read.error", error = e));
                        let _ = msg_tx.send(FrontendMessage::BackendDisconnected);
                        break;
                    }
                }
            }
        }

        // Wait for writer thread to finish
        drop(writer_handle);

        log::debug!("{}", t!("socket.handler.exited"));
        Ok(())
    }

    fn _send_message(stream: &mut UnixStream, message: &SocketMessage) -> anyhow::Result<()> {
        let json = message.to_json()?;
        stream.write_all(json.as_bytes())?;
        stream.flush()?;
        Ok(())
    }
}
