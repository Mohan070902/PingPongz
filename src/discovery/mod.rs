pub mod beacon;
pub mod mdns;
pub mod peer_tracker;
pub mod udp;

pub use beacon::{DiscoveryBeacon, BEACON_MAGIC, BEACON_VERSION};
pub use mdns::MdnsDiscoveryService;
pub use peer_tracker::{PeerTracker, MAX_TRACKED_PEERS};
pub use udp::{DiscoveredPeerEvent, UdpDiscoveryService};
