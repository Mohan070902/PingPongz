use rusqlite::{Connection, Result};
use tracing::info;

pub struct Tables;

impl Tables {
    pub fn create_all(connection: &Connection) -> Result<()> {
        // Automatic migration check for legacy pre-v1.0 identity table
        let mut stmt = connection.prepare("PRAGMA table_info(identity);")?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .filter_map(|r| r.ok())
            .collect();

        if !columns.is_empty() && !columns.contains(&"ed25519_pubkey".to_string()) {
            info!("Migrating legacy identity table to v1.0.0 schema");
            connection.execute("DROP TABLE identity;", [])?;
        }

        // FR-DB-02: Identity Table
        connection.execute(
            "CREATE TABLE IF NOT EXISTS identity (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hostname TEXT NOT NULL,
                nickname TEXT,
                fingerprint TEXT NOT NULL,
                ed25519_pubkey TEXT NOT NULL,
                x25519_pubkey TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
            [],
        )?;

        // FR-DB-03: Peers Table
        connection.execute(
            "CREATE TABLE IF NOT EXISTS peers (
                fingerprint TEXT PRIMARY KEY,
                hostname TEXT NOT NULL,
                nickname TEXT,
                last_ip TEXT NOT NULL,
                tcp_port INTEGER NOT NULL,
                status TEXT NOT NULL,
                trust_status TEXT NOT NULL,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL,
                previous_fingerprint TEXT
            );",
            [],
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_peers_hostname ON peers(hostname);",
            [],
        )?;

        // FR-DB-04: Messages Table
        connection.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                msg_id TEXT PRIMARY KEY,
                peer_fingerprint TEXT NOT NULL,
                sender_pubkey TEXT NOT NULL,
                direction TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                FOREIGN KEY (peer_fingerprint) REFERENCES peers(fingerprint)
            );",
            [],
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_peer_timestamp 
             ON messages(peer_fingerprint, timestamp_ms);",
            [],
        )?;

        // FR-DB-05: Settings Table
        connection.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }
}