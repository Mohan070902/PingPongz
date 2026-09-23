use serde::{Deserialize, Serialize};

pub const BEACON_MAGIC: [u8; 4] = *b"RUBX";
pub const BEACON_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryBeacon {
    pub magic: [u8; 4],
    pub version: u16,
    pub tcp_port: u16,
    pub fingerprint: String,
    pub hostname: String,
}

impl DiscoveryBeacon {
    pub fn new(tcp_port: u16, fingerprint: String, hostname: String) -> Self {
        Self {
            magic: BEACON_MAGIC,
            version: BEACON_VERSION,
            tcp_port,
            fingerprint,
            hostname,
        }
    }

    /// Serialize beacon to bytes using bincode
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// FR-PD-04: Parse beacon and discard/ignore invalid magic, unsupported version, or malformed structure
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err("Packet too short to be a valid beacon".to_string());
        }

        let beacon: DiscoveryBeacon = match bincode::deserialize(bytes) {
            Ok(b) => b,
            Err(e) => return Err(format!("Malformed beacon structure: {}", e)),
        };

        if beacon.magic != BEACON_MAGIC {
            return Err(format!(
                "Invalid magic: expected {:?}, got {:?}",
                BEACON_MAGIC, beacon.magic
            ));
        }

        if beacon.version != BEACON_VERSION {
            return Err(format!(
                "Unsupported version: expected {}, got {}",
                BEACON_VERSION, beacon.version
            ));
        }

        if beacon.fingerprint.trim().is_empty() || beacon.hostname.trim().is_empty() {
            return Err("Beacon missing required fingerprint or hostname".to_string());
        }

        Ok(beacon)
    }
}
