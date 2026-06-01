use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportPlan {
    pub primary: String,
    pub control_channel: String,
    pub video_channel: String,
    pub audio_channel: String,
    pub fallback: String,
}

pub fn default_transport_plan() -> TransportPlan {
    TransportPlan {
        primary: "QUIC over LAN UDP/TLS".into(),
        control_channel: "Reliable QUIC stream with typed commands".into(),
        video_channel: "QUIC datagrams for low-latency encoded frames".into(),
        audio_channel: "QUIC datagrams with jitter buffer".into(),
        fallback: "WebRTC for future NAT traversal".into(),
    }
}
