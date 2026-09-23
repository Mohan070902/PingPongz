use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::crypto::{CryptoKeys, Fingerprint};
use crate::database::database::Database;

#[derive(Clone)]
pub struct IdentityManager {
    inner: Arc<Mutex<IdentityState>>,
}

struct IdentityState {
    pub hostname: String,
    pub nickname: Option<String>,
    pub keys: CryptoKeys,
}

impl IdentityManager {
    /// Load existing identity or generate a new one on first launch (FR-ID-01, FR-ID-02, FR-ID-03)
    pub fn load_or_create(conn: &Connection) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let detected_hostname = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .trim()
            .to_string();

        let hostname = if detected_hostname.is_empty() {
            "PingPong-Device".to_string()
        } else {
            detected_hostname
        };

        // Check if identity exists in database
        if let Some(saved) = Database::get_latest_identity(conn)? {
            // Check if saved private keys exist in settings
            let ed_priv = Database::get_setting(conn, "ed25519_privkey")?;
            let x_priv = Database::get_setting(conn, "x25519_privkey")?;

            if let (Some(ed_hex), Some(x_hex)) = (ed_priv, x_priv) {
                if let Ok(keys) = CryptoKeys::from_hex(&ed_hex, &x_hex) {
                    info!(
                        "Loaded existing identity: {} [{}]",
                        saved.hostname, keys.fingerprint
                    );
                    return Ok(Self {
                        inner: Arc::new(Mutex::new(IdentityState {
                            hostname: saved.hostname,
                            nickname: saved.nickname,
                            keys,
                        })),
                    });
                }
            }
        }

        // First launch or missing keys: generate exactly once (FR-ID-02)
        info!("First launch: generating new Ed25519 and X25519 keypairs");
        let keys = CryptoKeys::generate_new();
        let fingerprint = keys.fingerprint.clone();
        let ed_pub = keys.ed25519_pubkey_hex();
        let x_pub = keys.x25519_pubkey_hex();
        let ed_priv = keys.ed25519_privkey_hex();
        let x_priv = keys.x25519_privkey_hex();

        Database::save_identity(conn, &hostname, None, &fingerprint, &ed_pub, &x_pub)?;
        Database::set_setting(conn, "ed25519_privkey", &ed_priv)?;
        Database::set_setting(conn, "x25519_privkey", &x_priv)?;

        info!("New identity generated: {} [{}]", hostname, fingerprint);

        Ok(Self {
            inner: Arc::new(Mutex::new(IdentityState {
                hostname,
                nickname: None,
                keys,
            })),
        })
    }

    /// FR-ID-04 & FR-ID-06: Formatted display label
    pub fn display_label(&self) -> String {
        let state = self.inner.lock().unwrap();
        if let Some(ref nick) = state.nickname {
            if !nick.trim().is_empty() {
                return format!("{} ({}) [{}]", nick.trim(), state.hostname, state.keys.fingerprint);
            }
        }
        format!("{} [{}]", state.hostname, state.keys.fingerprint)
    }

    pub fn hostname(&self) -> String {
        self.inner.lock().unwrap().hostname.clone()
    }

    pub fn nickname(&self) -> Option<String> {
        self.inner.lock().unwrap().nickname.clone()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.inner.lock().unwrap().keys.fingerprint.clone()
    }

    pub fn ed25519_verifying_key_bytes(&self) -> [u8; 32] {
        *self.inner.lock().unwrap().keys.ed25519_verifying_key.as_bytes()
    }

    pub fn x25519_secret_bytes(&self) -> [u8; 32] {
        self.inner.lock().unwrap().keys.x25519_secret.to_bytes()
    }

    pub fn x25519_public_bytes(&self) -> [u8; 32] {
        *self.inner.lock().unwrap().keys.x25519_public.as_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.inner.lock().unwrap().keys.sign(message).to_bytes()
    }

    /// FR-ID-06: Optionally configure nickname
    pub fn set_nickname(
        &self,
        conn: &Connection,
        new_nick: Option<String>,
    ) -> Result<(), rusqlite::Error> {
        let mut state = self.inner.lock().unwrap();
        state.nickname = new_nick.clone();
        Database::update_nickname(conn, new_nick.as_deref())?;
        info!("Updated nickname to: {:?}", new_nick);
        Ok(())
    }

    /// FR-ID-10: Reset Identity (regenerate both key pairs, preserve message history)
    pub fn reset_identity(
        &self,
        conn: &Connection,
    ) -> Result<Fingerprint, Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.inner.lock().unwrap();
        warn!("Resetting identity for hostname: {}", state.hostname);

        let new_keys = CryptoKeys::generate_new();
        let new_fingerprint = new_keys.fingerprint.clone();
        let ed_pub = new_keys.ed25519_pubkey_hex();
        let x_pub = new_keys.x25519_pubkey_hex();
        let ed_priv = new_keys.ed25519_privkey_hex();
        let x_priv = new_keys.x25519_privkey_hex();

        Database::save_identity(
            conn,
            &state.hostname,
            state.nickname.as_deref(),
            &new_fingerprint,
            &ed_pub,
            &x_pub,
        )?;
        Database::set_setting(conn, "ed25519_privkey", &ed_priv)?;
        Database::set_setting(conn, "x25519_privkey", &x_priv)?;

        state.keys = new_keys;
        info!("Identity reset successfully. New fingerprint: {}", new_fingerprint);

        Ok(new_fingerprint)
    }

    /// FR-ID-08: Check if a known hostname reappears with a different fingerprint
    pub fn detect_identity_change(
        conn: &Connection,
        hostname: &str,
        new_fingerprint: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        let known_peers = Database::find_peer_by_hostname(conn, hostname)?;
        for peer in known_peers {
            if peer.fingerprint != new_fingerprint {
                // Same hostname, different fingerprint detected!
                return Ok(Some(peer.fingerprint));
            }
        }
        Ok(None)
    }
}