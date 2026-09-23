use std::path::Path;
use rusqlite::Connection;

use PingPongzzz::crypto::CryptoKeys;
use PingPongzzz::crypto::noise::NoiseHandshake;
use PingPongzzz::database::database::Database;
use PingPongzzz::database::schema::Tables;
use PingPongzzz::discovery::{DiscoveryBeacon, PeerTracker, BEACON_MAGIC, BEACON_VERSION};
use PingPongzzz::identity::IdentityManager;
use PingPongzzz::models::{Message, PeerState, TrustStatus, MAX_MESSAGE_SIZE};

// -----------------------------------------------------------------------------
// Test 1: Identity & Cryptographic Keys (FR-ID-02, FR-ID-04, FR-ID-05, FR-SEC-01, FR-SEC-02)
// -----------------------------------------------------------------------------
#[test]
fn test_crypto_key_generation_and_fingerprint() {
    let keys = CryptoKeys::generate_new();

    // FR-ID-04: Fingerprint is exactly first 12 hex characters of Ed25519 public key
    assert_eq!(keys.fingerprint.len(), 12);
    assert!(keys.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));

    // Verify deterministic derivation
    let derived_fp = CryptoKeys::derive_fingerprint(&keys.ed25519_verifying_key);
    assert_eq!(keys.fingerprint, derived_fp);

    // Verify Ed25519 signature & verification (FR-SEC-01)
    let payload = b"Rubix PingPongzzz test payload";
    let signature = keys.sign(payload);
    let verify_res = CryptoKeys::verify(&keys.ed25519_verifying_key, payload, &signature);
    assert!(verify_res.is_ok());

    // Tampered payload fails verification
    let bad_payload = b"Tampered payload";
    let bad_verify = CryptoKeys::verify(&keys.ed25519_verifying_key, bad_payload, &signature);
    assert!(bad_verify.is_err());
}

// -----------------------------------------------------------------------------
// Test 2: Identity Display Formats (FR-ID-04, FR-ID-06)
// -----------------------------------------------------------------------------
#[test]
fn test_identity_display_formatting() {
    let conn = Connection::open_in_memory().unwrap();
    Tables::create_all(&conn).unwrap();

    let identity = IdentityManager::load_or_create(&conn).unwrap();
    let fp = identity.fingerprint();
    let host = identity.hostname();

    // Default: HOSTNAME [FINGERPRINT]
    let default_label = identity.display_label();
    assert_eq!(default_label, format!("{} [{}]", host, fp));

    // Optional nickname: Nickname (HOSTNAME) [FINGERPRINT]
    identity.set_nickname(&conn, Some("Alice".to_string())).unwrap();
    let nick_label = identity.display_label();
    assert_eq!(nick_label, format!("Alice ({}) [{}]", host, fp));

    // Clear nickname returns to default
    identity.set_nickname(&conn, None).unwrap();
    assert_eq!(identity.display_label(), format!("{} [{}]", host, fp));
}

// -----------------------------------------------------------------------------
// Test 3: Noise_XX Handshake and ChaCha20-Poly1305 Transport (FR-SEC-03, FR-SEC-04)
// -----------------------------------------------------------------------------
#[test]
fn test_noise_xx_handshake_and_encryption() {
    let alice_keys = CryptoKeys::generate_new();
    let bob_keys = CryptoKeys::generate_new();

    let mut alice_hs = NoiseHandshake::new_initiator(&alice_keys.x25519_secret.to_bytes()).unwrap();
    let mut bob_hs = NoiseHandshake::new_responder(&bob_keys.x25519_secret.to_bytes()).unwrap();

    // Step 1: Alice -> Bob (-> e)
    let msg1 = alice_hs.write_message(&[]).unwrap();
    bob_hs.read_message(&msg1).unwrap();

    // Step 2: Bob -> Alice (<- e, ee, s, es)
    let bob_payload = b"BobEd25519Payload";
    let msg2 = bob_hs.write_message(bob_payload).unwrap();
    let alice_rx_payload = alice_hs.read_message(&msg2).unwrap();
    assert_eq!(&alice_rx_payload, bob_payload);

    // Step 3: Alice -> Bob (-> s, se)
    let alice_payload = b"AliceEd25519Payload";
    let msg3 = alice_hs.write_message(alice_payload).unwrap();
    let bob_rx_payload = bob_hs.read_message(&msg3).unwrap();
    assert_eq!(&bob_rx_payload, alice_payload);

    // Both handshakes complete
    assert!(alice_hs.is_finished());
    assert!(bob_hs.is_finished());

    let mut alice_session = alice_hs.into_session().unwrap();
    let mut bob_session = bob_hs.into_session().unwrap();

    // Encrypt from Alice, Decrypt at Bob
    let secret_message = b"Secret offline-first peer-to-peer message";
    let ciphertext = alice_session.encrypt(secret_message).unwrap();
    assert_ne!(ciphertext, secret_message);

    let decrypted = bob_session.decrypt(&ciphertext).unwrap();
    assert_eq!(decrypted, secret_message);
}

// -----------------------------------------------------------------------------
// Test 4: UDP Discovery Beacon Validation (FR-PD-03, FR-PD-04)
// -----------------------------------------------------------------------------
#[test]
fn test_udp_beacon_serialization_and_validation() {
    let beacon = DiscoveryBeacon::new(9875, "A1B2C3D4E5F6".to_string(), "Host-Alpha".to_string());
    let bytes = beacon.to_bytes().unwrap();

    // Valid beacon parses successfully
    let parsed = DiscoveryBeacon::from_bytes(&bytes).unwrap();
    assert_eq!(parsed.magic, BEACON_MAGIC);
    assert_eq!(parsed.version, BEACON_VERSION);
    assert_eq!(parsed.tcp_port, 9875);
    assert_eq!(parsed.fingerprint, "A1B2C3D4E5F6");
    assert_eq!(parsed.hostname, "Host-Alpha");

    // Invalid magic discarded (FR-PD-04)
    let mut bad_magic_bytes = bytes.clone();
    bad_magic_bytes[0..4].copy_from_slice(b"BAD!");
    let err_magic = DiscoveryBeacon::from_bytes(&bad_magic_bytes);
    assert!(err_magic.is_err());

    // Corrupt bytes discarded (FR-PD-04)
    let garbage = vec![0xFF; 5];
    assert!(DiscoveryBeacon::from_bytes(&garbage).is_err());
}

// -----------------------------------------------------------------------------
// Test 5: Peer Sorting (FR-UI-05) and Peer Limit (FR-PD-07)
// -----------------------------------------------------------------------------
#[test]
fn test_peer_sorting_online_first_alphabetical() {
    let conn = Connection::open_in_memory().unwrap();
    Tables::create_all(&conn).unwrap();
    let tracker = PeerTracker::new();

    // Add 4 peers in various states
    tracker.register_peer(&conn, "FP01".into(), "Zeta".into(), "192.168.1.10".into(), 9875).unwrap();
    tracker.register_peer(&conn, "FP02".into(), "Bravo".into(), "192.168.1.11".into(), 9875).unwrap();
    tracker.register_peer(&conn, "FP03".into(), "Alpha".into(), "192.168.1.12".into(), 9875).unwrap();
    tracker.register_peer(&conn, "FP04".into(), "Delta".into(), "192.168.1.13".into(), 9875).unwrap();

    // Set Bravo and Zeta to Online, Alpha and Delta to Offline
    tracker.set_peer_state(&conn, "FP02", PeerState::Online); // Bravo (Online)
    tracker.set_peer_state(&conn, "FP01", PeerState::Online); // Zeta (Online)
    tracker.set_peer_state(&conn, "FP03", PeerState::Offline); // Alpha (Offline)
    tracker.set_peer_state(&conn, "FP04", PeerState::Offline); // Delta (Offline)

    let sorted = tracker.get_sorted_peers();
    let sorted_names: Vec<String> = sorted.iter().map(|p| p.hostname.clone()).collect();

    // FR-UI-05: Online peers first (Bravo, Zeta), then Offline peers (Alpha, Delta), alphabetically within each
    assert_eq!(sorted_names, vec!["Bravo", "Zeta", "Alpha", "Delta"]);
}

// -----------------------------------------------------------------------------
// Test 6: Message Size Enforced at 4096 Bytes (FR-MSG-03)
// -----------------------------------------------------------------------------
#[test]
fn test_message_size_limits() {
    let valid_content = "A".repeat(MAX_MESSAGE_SIZE);
    let msg_res = Message::new_outbound("FP123".into(), "PUBKEY".into(), valid_content);
    assert!(msg_res.is_ok());

    let oversized_content = "A".repeat(MAX_MESSAGE_SIZE + 1);
    let oversized_res = Message::new_outbound("FP123".into(), "PUBKEY".into(), oversized_content);
    assert!(oversized_res.is_err());
}

// -----------------------------------------------------------------------------
// Test 7: SQLite WAL Mode & Integrity Check (FR-DB-01, FR-DB-06, FR-DB-07)
// -----------------------------------------------------------------------------
#[test]
fn test_database_wal_mode_and_integrity_check() {
    let temp_db_path = format!("/tmp/test_pingpong_{}.db", uuid::Uuid::new_v4());
    let conn = Database::initialize_with_path(&temp_db_path).unwrap();

    // FR-DB-01: Verify WAL mode
    let mut stmt = conn.prepare("PRAGMA journal_mode;").unwrap();
    let mode: String = stmt.query_row([], |row| row.get(0)).unwrap();
    assert_eq!(mode.to_uppercase(), "WAL");

    // FR-DB-06: Verify integrity check
    let integrity_res = Database::run_integrity_check(Path::new(&temp_db_path));
    assert!(integrity_res.is_ok());

    // Clean up
    let _ = std::fs::remove_file(&temp_db_path);
    let _ = std::fs::remove_file(format!("{}-wal", temp_db_path));
    let _ = std::fs::remove_file(format!("{}-shm", temp_db_path));
}

// -----------------------------------------------------------------------------
// Test 8: Identity Change Detection (FR-ID-08, FR-ID-09)
// -----------------------------------------------------------------------------
#[test]
fn test_identity_change_detection_and_resolution() {
    let conn = Connection::open_in_memory().unwrap();
    Tables::create_all(&conn).unwrap();
    let tracker = PeerTracker::new();

    // Initial peer registration: Hostname "Alice-PC" with fingerprint "FP_ORIGINAL"
    tracker.register_peer(&conn, "FP_ORIGINAL".into(), "Alice-PC".into(), "192.168.1.50".into(), 9875).unwrap();

    // Peer reappears with SAME hostname but DIFFERENT fingerprint "FP_NEW"
    let updated = tracker.register_peer(&conn, "FP_NEW".into(), "Alice-PC".into(), "192.168.1.50".into(), 9875).unwrap();
    assert!(updated.is_some());
    let peer = updated.unwrap();

    // FR-ID-08: Detects identity change!
    assert_eq!(peer.trust_status, TrustStatus::IdentityChanged);
    assert_eq!(peer.previous_fingerprint.as_deref(), Some("FP_ORIGINAL"));

    // FR-ID-09: User option 1: Trust New Identity
    tracker.resolve_identity_change(&conn, "FP_NEW", true).unwrap();
    let trusted_peer = tracker.get_peer("FP_NEW").unwrap();
    assert_eq!(trusted_peer.trust_status, TrustStatus::Trusted);

    // User option 2: Block
    tracker.resolve_identity_change(&conn, "FP_NEW", false).unwrap();
    let blocked_peer = tracker.get_peer("FP_NEW").unwrap();
    assert_eq!(blocked_peer.trust_status, TrustStatus::Blocked);
}
