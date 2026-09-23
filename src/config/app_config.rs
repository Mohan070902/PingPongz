#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_name: String,
    pub version: String,
    pub tcp_port: u16,
    pub udp_port: u16,
    pub mdns_service_type: String,
    pub heartbeat_interval_secs: u64,
    pub missed_heartbeats_limit: u32,
    pub inactivity_timeout_secs: u64,
    pub max_active_connections: usize,
    pub max_tracked_peers: usize,
    pub max_message_size: usize,
    pub notification_rate_limit_secs: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: String::from("Rubix - PingPongzzz"),
            version: String::from("1.0.0"),
            tcp_port: 9875,
            udp_port: 9876,
            mdns_service_type: String::from("_rubix._tcp.local"),
            heartbeat_interval_secs: 10,
            missed_heartbeats_limit: 3,
            inactivity_timeout_secs: 300, // 5 minutes
            max_active_connections: 10,
            max_tracked_peers: 200,
            max_message_size: 4096,
            notification_rate_limit_secs: 5,
        }
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }
}