use serde::{Deserialize, Serialize};

pub const PACKET_HANDSHAKE_1: u8 = 0x01;
pub const PACKET_HANDSHAKE_2: u8 = 0x02;
pub const PACKET_HANDSHAKE_3: u8 = 0x03;
pub const PACKET_TRANSPORT: u8 = 0x04;

pub const INNER_PING: u8 = 0x10;
pub const INNER_PONG: u8 = 0x11;
pub const INNER_MESSAGE: u8 = 0x20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireMessagePayload {
    pub msg_id: String,
    pub sender_pubkey: String,
    pub timestamp_ms: i64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportPacket {
    Ping,
    Pong,
    Message(WireMessagePayload),
}

impl TransportPacket {
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}
