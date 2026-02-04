use std::net::{ToSocketAddrs, UdpSocket};

use rosc::{OscMessage, OscPacket, OscType, encoder};

use common::config::Config;

pub fn connect_with_config(
    addr: impl ToSocketAddrs,
    config: &Config,
) -> anyhow::Result<VRCMessageOSC> {
    let socket = UdpSocket::bind(addr)?;
    Ok(VRCMessageOSC::new(socket, config.udp.to.clone()))
}

pub struct VRCMessageOSC {
    socket: UdpSocket,
    target_addr: String,
}

impl VRCMessageOSC {
    pub fn new(stream: UdpSocket, target_addr: String) -> Self {
        VRCMessageOSC {
            socket: stream,
            target_addr,
        }
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
        self.socket.send_to(&msg_buf, &self.target_addr)?;
        Ok(())
    }

    pub fn send_set_typing(&mut self, is_typing: bool) -> anyhow::Result<()> {
        let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/chatbox/typing".to_owned(),
            args: vec![OscType::Bool(is_typing)],
        }))?;
        self.socket.send_to(&msg_buf, &self.target_addr)?;
        Ok(())
    }
}
