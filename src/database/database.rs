use std::path::Path;
use rusqlite::{params, Connection, Result};
use tracing::{error, info, warn};

use crate::database::schema::Tables;
use crate::models::{IdentityModel, Message, MessageDirection, MessageStatus, Peer, PeerState, TrustStatus};

pub struct Database;

impl Database {
    pub const DEFAULT_DB_PATH: &'static str = "pingpongzzz.db";

    /// Resolves the database file path safely, ensuring writable storage regardless of current working directory
    pub fn resolve_db_path() -> std::path::PathBuf {
        // 1. Explicit override via env var
        if let Ok(path) = std::env::var("PINGPONGZZZ_DB") {
            return std::path::PathBuf::from(path);
        }

        // 2. Check if a local project database exists and is accessible
        let project_db = std::path::PathBuf::from("/mnt/e/Projects/PingPongzzz/pingpongzzz.db");
        if project_db.exists() {
            return project_db;
        }

        // 3. Check current working directory if not a protected system path
        if let Ok(cur_dir) = std::env::current_dir() {
            let cur_str = cur_dir.to_string_lossy();
            let is_system_dir = cur_str.contains("/Windows/") || cur_str.contains("\\Windows\\") || cur_str == "/";
            if !is_system_dir {
                let test_file = cur_dir.join(".pingpongzzz_write_test");
                if std::fs::write(&test_file, b"").is_ok() {
                    let _ = std::fs::remove_file(&test_file);
                    return cur_dir.join(Self::DEFAULT_DB_PATH);
                }
            }
        }

        // 4. User data directory in Linux / WSL
        if let Ok(home) = std::env::var("HOME") {
            let data_dir = std::path::PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("pingpongzzz");
            let _ = std::fs::create_dir_all(&data_dir);
            return data_dir.join(Self::DEFAULT_DB_PATH);
        }

        // 5. Windows AppData fallback
        if let Ok(appdata) = std::env::var("APPDATA") {
            let data_dir = std::path::PathBuf::from(appdata).join("PingPongzzz");
            let _ = std::fs::create_dir_all(&data_dir);
            return data_dir.join(Self::DEFAULT_DB_PATH);
        }

        std::path::PathBuf::from(Self::DEFAULT_DB_PATH)
    }

    /// Initialize SQLite connection, enable WAL mode, perform integrity check (FR-DB-01, FR-DB-06, FR-DB-07)
    pub fn initialize_with_path<P: AsRef<Path>>(path: P) -> Result<Connection> {
        let path = path.as_ref();
        info!("Opening SQLite database at {:?}", path);

        // Ensure parent directory exists if path has parents
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        // Pre-flight check: if database file exists, run integrity check
        if path.exists() {
            if let Err(e) = Self::run_integrity_check(path) {
                error!("Database integrity check failed: {}. Recovering...", e);
                Self::handle_corrupt_database(path)?;
            }
        }

        let connection = Connection::open(path)?;

        // FR-DB-01: Enable Write-Ahead Logging (WAL) mode and normal synchrony
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;

        Tables::create_all(&connection)?;
        info!("Database initialized and tables verified");

        Ok(connection)
    }

    pub fn initialize() -> Result<Connection> {
        Self::initialize_with_path(Self::resolve_db_path())
    }

    /// FR-DB-06: On startup, run PRAGMA integrity_check on the local database
    pub fn run_integrity_check(path: &Path) -> Result<()> {
        let conn = Connection::open(path)?;
        let mut stmt = conn.prepare("PRAGMA integrity_check;")?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let result: String = row.get(0)?;
            if result == "ok" {
                info!("Database integrity check passed (ok)");
                return Ok(());
            } else {
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                    Some(format!("Integrity check reported: {}", result)),
                ));
            }
        }

        Ok(())
    }

    /// FR-DB-07: If corruption is detected, rename corrupt file to database.corrupt, create fresh database, notify user
    fn handle_corrupt_database(path: &Path) -> Result<()> {
        let corrupt_path = path.with_extension("db.corrupt");
        warn!(
            "Corrupt database detected at {:?}. Renaming to {:?}",
            path, corrupt_path
        );

        if let Err(e) = std::fs::rename(path, &corrupt_path) {
            error!("Failed to rename corrupt database: {}", e);
        }

        // Also clean up any WAL/SHM companion files if present
        let wal_path = format!("{}-wal", path.display());
        let shm_path = format!("{}-shm", path.display());
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(shm_path);

        info!("Creating fresh database at {:?}", path);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Identity Persistence (FR-DB-02)
    // -------------------------------------------------------------------------

    pub fn get_latest_identity(connection: &Connection) -> Result<Option<IdentityModel>> {
        let mut stmt = connection.prepare(
            "SELECT id, hostname, nickname, fingerprint, ed25519_pubkey, x25519_pubkey, created_at 
             FROM identity ORDER BY id DESC LIMIT 1;",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(IdentityModel {
                id: row.get(0)?,
                hostname: row.get(1)?,
                nickname: row.get(2)?,
                fingerprint: row.get(3)?,
                ed25519_pubkey: row.get(4)?,
                x25519_pubkey: row.get(5)?,
                created_at: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn save_identity(
        connection: &Connection,
        hostname: &str,
        nickname: Option<&str>,
        fingerprint: &str,
        ed25519_pubkey: &str,
        x25519_pubkey: &str,
    ) -> Result<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        connection.execute(
            "INSERT INTO identity (hostname, nickname, fingerprint, ed25519_pubkey, x25519_pubkey, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
            params![hostname, nickname, fingerprint, ed25519_pubkey, x25519_pubkey, now],
        )?;

        Ok(connection.last_insert_rowid())
    }

    pub fn update_nickname(connection: &Connection, nickname: Option<&str>) -> Result<()> {
        connection.execute(
            "UPDATE identity SET nickname = ?1 WHERE id = (SELECT id FROM identity ORDER BY id DESC LIMIT 1);",
            params![nickname],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Peers Persistence (FR-DB-03)
    // -------------------------------------------------------------------------

    pub fn upsert_peer(connection: &Connection, peer: &Peer) -> Result<()> {
        connection.execute(
            "INSERT INTO peers (fingerprint, hostname, nickname, last_ip, tcp_port, status, trust_status, first_seen, last_seen, previous_fingerprint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(fingerprint) DO UPDATE SET
                hostname = excluded.hostname,
                nickname = COALESCE(excluded.nickname, peers.nickname),
                last_ip = excluded.last_ip,
                tcp_port = excluded.tcp_port,
                status = excluded.status,
                last_seen = excluded.last_seen;",
            params![
                peer.fingerprint,
                peer.hostname,
                peer.nickname,
                peer.last_ip,
                peer.tcp_port,
                peer.state.to_string(),
                peer.trust_status.to_string(),
                peer.first_seen,
                peer.last_seen,
                peer.previous_fingerprint,
            ],
        )?;

        Ok(())
    }

    pub fn get_peer(connection: &Connection, fingerprint: &str) -> Result<Option<Peer>> {
        let mut stmt = connection.prepare(
            "SELECT fingerprint, hostname, nickname, last_ip, tcp_port, status, trust_status, first_seen, last_seen, previous_fingerprint
             FROM peers WHERE fingerprint = ?1;",
        )?;

        let mut rows = stmt.query(params![fingerprint])?;
        if let Some(row) = rows.next()? {
            let status_str: String = row.get(5)?;
            let trust_str: String = row.get(6)?;

            let state = match status_str.as_str() {
                "Online" => PeerState::Online,
                "Connecting" => PeerState::Connecting,
                "Discovered" => PeerState::Discovered,
                _ => PeerState::Offline,
            };

            let trust_status = match trust_str.as_str() {
                "Blocked" => TrustStatus::Blocked,
                "Identity Changed" => TrustStatus::IdentityChanged,
                _ => TrustStatus::Trusted,
            };

            Ok(Some(Peer {
                fingerprint: row.get(0)?,
                hostname: row.get(1)?,
                nickname: row.get(2)?,
                last_ip: row.get(3)?,
                tcp_port: row.get(4)?,
                state,
                trust_status,
                first_seen: row.get(7)?,
                last_seen: row.get(8)?,
                previous_fingerprint: row.get(9)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_peers(connection: &Connection) -> Result<Vec<Peer>> {
        let mut stmt = connection.prepare(
            "SELECT fingerprint, hostname, nickname, last_ip, tcp_port, status, trust_status, first_seen, last_seen, previous_fingerprint
             FROM peers ORDER BY last_seen DESC;",
        )?;

        let peer_iter = stmt.query_map([], |row| {
            let status_str: String = row.get(5)?;
            let trust_str: String = row.get(6)?;

            let state = match status_str.as_str() {
                "Online" => PeerState::Online,
                "Connecting" => PeerState::Connecting,
                "Discovered" => PeerState::Discovered,
                _ => PeerState::Offline,
            };

            let trust_status = match trust_str.as_str() {
                "Blocked" => TrustStatus::Blocked,
                "Identity Changed" => TrustStatus::IdentityChanged,
                _ => TrustStatus::Trusted,
            };

            Ok(Peer {
                fingerprint: row.get(0)?,
                hostname: row.get(1)?,
                nickname: row.get(2)?,
                last_ip: row.get(3)?,
                tcp_port: row.get(4)?,
                state,
                trust_status,
                first_seen: row.get(7)?,
                last_seen: row.get(8)?,
                previous_fingerprint: row.get(9)?,
            })
        })?;

        let mut peers = Vec::new();
        for peer in peer_iter {
            peers.push(peer?);
        }

        Ok(peers)
    }

    pub fn find_peer_by_hostname(connection: &Connection, hostname: &str) -> Result<Vec<Peer>> {
        let mut stmt = connection.prepare(
            "SELECT fingerprint, hostname, nickname, last_ip, tcp_port, status, trust_status, first_seen, last_seen, previous_fingerprint
             FROM peers WHERE hostname = ?1;",
        )?;

        let peer_iter = stmt.query_map(params![hostname], |row| {
            let status_str: String = row.get(5)?;
            let trust_str: String = row.get(6)?;

            let state = match status_str.as_str() {
                "Online" => PeerState::Online,
                "Connecting" => PeerState::Connecting,
                "Discovered" => PeerState::Discovered,
                _ => PeerState::Offline,
            };

            let trust_status = match trust_str.as_str() {
                "Blocked" => TrustStatus::Blocked,
                "Identity Changed" => TrustStatus::IdentityChanged,
                _ => TrustStatus::Trusted,
            };

            Ok(Peer {
                fingerprint: row.get(0)?,
                hostname: row.get(1)?,
                nickname: row.get(2)?,
                last_ip: row.get(3)?,
                tcp_port: row.get(4)?,
                state,
                trust_status,
                first_seen: row.get(7)?,
                last_seen: row.get(8)?,
                previous_fingerprint: row.get(9)?,
            })
        })?;

        let mut peers = Vec::new();
        for peer in peer_iter {
            peers.push(peer?);
        }

        Ok(peers)
    }

    pub fn update_peer_trust_status(
        connection: &Connection,
        fingerprint: &str,
        trust_status: TrustStatus,
    ) -> Result<()> {
        connection.execute(
            "UPDATE peers SET trust_status = ?1 WHERE fingerprint = ?2;",
            params![trust_status.to_string(), fingerprint],
        )?;
        Ok(())
    }

    pub fn update_peer_state(
        connection: &Connection,
        fingerprint: &str,
        state: PeerState,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        connection.execute(
            "UPDATE peers SET status = ?1, last_seen = ?2 WHERE fingerprint = ?3;",
            params![state.to_string(), now, fingerprint],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Messages Persistence (FR-DB-04)
    // -------------------------------------------------------------------------

    pub fn insert_message(connection: &Connection, message: &Message) -> Result<()> {
        connection.execute(
            "INSERT INTO messages (msg_id, peer_fingerprint, sender_pubkey, direction, content, status, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                message.msg_id,
                message.peer_fingerprint,
                message.sender_pubkey,
                message.direction.to_string(),
                message.content,
                message.status.to_string(),
                message.timestamp_ms,
            ],
        )?;
        Ok(())
    }

    pub fn update_message_status(
        connection: &Connection,
        msg_id: &str,
        status: MessageStatus,
    ) -> Result<()> {
        connection.execute(
            "UPDATE messages SET status = ?1 WHERE msg_id = ?2;",
            params![status.to_string(), msg_id],
        )?;
        Ok(())
    }

    pub fn get_messages_for_peer(
        connection: &Connection,
        peer_fingerprint: &str,
    ) -> Result<Vec<Message>> {
        let mut stmt = connection.prepare(
            "SELECT msg_id, peer_fingerprint, sender_pubkey, direction, content, status, timestamp_ms
             FROM messages WHERE peer_fingerprint = ?1 ORDER BY timestamp_ms ASC;",
        )?;

        let msg_iter = stmt.query_map(params![peer_fingerprint], |row| {
            let dir_str: String = row.get(3)?;
            let status_str: String = row.get(5)?;

            let direction = dir_str
                .parse::<MessageDirection>()
                .unwrap_or(MessageDirection::Inbound);
            let status = status_str
                .parse::<MessageStatus>()
                .unwrap_or(MessageStatus::Sent);

            Ok(Message {
                msg_id: row.get(0)?,
                peer_fingerprint: row.get(1)?,
                sender_pubkey: row.get(2)?,
                direction,
                content: row.get(4)?,
                status,
                timestamp_ms: row.get(6)?,
            })
        })?;

        let mut messages = Vec::new();
        for msg in msg_iter {
            messages.push(msg?);
        }

        Ok(messages)
    }

    // -------------------------------------------------------------------------
    // Settings Persistence (FR-DB-05)
    // -------------------------------------------------------------------------

    pub fn set_setting(connection: &Connection, key: &str, value: &str) -> Result<()> {
        connection.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(connection: &Connection, key: &str) -> Result<Option<String>> {
        let mut stmt = connection.prepare("SELECT value FROM settings WHERE key = ?1;")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }
}
