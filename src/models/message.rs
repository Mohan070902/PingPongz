use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_MESSAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

impl std::fmt::Display for MessageDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageDirection::Inbound => write!(f, "inbound"),
            MessageDirection::Outbound => write!(f, "outbound"),
        }
    }
}

impl std::str::FromStr for MessageDirection {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inbound" => Ok(MessageDirection::Inbound),
            "outbound" => Ok(MessageDirection::Outbound),
            _ => Err(format!("Unknown direction: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Pending,
    Sent,
    Failed,
}

impl std::fmt::Display for MessageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageStatus::Pending => write!(f, "pending"),
            MessageStatus::Sent => write!(f, "sent"),
            MessageStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for MessageStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(MessageStatus::Pending),
            "sent" => Ok(MessageStatus::Sent),
            "failed" => Ok(MessageStatus::Failed),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub msg_id: String,
    pub peer_fingerprint: String,
    pub sender_pubkey: String,
    pub direction: MessageDirection,
    pub content: String,
    pub status: MessageStatus,
    pub timestamp_ms: i64,
}

impl Message {
    pub fn new_outbound(
        peer_fingerprint: String,
        sender_pubkey: String,
        content: String,
    ) -> Result<Self, String> {
        if content.as_bytes().len() > MAX_MESSAGE_SIZE {
            return Err(format!(
                "Message size exceeds {} bytes (got {} bytes)",
                MAX_MESSAGE_SIZE,
                content.as_bytes().len()
            ));
        }

        Ok(Self {
            msg_id: Uuid::new_v4().to_string(),
            peer_fingerprint,
            sender_pubkey,
            direction: MessageDirection::Outbound,
            content,
            status: MessageStatus::Pending,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        })
    }

    pub fn new_inbound(
        msg_id: String,
        peer_fingerprint: String,
        sender_pubkey: String,
        content: String,
        timestamp_ms: i64,
    ) -> Result<Self, String> {
        if content.as_bytes().len() > MAX_MESSAGE_SIZE {
            return Err(format!(
                "Incoming message size exceeds {} bytes (got {} bytes)",
                MAX_MESSAGE_SIZE,
                content.as_bytes().len()
            ));
        }

        Ok(Self {
            msg_id,
            peer_fingerprint,
            sender_pubkey,
            direction: MessageDirection::Inbound,
            content,
            status: MessageStatus::Sent,
            timestamp_ms,
        })
    }
}
