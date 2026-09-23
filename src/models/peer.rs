use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    Unknown,
    Discovered,
    Connecting,
    Online,
    Offline,
}

impl std::fmt::Display for PeerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerState::Unknown => write!(f, "Unknown"),
            PeerState::Discovered => write!(f, "Discovered"),
            PeerState::Connecting => write!(f, "Connecting"),
            PeerState::Online => write!(f, "Online"),
            PeerState::Offline => write!(f, "Offline"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustStatus {
    Trusted,
    Blocked,
    IdentityChanged,
}

impl std::fmt::Display for TrustStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustStatus::Trusted => write!(f, "Trusted"),
            TrustStatus::Blocked => write!(f, "Blocked"),
            TrustStatus::IdentityChanged => write!(f, "Identity Changed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub fingerprint: String,
    pub hostname: String,
    pub nickname: Option<String>,
    pub last_ip: String,
    pub tcp_port: u16,
    pub state: PeerState,
    pub trust_status: TrustStatus,
    pub first_seen: i64,
    pub last_seen: i64,
    pub previous_fingerprint: Option<String>,
}

impl Peer {
    pub fn new(fingerprint: String, hostname: String, last_ip: String, tcp_port: u16) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            fingerprint,
            hostname,
            nickname: None,
            last_ip,
            tcp_port,
            state: PeerState::Discovered,
            trust_status: TrustStatus::Trusted,
            first_seen: now,
            last_seen: now,
            previous_fingerprint: None,
        }
    }

    pub fn display_label(&self) -> String {
        if let Some(ref nick) = self.nickname {
            format!("{} ({}) [{}]", nick, self.hostname, self.fingerprint)
        } else {
            format!("{} [{}]", self.hostname, self.fingerprint)
        }
    }
}
