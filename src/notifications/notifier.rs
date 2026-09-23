use notify_rust::Notification;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn};

pub const RATE_LIMIT_SECS: u64 = 5;

#[derive(Clone)]
pub struct Notifier {
    rate_limits: Arc<Mutex<HashMap<String, (Instant, usize)>>>,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// FR-NOT-01, FR-NOT-02, FR-NOT-03
    pub fn on_message_received(
        &self,
        peer_name: &str,
        peer_fingerprint: &str,
        message_content: &str,
        is_window_focused: bool,
    ) {
        // FR-NOT-01: When application is focused, no OS notification
        if is_window_focused {
            return;
        }

        // FR-NOT-02 & FR-NOT-03: When backgrounded, rate limit to 1 per peer per 5s and batch bursts
        let mut limits = self.rate_limits.lock().unwrap();
        let now = Instant::now();

        if let Some((last_sent, count)) = limits.get_mut(peer_fingerprint) {
            if now.duration_since(*last_sent) < Duration::from_secs(RATE_LIMIT_SECS) {
                *count += 1;
                info!(
                    "Burst rate limit: batching notification for {} (count: {})",
                    peer_name, count
                );
                return;
            } else {
                let burst_count = *count;
                *last_sent = now;
                *count = 1;

                if burst_count > 1 {
                    let summary = format!("{} sent {} messages", peer_name, burst_count);
                    Self::send_os_notification(peer_name, &summary);
                    return;
                }
            }
        } else {
            limits.insert(peer_fingerprint.to_string(), (now, 1));
        }

        let preview = if message_content.len() > 60 {
            format!("{}...", &message_content[0..60])
        } else {
            message_content.to_string()
        };

        Self::send_os_notification(peer_name, &preview);
    }

    fn send_os_notification(title: &str, body: &str) {
        #[cfg(not(target_os = "unknown"))]
        {
            let res = Notification::new()
                .appname("Rubix - PingPongzzz")
                .summary(title)
                .body(body)
                .timeout(Duration::from_secs(5))
                .show();

            if let Err(e) = res {
                warn!("Could not display OS notification: {}", e);
            }
        }
    }
}
