//! OSC (Open Sound Control) communication with VRChat
//!
//! Sends chat messages and typing indicators to VRChat via UDP OSC.

use std::net::UdpSocket;

use rosc::{OscMessage, OscPacket, OscType, encoder};

use common::config::Config;

/// VRChat OSC message sender
pub struct VrcOsc {
    socket: UdpSocket,
    target_addr: String,
}

impl VrcOsc {
    /// Create a new OSC client from configuration
    pub fn from_config(config: &Config) -> anyhow::Result<Self> {
        let bind_addr: (&str, u16) = ("0.0.0.0", config.udp.port);
        let socket = UdpSocket::bind(bind_addr)?;

        Ok(Self {
            socket,
            target_addr: config.udp.to.clone(),
        })
    }

    /// Send a chat message to VRChat
    pub fn send_message(&mut self, message: &str) -> anyhow::Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: "/chatbox/input".to_owned(),
            args: vec![
                OscType::String(message.to_owned()),
                OscType::Bool(true),  // send true - actually send the message
                OscType::Bool(false), // display large message
            ],
        });

        let msg_buf = encoder::encode(&packet)?;
        self.socket.send_to(&msg_buf, &self.target_addr)?;
        Ok(())
    }

    /// Set typing indicator state
    pub fn set_typing(&mut self, is_typing: bool) -> anyhow::Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: "/chatbox/typing".to_owned(),
            args: vec![OscType::Bool(is_typing)],
        });

        let msg_buf = encoder::encode(&packet)?;
        self.socket.send_to(&msg_buf, &self.target_addr)?;
        Ok(())
    }
}
