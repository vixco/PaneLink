use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub os: OperatingSystem,
    pub address: String,
    pub last_seen: String,
    pub status: PeerStatus,
    pub trusted: bool,
    pub latency_ms: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperatingSystem {
    #[serde(rename = "macOS")]
    MacOs,
    Windows,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerStatus {
    Online,
    Sleeping,
    Offline,
    Pairing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub app_version: String,
    pub peer_id: String,
    pub platform: String,
    pub video_encoders: Vec<String>,
    pub transport: Vec<String>,
    pub audio: AudioCapabilities,
    pub display: DisplayCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioCapabilities {
    pub output_capture: bool,
    pub microphone_capture: bool,
    pub virtual_routing: RoutingState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayCapabilities {
    pub capture: CaptureState,
    pub virtual_display: VirtualDisplayState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingState {
    Planned,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureState {
    Available,
    PermissionRequired,
    Stub,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VirtualDisplayState {
    DriverRequired,
    Available,
    Planned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub kind: AudioDeviceKind,
    pub is_default: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioDeviceKind {
    Output,
    Input,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionState {
    pub key: String,
    pub label: String,
    pub status: PermissionStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionStatus {
    Granted,
    Required,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub status: ConnectionStatus,
    pub active_peer_id: Option<String>,
    pub display: String,
    pub resolution: String,
    pub screens: Vec<RemoteScreen>,
    pub fps: u16,
    pub latency_ms: u16,
    pub bitrate_mbps: f32,
    pub packet_loss: f32,
    pub encoder: String,
    pub transport: TransportMode,
    pub audio_output: String,
    pub mic_input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteScreen {
    pub id: String,
    pub name: String,
    pub role: ScreenRole,
    pub source_display: String,
    pub target_display: String,
    pub native_resolution: String,
    pub fitted_resolution: String,
    pub scale_mode: ScaleMode,
    pub status: ScreenStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenRole {
    Primary,
    Extended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleMode {
    AutoFit,
    Native,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenStatus {
    Ready,
    Connected,
    RollbackPending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Ready,
    Connecting,
    Connected,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportMode {
    #[serde(rename = "LAN QUIC")]
    LanQuic,
    #[serde(rename = "WebRTC")]
    WebRtc,
    #[serde(rename = "Local preview")]
    LocalPreview,
}

pub fn local_peer_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn demo_peers() -> Vec<Peer> {
    vec![
        Peer {
            id: "macbook-pro".into(),
            name: "This MacBook".into(),
            os: OperatingSystem::MacOs,
            address: "192.168.1.24".into(),
            last_seen: "Now".into(),
            status: PeerStatus::Online,
            trusted: true,
            latency_ms: 7,
        },
        Peer {
            id: "windows-desk".into(),
            name: "Windows Desk".into(),
            os: OperatingSystem::Windows,
            address: "192.168.1.42".into(),
            last_seen: "Now".into(),
            status: PeerStatus::Online,
            trusted: true,
            latency_ms: 9,
        },
    ]
}

pub fn demo_session(status: ConnectionStatus, active_peer_id: Option<String>) -> SessionSnapshot {
    SessionSnapshot {
        status,
        active_peer_id,
        display: "Desk monitor".into(),
        resolution: "2560 x 1440 @ 120 Hz".into(),
        screens: vec![RemoteScreen {
            id: "screen-main".into(),
            name: "Desk monitor".into(),
            role: ScreenRole::Primary,
            source_display: "MacBook display".into(),
            target_display: "Windows Display 1".into(),
            native_resolution: "2560 x 1440 @ 120 Hz".into(),
            fitted_resolution: "2560 x 1440 @ 120 Hz".into(),
            scale_mode: ScaleMode::AutoFit,
            status: ScreenStatus::Ready,
        }],
        fps: 120,
        latency_ms: 9,
        bitrate_mbps: 58.0,
        packet_loss: 0.1,
        encoder: "H.264 low latency".into(),
        transport: TransportMode::LanQuic,
        audio_output: "System Default Output".into(),
        mic_input: "System Default Microphone".into(),
    }
}
