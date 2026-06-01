use panelink_core::{
    demo_session, AudioCapabilities, AudioDevice, Capabilities, CaptureState, ConnectionStatus,
    DisplayCapabilities, PermissionState, PermissionStatus, RoutingState, SessionSnapshot,
    VirtualDisplayState,
};

#[tauri::command]
fn list_peers() -> Vec<panelink_core::Peer> {
    panelink_discovery::list_cached_peers()
}

#[tauri::command]
fn get_session_snapshot() -> SessionSnapshot {
    demo_session(ConnectionStatus::Ready, Some("windows-desk".into()))
}

#[tauri::command]
fn connect_peer(peer_id: String) -> SessionSnapshot {
    demo_session(ConnectionStatus::Connected, Some(peer_id))
}

#[tauri::command]
fn disconnect_peer() -> SessionSnapshot {
    demo_session(ConnectionStatus::Ready, None)
}

#[tauri::command]
fn list_audio_devices() -> Vec<AudioDevice> {
    panelink_audio::list_devices()
}

#[tauri::command]
fn get_capabilities() -> Capabilities {
    let capture = panelink_capture::current_capture_backend();

    Capabilities {
        app_version: env!("CARGO_PKG_VERSION").into(),
        peer_id: panelink_core::local_peer_id(),
        platform: std::env::consts::OS.into(),
        video_encoders: vec![
            "H.264 low latency".into(),
            "HEVC hardware planned".into(),
            "AV1 planned".into(),
        ],
        transport: vec![
            panelink_transport::default_transport_plan().primary,
            panelink_discovery::SERVICE_NAME.into(),
        ],
        audio: AudioCapabilities {
            output_capture: true,
            microphone_capture: true,
            virtual_routing: RoutingState::Planned,
        },
        display: DisplayCapabilities {
            capture: if capture.available {
                CaptureState::Available
            } else {
                CaptureState::Stub
            },
            virtual_display: VirtualDisplayState::DriverRequired,
        },
    }
}

#[tauri::command]
fn get_permissions() -> Vec<PermissionState> {
    vec![
        PermissionState {
            key: "screen-capture".into(),
            label: "Screen capture".into(),
            status: if cfg!(target_os = "macos") {
                PermissionStatus::Required
            } else {
                PermissionStatus::Granted
            },
            detail: "macOS uses ScreenCaptureKit; Windows uses DXGI Desktop Duplication.".into(),
        },
        PermissionState {
            key: "input-control".into(),
            label: "Input control".into(),
            status: if cfg!(target_os = "macos") {
                PermissionStatus::Required
            } else {
                PermissionStatus::Granted
            },
            detail: "macOS requires Accessibility permission; Windows uses SendInput.".into(),
        },
        PermissionState {
            key: "virtual-audio".into(),
            label: "Virtual audio routing".into(),
            status: PermissionStatus::Unsupported,
            detail: "Full default speaker/mic takeover requires signed virtual audio drivers."
                .into(),
        },
    ]
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_peers,
            get_session_snapshot,
            connect_peer,
            disconnect_peer,
            list_audio_devices,
            get_capabilities,
            get_permissions
        ])
        .run(tauri::generate_context!())
        .expect("failed to run PaneLink");
}
