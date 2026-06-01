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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StreamStatus {
    Idle,
    Starting,
    Live,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFrame {
    pub id: u64,
    pub presented_at_unix_ms: u64,
    pub width: u32,
    pub height: u32,
    pub screen_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamState {
    pub status: StreamStatus,
    pub active_peer_id: Option<String>,
    pub screen_ids: Vec<String>,
    pub screen_count: usize,
    pub codec: String,
    pub transport: TransportMode,
    pub quality: String,
    pub width: u32,
    pub height: u32,
    pub target_fps: u16,
    pub fps: u16,
    pub bitrate_mbps: f32,
    pub latency_ms: u16,
    pub jitter_ms: f32,
    pub packet_loss: f32,
    pub frames_sent: u64,
    pub dropped_frames: u64,
    pub frame_id: u64,
    pub audio_active: bool,
    pub microphone_active: bool,
    pub updated_at_unix_ms: u64,
    pub error: Option<String>,
    pub last_frame: Option<StreamFrame>,
    pub message: String,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            status: StreamStatus::Idle,
            active_peer_id: None,
            screen_ids: Vec::new(),
            screen_count: 0,
            codec: "H.264 low latency".into(),
            transport: TransportMode::LanQuic,
            quality: "Low latency".into(),
            width: 0,
            height: 0,
            target_fps: 120,
            fps: 0,
            bitrate_mbps: 0.0,
            latency_ms: 0,
            jitter_ms: 0.0,
            packet_loss: 0.0,
            frames_sent: 0,
            dropped_frames: 0,
            frame_id: 0,
            audio_active: true,
            microphone_active: true,
            updated_at_unix_ms: now_unix_ms(),
            error: None,
            last_frame: None,
            message: "Stream idle".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStreamRequest {
    pub peer_id: String,
    pub screen_ids: Vec<String>,
    pub quality: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub status: ConnectionStatus,
    pub active_peer_id: Option<String>,
    pub connected_at_unix_ms: Option<u64>,
    pub metrics: TransportMetrics,
    pub screens: Vec<RemoteScreen>,
    pub stream: StreamState,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Ready,
            active_peer_id: None,
            connected_at_unix_ms: None,
            metrics: TransportMetrics::default(),
            screens: vec![default_screen(false, None)],
            stream: StreamState::default(),
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
    NotConnected,
    ScreenLimitReached,
    ScreenNotFound,
    CannotRemoveLastScreen,
    StreamUnavailable,
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
                peer_id: Some(peer_id.clone()),
                ping_count: 0,
                last_latency_ms: 9,
                average_latency_ms: 9.0,
                packet_loss: 0.0,
                bitrate_mbps: 58.0,
                fps: 120,
            },
            screens: vec![default_screen(true, Some(peer_id.clone()))],
            stream: StreamState {
                status: StreamStatus::Idle,
                active_peer_id: Some(peer_id),
                screen_ids: Vec::new(),
                screen_count: 1,
                quality: "Low latency".into(),
                updated_at_unix_ms: now_unix_ms(),
                fps: 0,
                bitrate_mbps: 0.0,
                latency_ms: 0,
                frames_sent: 0,
                dropped_frames: 0,
                last_frame: None,
                message: "Connected; stream ready".into(),
                ..StreamState::default()
            },
        };

        Ok(self.snapshot())
    }

    pub fn disconnect(&mut self) -> SessionSnapshot {
        self.state = SessionState::default();
        self.snapshot()
    }

    pub fn start_stream(
        &mut self,
        _request: Option<StartStreamRequest>,
    ) -> Result<StreamState, SessionError> {
        if self.state.active_peer_id.is_none() {
            return Err(SessionError::NotConnected);
        }

        self.state.metrics.fps = 0;
        self.state.metrics.bitrate_mbps = 0.0;
        self.state.stream = StreamState {
            status: StreamStatus::Idle,
            active_peer_id: self.state.active_peer_id.clone(),
            screen_count: self.state.screens.len(),
            updated_at_unix_ms: now_unix_ms(),
            error: Some("Real video/audio transport is not implemented yet".into()),
            message: "Connected; stream engine unavailable".into(),
            ..StreamState::default()
        };

        Err(SessionError::StreamUnavailable)
    }

    pub fn stop_stream(&mut self) -> StreamState {
        self.state.metrics.fps = 0;
        self.state.metrics.bitrate_mbps = 0.0;
        self.state.stream = StreamState {
            status: StreamStatus::Idle,
            active_peer_id: self.state.active_peer_id.clone(),
            screen_count: self.state.screens.len(),
            updated_at_unix_ms: now_unix_ms(),
            message: if self.state.active_peer_id.is_some() {
                "Stream stopped; session still connected".into()
            } else {
                "Stream idle".into()
            },
            ..StreamState::default()
        };

        self.state.stream.clone()
    }

    pub fn stream_state(&mut self) -> StreamState {
        if self.state.stream.status == StreamStatus::Live {
            let next_frame = self.state.stream.frames_sent.saturating_add(1);
            let screen_index = (next_frame as usize - 1) % self.state.screens.len().max(1);
            let screen = &self.state.screens[screen_index];
            self.state.stream.frames_sent = next_frame;
            self.state.stream.screen_count = self.state.screens.len();
            self.state.stream.fps = self.state.metrics.fps;
            self.state.stream.bitrate_mbps = self.state.metrics.bitrate_mbps;
            self.state.stream.latency_ms = self.state.metrics.last_latency_ms;
            self.state.stream.frame_id = next_frame;
            self.state.stream.updated_at_unix_ms = now_unix_ms();
            self.state.stream.last_frame = Some(frame_for_screen(next_frame, screen));
        }

        self.state.stream.clone()
    }

    pub fn add_remote_screen(&mut self) -> Result<SessionSnapshot, SessionError> {
        if self.state.active_peer_id.is_none() {
            return Err(SessionError::NotConnected);
        }

        if self.state.screens.len() >= 3 {
            return Err(SessionError::ScreenLimitReached);
        }

        let index = self.state.screens.len();
        self.state.screens.push(remote_screen(
            index,
            true,
            self.state.active_peer_id.clone(),
        ));
        self.sync_metrics_after_screen_change();

        Ok(self.snapshot())
    }

    pub fn remove_remote_screen(
        &mut self,
        screen_id: String,
    ) -> Result<SessionSnapshot, SessionError> {
        if self.state.screens.len() <= 1 {
            return Err(SessionError::CannotRemoveLastScreen);
        }

        let original_len = self.state.screens.len();
        self.state.screens.retain(|screen| screen.id != screen_id);

        if self.state.screens.len() == original_len {
            return Err(SessionError::ScreenNotFound);
        }

        self.sync_metrics_after_screen_change();
        Ok(self.snapshot())
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

    fn sync_metrics_after_screen_change(&mut self) {
        let connected = self.state.status == ConnectionStatus::Connected;
        self.state.metrics.bitrate_mbps = if connected {
            bitrate_for_screens(self.state.screens.len())
        } else {
            0.0
        };
        self.state.stream.screen_count = self.state.screens.len();
        self.state.stream.bitrate_mbps = if self.state.stream.status == StreamStatus::Live {
            self.state.metrics.bitrate_mbps
        } else {
            0.0
        };
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

pub fn start_stream(request: Option<StartStreamRequest>) -> Result<StreamState, SessionError> {
    with_session_manager(|manager| manager.start_stream(request))
}

pub fn stop_stream() -> StreamState {
    with_session_manager(|manager| manager.stop_stream())
}

pub fn stream_state() -> StreamState {
    with_session_manager(|manager| manager.stream_state())
}

pub fn add_remote_screen() -> Result<SessionSnapshot, SessionError> {
    with_session_manager(|manager| manager.add_remote_screen())
}

pub fn remove_remote_screen(screen_id: String) -> Result<SessionSnapshot, SessionError> {
    with_session_manager(|manager| manager.remove_remote_screen(screen_id))
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
        screens: state.screens.clone(),
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

fn default_screen(connected: bool, peer_id: Option<String>) -> RemoteScreen {
    remote_screen(0, connected, peer_id)
}

fn remote_screen(index: usize, connected: bool, peer_id: Option<String>) -> RemoteScreen {
    let (native_resolution, fitted_resolution) = match index {
        0 => ("2560 x 1440 @ 120 Hz", "2560 x 1440 @ 120 Hz"),
        1 => ("1920 x 1080 @ 144 Hz", "1920 x 1080 @ 120 Hz"),
        _ => ("1440 x 900 @ 60 Hz", "1440 x 900 @ 60 Hz"),
    };

    RemoteScreen {
        id: format!("screen-{}", index + 1),
        name: if index == 0 {
            "LAN peer display".into()
        } else {
            format!("Extended display {}", index + 1)
        },
        role: if index == 0 {
            ScreenRole::Primary
        } else {
            ScreenRole::Extended
        },
        source_display: if index == 0 {
            "Local display".into()
        } else {
            format!("Virtual display {}", index + 1)
        },
        target_display: peer_id.unwrap_or_else(|| "No peer connected".into()),
        native_resolution: native_resolution.into(),
        fitted_resolution: fitted_resolution.into(),
        scale_mode: ScaleMode::AutoFit,
        status: if connected {
            ScreenStatus::Connected
        } else {
            ScreenStatus::Ready
        },
    }
}

fn frame_for_screen(id: u64, screen: &RemoteScreen) -> StreamFrame {
    let (width, height) = parse_resolution(&screen.fitted_resolution).unwrap_or((1920, 1080));

    StreamFrame {
        id,
        presented_at_unix_ms: now_unix_ms(),
        width,
        height,
        screen_id: screen.id.clone(),
    }
}

fn parse_resolution(value: &str) -> Option<(u32, u32)> {
    let (width, rest) = value.split_once(" x ")?;
    let height = rest.split_whitespace().next()?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn bitrate_for_screens(screen_count: usize) -> f32 {
    match screen_count {
        0 => 0.0,
        1 => 58.0,
        2 => 92.0,
        _ => 118.0,
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
    fn stream_start_is_not_faked_without_transport() {
        let mut manager = SessionManager::new();
        manager
            .connect(SessionConnectRequest {
                peer_id: "peer-a".into(),
                pairing: None,
            })
            .expect("connect should succeed");

        let started = manager.start_stream(Some(StartStreamRequest {
            peer_id: "peer-a".into(),
            screen_ids: vec!["screen-1".into()],
            quality: "Low latency".into(),
        }));
        assert_eq!(started, Err(SessionError::StreamUnavailable));
        assert_eq!(manager.state().stream.status, StreamStatus::Idle);
        assert_eq!(manager.state().stream.frames_sent, 0);
        assert!(manager.state().stream.error.is_some());

        let stopped = manager.stop_stream();
        assert_eq!(stopped.status, StreamStatus::Idle);
        assert_eq!(manager.state().metrics.fps, 0);
    }

    #[test]
    fn remote_screens_can_be_added_and_removed() {
        let mut manager = SessionManager::new();
        manager
            .connect(SessionConnectRequest {
                peer_id: "peer-a".into(),
                pairing: None,
            })
            .expect("connect should succeed");

        let snapshot = manager.add_remote_screen().expect("screen should be added");
        assert_eq!(snapshot.screens.len(), 2);
        assert_eq!(snapshot.screens[1].role, ScreenRole::Extended);

        let snapshot = manager
            .remove_remote_screen("screen-2".into())
            .expect("screen should be removed");
        assert_eq!(snapshot.screens.len(), 1);
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
