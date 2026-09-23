# FUNCTIONAL REQUIREMENTS DOCUMENT
## Rubix – PingPongzzz
### Secure Offline-First Peer-to-Peer LAN Messaging Application

| Document Field | Value |
|---|---|
| **Document Type** | Functional Requirements Document (FRD) |
| **Derived From** | RUBIX-PINGPONGZZZ Business Requirements Document (BRD) v1.0.0 |
| **Version** | 1.0.0 |
| **Status** | DRAFT — FOR REVIEW |
| **Author** | Manikkavasakar K |
| **Prepared On** | July 02, 2026 |
| **Release Type** | Messaging Only |
| **Target Platforms** | Windows, Linux, macOS |

---

## 1. Document Purpose & Scope
This Functional Requirements Document (FRD) translates the RUBIX-PINGPONGZZZ Business Requirements Document (BRD v1.0.0, Status: FROZEN) into detailed, testable functional requirements. It defines exactly what the system must do, for each module, so that engineering, QA, and stakeholders share a single unambiguous reference for Version 1.0 (Messaging Only release).

Each requirement is assigned a unique FR ID, mapped to the responsible actor (System, User, Peer), a priority level, and its originating BRD section for traceability.

---

## 2. Product Summary
Rubix-PingPongzzz is a secure, offline-first, peer-to-peer LAN messaging application built in Rust. It enables encrypted real-time one-to-one communication between devices on the same local network, without internet access, cloud infrastructure, user accounts, or centralized servers.

### Primary Use Cases
- Office communication
- School labs
- Training institutes
- Development teams
- Research labs
- Air-gapped local networks
- Home networks

---

## 3. In-Scope vs Out-of-Scope (Version 1.0)

### Included in v1.0
- Peer Discovery
- Identity Management
- Secure Messaging
- Local Message Storage
- Notifications
- Cross-Platform Support (Windows, Linux, macOS)

### Excluded from v1.0
- File Transfer
- Read Receipts
- Typing Indicators
- Group Chat
- Voice Calls
- Video Calls
- Mobile Apps
- LDAP / Active Directory / Enterprise Policies
- Internet Messaging
- Cross-Subnet Communication
- Message Synchronization
- QR Verification

---

## 4. Functional Requirements — Identity Management
Covers device identity generation, display, collision handling, change detection, and reset (BRD Section 5).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-ID-01** | On first launch, the system shall auto-detect and use the computer hostname as the display label, without requiring user registration or a username. | System | Must | §5 |
| **FR-ID-02** | On first launch, the system shall generate one Ed25519 key pair (identity/signing) and one X25519 key pair (key exchange), generated exactly once. | System | Must | §5.1 |
| **FR-ID-03** | The system shall securely store cryptographic keys using the OS-native secure store: DPAPI (Windows), Keychain (macOS), Secret Service (Linux). | System | Must | §5.1 |
| **FR-ID-04** | The system shall display identity in the format `HOSTNAME [FINGERPRINT]`, where `FINGERPRINT` is the first 12 hex characters of the Ed25519 public key. | System | Must | §5.2 |
| **FR-ID-05** | The system shall treat the fingerprint as the authoritative identity; the hostname is a cosmetic label only. | System | Must | §5.2 |
| **FR-ID-06** | The user shall optionally configure a nickname, displayed as `Nickname (HOSTNAME) [FINGERPRINT]`; the nickname shall not affect identity resolution. | User | Should | §5.3 |
| **FR-ID-07** | The system shall allow multiple devices with identical hostnames to coexist and remain independently visible, with no uniqueness enforcement. | System | Must | §5.4 |
| **FR-ID-08** | The system shall detect when a known hostname reappears with a different fingerprint and display an 'Identity Changed' warning showing old and new fingerprints. | System | Must | §5.5 |
| **FR-ID-09** | On identity-change warning, the system shall present the user two options: Trust New Identity or Block. | User | Must | §5.5 |
| **FR-ID-10** | The system shall provide a Settings → Reset Identity action that regenerates both key pairs, preserves existing message history, and notifies peers via the identity-change mechanism. | User | Should | §5.6 |

---

## 5. Functional Requirements — Peer Discovery
Covers automatic peer discovery via mDNS, UDP broadcast fallback, and manual entry (BRD Section 6).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-PD-01** | The system shall advertise itself and discover peers using mDNS service type `_rubix._tcp.local` as the primary discovery mechanism. | System | Must | §6.1 |
| **FR-PD-02** | When mDNS is unavailable, the system shall fall back to UDP broadcast on `255.255.255.255`, port `9876`. | System | Must | §6.2 |
| **FR-PD-03** | The system shall construct UDP beacons with fields: magic (`"RUBX"`), version, tcp_port, fingerprint, hostname. | System | Must | §6.2 |
| **FR-PD-04** | The system shall discard/ignore any UDP beacon with an invalid magic value, unsupported version, or malformed structure. | System | Must | §6.2 |
| **FR-PD-05** | If UDP port 9876 is unavailable, the system shall log a warning ("UDP discovery unavailable") and continue operating via mDNS/manual entry without failing startup. | System | Must | §6.3 |
| **FR-PD-06** | The user shall be able to manually add a peer by entering IP address and fingerprint; the system shall establish a TCP connection, perform a Noise handshake, and verify the fingerprint. | User / System | Must | §6.4 |
| **FR-PD-07** | The system shall support tracking up to 200 discovered peers. | System | Must | §2, §8 |

---

## 6. Functional Requirements — Peer State & Connection Management
Covers peer lifecycle states, connection limits, and eviction policy (BRD Sections 7–9).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-PC-01** | The system shall track each peer through the state machine: `Unknown` → `Discovered` → `Connecting` → `Online` → `Offline`. | System | Must | §7 |
| **FR-PC-02** | The system shall retain message history for peers in the `Offline` state. | System | Must | §7 |
| **FR-PC-03** | The system shall support a maximum of 10 simultaneous active (TCP) connections, to avoid full-mesh architecture. | System | Must | §8 |
| **FR-PC-04** | The system shall open a connection to a peer only when the user opens that peer's chat window. | System | Must | §8.1 |
| **FR-PC-05** | The system shall automatically close a peer connection after 5 minutes of inactivity. | System | Must | §8.1 |
| **FR-PC-06** | When an 11th connection is required, the system shall evict the least-recently-used (LRU) active connection before opening the new one. | System | Must | §8.2 |
| **FR-PC-07** | The system shall send a heartbeat every 10 seconds per active connection to detect disconnects. | System | Must | §9 |
| **FR-PC-08** | The system shall mark a peer `Offline` after 3 consecutive missed heartbeats. | System | Must | §9 |

---

## 7. Functional Requirements — Secure Messaging
Covers message structure, states, and delivery behavior (BRD Sections 10–11).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-MSG-01** | The system shall support one-to-one messaging only (no group conversations in v1.0). | System | Must | §10 |
| **FR-MSG-02** | Each message frame shall contain: `msg_id` (UUIDv4), `sender_pubkey` (Ed25519), `timestamp` (ms since epoch), and UTF-8 content. | System | Must | §10.1 |
| **FR-MSG-03** | The system shall enforce a maximum message size of 4096 bytes and reject/prevent sending oversized messages. | System | Must | §10.1 |
| **FR-MSG-04** | The system shall track message state as `PENDING` (clock icon) while awaiting TCP write. | System | Must | §10.2 |
| **FR-MSG-05** | The system shall mark a message `SENT` (✓) once the TCP write succeeds. | System | Must | §10.2 |
| **FR-MSG-06** | The system shall mark a message `FAILED` (✗) on TCP write failure; v1.0 shall not implement Delivered/Read states or automatic retry. | System | Must | §10.2 |
| **FR-MSG-07** | If the recipient peer is `Online`, the system shall send the message immediately. | System | Must | §11 |
| **FR-MSG-08** | If the recipient peer is `Offline`, the system shall reject the send immediately with no queueing, no offline storage, and no auto-retry. | System | Must | §11 |
| **FR-MSG-09** | The user shall be able to manually resend a failed message. | User | Should | §11 |

---

## 8. Functional Requirements — Security Architecture
Covers cryptographic protocol stack and trust model (BRD Section 12).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-SEC-01** | The system shall use Ed25519 for identity/signature verification. | System | Must | §12.1 |
| **FR-SEC-02** | The system shall use X25519 for shared-secret key exchange. | System | Must | §12.2 |
| **FR-SEC-03** | The system shall perform session establishment using the `Noise_XX_25519_ChaChaPoly_SHA256` handshake pattern. | System | Must | §12.3 |
| **FR-SEC-04** | The system shall encrypt all message traffic using ChaCha20-Poly1305 for confidentiality and authentication. | System | Must | §12.4 |
| **FR-SEC-05** | The system shall use SHA-256 for integrity hashing where required. | System | Must | §12.5 |
| **FR-SEC-06** | The system shall operate without any PKI, Certificate Authority, or central coordinator; trust shall be established solely via fingerprint verification. | System | Must | §12.6 |

---

## 9. Functional Requirements — Local Storage
Covers persistence layer and integrity checking (BRD Section 13).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-DB-01** | The system shall persist data locally using SQLite in WAL (Write-Ahead Logging) mode. | System | Must | §13 |
| **FR-DB-02** | The system shall maintain an `identity` table storing keys, hostname, and nickname. | System | Must | §13.1 |
| **FR-DB-03** | The system shall maintain a `peers` table storing fingerprint, hostname, last IP, first-seen, and last-seen timestamps. | System | Must | §13.1 |
| **FR-DB-04** | The system shall maintain a `messages` table storing message ID, peer fingerprint, direction, content, status, and timestamps. | System | Must | §13.1 |
| **FR-DB-05** | The system shall maintain a `settings` table storing theme, notification preferences, and other user preferences. | System | Must | §13.1 |
| **FR-DB-06** | On startup, the system shall run `PRAGMA integrity_check` on the local database. | System | Must | §13.2 |
| **FR-DB-07** | If corruption is detected, the system shall rename the corrupt file to `database.corrupt`, create a fresh database, and notify the user. | System | Must | §13.2 |

---

## 10. Functional Requirements — Notifications
Covers notification triggers, behavior, and rate limiting (BRD Section 14).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-NOT-01** | When the application is focused, the system shall update the conversation window in real time on message receipt (no OS notification). | System | Must | §14 |
| **FR-NOT-02** | When the application is backgrounded, the system shall raise an OS-level notification on message receipt. | System | Must | §14 |
| **FR-NOT-03** | The system shall rate-limit notifications to a maximum of 1 notification per peer per 5 seconds, batching bursts into a summary (e.g., "Ruby sent 12 messages"). | System | Must | §14 |

---

## 11. Functional Requirements — User Interface
Covers layout and interaction elements (BRD Section 15).

| FR ID | Functional Requirement | Actor | Priority | BRD Ref |
|---|---|---|---|---|
| **FR-UI-01** | The system shall provide a top bar containing Search, Settings, and Profile controls. | System | Must | §15 |
| **FR-UI-02** | The system shall provide a left panel listing all known peers. | System | Must | §15 |
| **FR-UI-03** | The system shall provide a right panel showing the active conversation. | System | Must | §15 |
| **FR-UI-04** | The system shall provide a bottom message input box with a Send button. | System | Must | §15 |
| **FR-UI-05** | The system shall sort the peer list with Online peers first, then Offline peers, alphabetically within each group. | System | Must | §15 |

---

## 12. Non-Functional Requirements (Performance Targets)

| Metric | Target | BRD Ref |
|---|---|---|
| Application launch time | < 3 seconds | §16 |
| Peer discovery time | < 10 seconds | §16 |
| Message latency | < 150 ms | §16 |
| Memory usage | < 200 MB | §16 |
| Max discovered peers | 200 | §16 |
| Max active connections | 10 | §16 |

---

## 13. Technology Stack

| Layer | Technology |
|---|---|
| Language | Rust (2024 edition) |
| GUI | `egui`, `eframe` |
| Async Runtime | `tokio` |
| Discovery | `mdns-sd` |
| Cryptography | `snow`, `ed25519-dalek`, `x25519-dalek` |
| Database | `rusqlite` |
| Serialization | `serde`, `bincode` |
| Logging | `tracing`, `tracing-subscriber` |

---

## 14. Platform Support

| Platform | Min. Version | Installer/Package |
|---|---|---|
| Windows | 10+ | MSI |
| macOS | 12+ | DMG |
| Linux | Ubuntu 22.04+, Fedora | AppImage, deb, rpm |

---

## 15. Target Network Environment
### Supported
- IPv4 LAN, single broadcast domain
- Same router / switch / office / school network

### Not Supported
- VPN networks
- Cross-VLAN communication
- Cross-subnet communication
- Internet messaging
- IPv6-only networks

---

## 16. Acceptance Criteria
- Two devices discover each other within 10 seconds.
- Messages encrypted and decrypted successfully end-to-end.
- Packet capture confirms all traffic is encrypted.
- Message history survives application restart.
- Offline peer send attempts are rejected immediately.
- Identity-change warnings trigger correctly on fingerprint mismatch.
- Application functions fully with no internet connection.
- Windows, Linux, and macOS builds pass functional testing.
- 50 peers run continuously for 24 hours without failure.
- Memory growth remains below 5% over a 24-hour session.
- Message delivery success rate exceeds 99.9% for online peers.
- 200 peers are discoverable simultaneously on the same LAN.
- No centralized server dependency exists at any point.

---

## 17. Assumptions & Constraints
- All communicating devices reside on the same IPv4 broadcast domain.
- No user authentication, accounts, or login/signup flows exist in v1.0.
- No message queueing or offline delivery is provided; sending to an offline peer fails immediately by design.
- Group chat, file transfer, and mobile support are explicitly out of scope for v1.0 and reserved for the roadmap.

---

## 18. Roadmap (Post v1.0, Reference Only)
### Version 1.1
- File Transfer (up to 5 GB)
- Resume Support
- SHA-256 file verification
- Search

### Version 1.2
- Group Chat

### Version 2.0
- Mobile Applications
- QR Trust Verification
- Public SDK/API
