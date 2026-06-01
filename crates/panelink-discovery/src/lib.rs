use panelink_core::{demo_peers, Peer};

pub const SERVICE_NAME: &str = "_panelink._udp.local";

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub service_name: &'static str,
    pub port: u16,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            service_name: SERVICE_NAME,
            port: 48170,
        }
    }
}

pub fn list_cached_peers() -> Vec<Peer> {
    demo_peers()
}

pub fn advertise_payload() -> Vec<(&'static str, String)> {
    vec![
        ("service", SERVICE_NAME.to_string()),
        ("version", env!("CARGO_PKG_VERSION").to_string()),
        ("transport", "quic".to_string()),
        ("pairing", "required".to_string()),
    ]
}
