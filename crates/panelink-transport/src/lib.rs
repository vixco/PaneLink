use panelink_core::{
    ConnectionStatus, RemoteScreen, ScaleMode, ScreenRole, ScreenStatus, SessionSnapshot,
    TransportMode,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        primary: "In-process LAN session over typed Rust state".into(),
        control_channel: "Reliable command model stubbed behind SessionManager".into(),
        video_channel: "Encoded frame channel planned; metrics are typed now".into(),
        audio_channel: "Audio channel planned; jitter metrics are typed now".into(),
        fallback: "WebRTC for future NAT traversal".into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingProof {
    pub peer_id: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub peer_id: String,
    pub sent_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingSample {
    pub peer_id: String,
    pub sent_at_unix_ms: u64,
    pub received_at_unix_ms: u64,
    pub latency_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportMetrics {
    pub peer_id: Option<String>,
    pub ping_count: u64,
    pub last_latency_ms: u16,
    pub average_latency_ms: f32,
    pub packet_loss: f32,
    pub bitrate_mbps: f32,
    pub fps: u16,
}

impl Default for TransportMetrics {
    fn default() -> Self {
        Self {
            peer_id: None,
            ping_count: 0,
            last_latency_ms: 0,
            average_latency_ms: 0.0,
            packet_loss: 0.0,
            bitrate_mbps: 0.0,
            fps: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub status: ConnectionStatus,
    pub active_peer_id: Option<String>,
    pub connected_at_unix_ms: Option<u64>,
    pub metrics: TransportMetrics,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Ready,
            active_peer_id: None,
            connected_at_unix_ms: None,
            metrics: TransportMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionConnectRequest {
    pub peer_id: String,
    pub pairing: Option<PairingProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionError {
    EmptyPeerId,
}

#[derive(Debug, Default)]
pub struct SessionManager {
    state: SessionState,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect(
        &mut self,
        request: SessionConnectRequest,
    ) -> Result<SessionSnapshot, SessionError> {
        if request.peer_id.trim().is_empty() {
            return Err(SessionError::EmptyPeerId);
        }

        let peer_id = request.peer_id;
        self.state = SessionState {
            status: ConnectionStatus::Connected,
            active_peer_id: Some(peer_id.clone()),
            connected_at_unix_ms: Some(now_unix_ms()),
            metrics: TransportMetrics {
                peer_id: Some(peer_id),
                ping_count: 0,
                last_latency_ms: 9,
                average_latency_ms: 9.0,
                packet_loss: 0.0,
                bitrate_mbps: 58.0,
                fps: 120,
            },
        };

        Ok(self.snapshot())
    }

    pub fn disconnect(&mut self) -> SessionSnapshot {
        self.state = SessionState::default();
        self.snapshot()
    }

    pub fn ping(&mut self, request: PingRequest) -> Result<PingSample, SessionError> {
        if request.peer_id.trim().is_empty() {
            return Err(SessionError::EmptyPeerId);
        }

        let received_at_unix_ms = now_unix_ms().max(request.sent_at_unix_ms);
        let latency_ms = received_at_unix_ms
            .saturating_sub(request.sent_at_unix_ms)
            .min(u64::from(u16::MAX)) as u16;

        self.state.metrics.peer_id = Some(request.peer_id.clone());
        self.state.metrics.ping_count = self.state.metrics.ping_count.saturating_add(1);
        self.state.metrics.last_latency_ms = latency_ms;
        self.state.metrics.average_latency_ms = running_average(
            self.state.metrics.average_latency_ms,
            self.state.metrics.ping_count,
            latency_ms,
        );

        Ok(PingSample {
            peer_id: request.peer_id,
            sent_at_unix_ms: request.sent_at_unix_ms,
            received_at_unix_ms,
            latency_ms,
        })
    }

    pub fn state(&self) -> SessionState {
        self.state.clone()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        session_snapshot(&self.state)
    }
}

pub fn get_session_snapshot() -> SessionSnapshot {
    with_session_manager(|manager| manager.snapshot())
}

pub fn connect_peer(peer_id: String) -> Result<SessionSnapshot, SessionError> {
    with_session_manager(|manager| {
        manager.connect(SessionConnectRequest {
            peer_id,
            pairing: None,
        })
    })
}

pub fn disconnect_peer() -> SessionSnapshot {
    with_session_manager(|manager| manager.disconnect())
}

pub fn ping_peer(peer_id: String) -> Result<PingSample, SessionError> {
    with_session_manager(|manager| {
        manager.ping(PingRequest {
            peer_id,
            sent_at_unix_ms: now_unix_ms(),
        })
    })
}

pub fn session_state() -> SessionState {
    with_session_manager(|manager| manager.state())
}

fn with_session_manager<T>(run: impl FnOnce(&mut SessionManager) -> T) -> T {
    static MANAGER: OnceLock<Mutex<SessionManager>> = OnceLock::new();

    let mut manager = MANAGER
        .get_or_init(|| Mutex::new(SessionManager::new()))
        .lock()
        .expect("session manager mutex poisoned");

    run(&mut manager)
}

fn session_snapshot(state: &SessionState) -> SessionSnapshot {
    let metrics = &state.metrics;
    let connected = state.status == ConnectionStatus::Connected;

    SessionSnapshot {
        status: state.status,
        active_peer_id: state.active_peer_id.clone(),
        display: if connected {
            "LAN peer display".into()
        } else {
            "No active display".into()
        },
        resolution: if connected {
            "2560 x 1440 @ 120 Hz".into()
        } else {
            "Not negotiated".into()
        },
        display_plan: None,
        rollback_snapshot: None,
        screens: vec![RemoteScreen {
            id: "screen-main".into(),
            name: if connected {
                "LAN peer display".into()
            } else {
                "Waiting for peer".into()
            },
            role: ScreenRole::Primary,
            source_display: "Local display".into(),
            target_display: state
                .active_peer_id
                .clone()
                .unwrap_or_else(|| "No peer connected".into()),
            native_resolution: "2560 x 1440 @ 120 Hz".into(),
            fitted_resolution: "2560 x 1440 @ 120 Hz".into(),
            scale_mode: ScaleMode::AutoFit,
            status: if connected {
                ScreenStatus::Connected
            } else {
                ScreenStatus::Ready
            },
        }],
        fps: metrics.fps,
        latency_ms: metrics.last_latency_ms,
        bitrate_mbps: metrics.bitrate_mbps,
        packet_loss: metrics.packet_loss,
        encoder: "H.264 low latency".into(),
        transport: TransportMode::LanQuic,
        audio_output: "System Default Output".into(),
        mic_input: "System Default Microphone".into(),
    }
}

fn running_average(previous_average: f32, sample_count: u64, latest: u16) -> f32 {
    if sample_count <= 1 {
        return f32::from(latest);
    }

    previous_average + ((f32::from(latest) - previous_average) / sample_count as f32)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_sets_active_peer_and_metrics() {
        let mut manager = SessionManager::new();

        let snapshot = manager
            .connect(SessionConnectRequest {
                peer_id: "peer-a".into(),
                pairing: None,
            })
            .expect("connect should succeed");

        assert_eq!(snapshot.status, ConnectionStatus::Connected);
        assert_eq!(snapshot.active_peer_id.as_deref(), Some("peer-a"));
        assert_eq!(snapshot.screens[0].status, ScreenStatus::Connected);
        assert_eq!(manager.state().metrics.peer_id.as_deref(), Some("peer-a"));
    }

    #[test]
    fn disconnect_returns_ready_snapshot() {
        let mut manager = SessionManager::new();
        manager
            .connect(SessionConnectRequest {
                peer_id: "peer-a".into(),
                pairing: None,
            })
            .expect("connect should succeed");

        let snapshot = manager.disconnect();

        assert_eq!(snapshot.status, ConnectionStatus::Ready);
        assert_eq!(snapshot.active_peer_id, None);
        assert_eq!(snapshot.latency_ms, 0);
    }

    #[test]
    fn ping_updates_running_metrics() {
        let mut manager = SessionManager::new();
        let sent_at = now_unix_ms();

        let sample = manager
            .ping(PingRequest {
                peer_id: "peer-a".into(),
                sent_at_unix_ms: sent_at,
            })
            .expect("ping should succeed");

        assert_eq!(sample.peer_id, "peer-a");
        assert_eq!(manager.state().metrics.ping_count, 1);
        assert_eq!(manager.state().metrics.last_latency_ms, sample.latency_ms);
    }

    #[test]
    fn empty_peer_ids_are_rejected() {
        let mut manager = SessionManager::new();

        assert_eq!(
            manager.connect(SessionConnectRequest {
                peer_id: " ".into(),
                pairing: None,
            }),
            Err(SessionError::EmptyPeerId)
        );
    }
}
