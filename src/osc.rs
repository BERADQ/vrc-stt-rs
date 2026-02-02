use std::net::{ToSocketAddrs, UdpSocket};

use rosc::{OscMessage, OscPacket, OscType, encoder};

use crate::config::CONFIG;

pub fn connect(addr: impl ToSocketAddrs) -> anyhow::Result<VRCMessageOSC> {
    let socket = UdpSocket::bind(addr)?;
    Ok(VRCMessageOSC::new(socket))
}

pub struct VRCMessageOSC {
    socket: UdpSocket,
}

impl VRCMessageOSC {
    pub fn new(stream: UdpSocket) -> Self {
        VRCMessageOSC { socket: stream }
    }

    pub fn send_message(&mut self, message: &str) -> anyhow::Result<()> {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/chatbox/input".to_owned(),
            args: vec![
                OscType::String(message.to_owned()),
                OscType::Bool(true),
                OscType::Bool(false),
            ],
        }))?;
        self.socket.send_to(&msg_buf, &CONFIG.udp.to)?;
        Ok(())
    }

    pub fn send_set_typing(&mut self, is_typing: bool) -> anyhow::Result<()> {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/chatbox/typing".to_owned(),
            args: vec![OscType::Bool(is_typing)],
        }))?;
        self.socket.send_to(&msg_buf, &CONFIG.udp.to)?;
        Ok(())
    }
}
