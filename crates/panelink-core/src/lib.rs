use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use uuid::Uuid;

pub mod display_plan;

pub use display_plan::{
    auto_fit_mode, fit_resolution, layout_from_topology, plan_add_screen, DisplayLayout,
    DisplayLayoutEntry, DisplayMode, DisplayPlanError, DisplayRect, DisplaySessionPlan,
    DisplayTopology, FittedResolution, PlannedScreen, RollbackReason, RollbackSnapshot,
    ScaleInsets, SourceDisplay, TargetDisplay, TargetDisplayRole,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatingSystem {
    #[serde(rename = "macOS")]
    MacOs,
    Windows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub status: ConnectionStatus,
    pub active_peer_id: Option<String>,
    pub display: String,
    pub resolution: String,
    pub display_plan: Option<DisplaySessionPlan>,
    pub rollback_snapshot: Option<RollbackSnapshot>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenRole {
    Primary,
    Extended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaleMode {
    AutoFit,
    Native,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScreenStatus {
    Ready,
    Connected,
    RollbackPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Ready,
    Connecting,
    Connected,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportMode {
    #[serde(rename = "LAN QUIC")]
    LanQuic,
    #[serde(rename = "WebRTC")]
    WebRtc,
    #[serde(rename = "Local preview")]
    LocalPreview,
}

pub fn local_peer_id() -> String {
    static LOCAL_PEER_ID: OnceLock<String> = OnceLock::new();

    LOCAL_PEER_ID
        .get_or_init(|| Uuid::new_v4().to_string())
        .clone()
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
    let source_display = SourceDisplay {
        id: "macbook-built-in".into(),
        name: "MacBook display".into(),
        native_mode: DisplayMode {
            width: 2560,
            height: 1600,
            refresh_hz: 120,
        },
        current_mode: DisplayMode {
            width: 2560,
            height: 1600,
            refresh_hz: 120,
        },
    };
    let target_display = TargetDisplay {
        id: "windows-display-1".into(),
        name: "Windows Display 1".into(),
        role: TargetDisplayRole::Primary,
        native_mode: DisplayMode {
            width: 2560,
            height: 1440,
            refresh_hz: 120,
        },
        current_mode: DisplayMode {
            width: 2560,
            height: 1440,
            refresh_hz: 120,
        },
        supported_modes: vec![
            DisplayMode {
                width: 2560,
                height: 1440,
                refresh_hz: 120,
            },
            DisplayMode {
                width: 1920,
                height: 1200,
                refresh_hz: 60,
            },
            DisplayMode {
                width: 1920,
                height: 1080,
                refresh_hz: 120,
            },
        ],
        bounds: DisplayRect {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        },
        attached: true,
    };
    let windows_pc = DisplayTopology {
        pc_id: "windows-desk".into(),
        pc_name: "Windows Desk".into(),
        displays: vec![target_display],
    };
    let display_plan = plan_add_screen(
        "demo-plan",
        active_peer_id.as_deref().unwrap_or("windows-desk"),
        windows_pc,
        source_display,
        "windows-display-1",
    )
    .ok();
    let rollback_snapshot = display_plan
        .as_ref()
        .map(|plan| plan.rollback_snapshot.clone());
    let screens = display_plan
        .as_ref()
        .map(|plan| {
            plan.screens
                .iter()
                .map(|screen| screen.remote_screen.clone())
                .collect()
        })
        .unwrap_or_default();

    SessionSnapshot {
        status,
        active_peer_id,
        display: "Desk monitor".into(),
        resolution: "2560 x 1440 @ 120 Hz".into(),
        display_plan,
        rollback_snapshot,
        screens,
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
