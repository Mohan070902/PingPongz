use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityModel {
    pub id: i64,
    pub hostname: String,
    pub nickname: Option<String>,
    pub fingerprint: String,
    pub ed25519_pubkey: String,
    pub x25519_pubkey: String,
    pub created_at: i64,
}

impl IdentityModel {
    pub fn display_label(&self) -> String {
        if let Some(ref nick) = self.nickname {
            if !nick.trim().is_empty() {
                return format!("{} ({}) [{}]", nick.trim(), self.hostname, self.fingerprint);
            }
        }
        format!("{} [{}]", self.hostname, self.fingerprint)
    }
}