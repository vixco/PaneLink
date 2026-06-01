use panelink_core::{
    AudioCapabilities, AudioDevice, Capabilities, CaptureState, DisplayCapabilities,
    PermissionState, PermissionStatus, RoutingState, SessionSnapshot, VirtualDisplayState,
};

#[tauri::command]
fn list_peers() -> Vec<panelink_core::Peer> {
    panelink_discovery::list_cached_peers()
}

#[tauri::command]
fn advertise_peer() -> panelink_discovery::AdvertisementPayload {
    panelink_discovery::advertise_payload()
}

#[tauri::command]
fn issue_pairing_token(peer_id: String) -> Result<panelink_discovery::PairingToken, String> {
    panelink_discovery::issue_pairing_token(&peer_id)
        .ok_or_else(|| format!("peer '{peer_id}' is not in the discovery cache"))
}

#[tauri::command]
fn get_session_snapshot() -> SessionSnapshot {
    panelink_transport::get_session_snapshot()
}

#[tauri::command]
fn get_transport_state() -> panelink_transport::SessionState {
    panelink_transport::session_state()
}

#[tauri::command]
fn connect_peer(peer_id: String) -> Result<SessionSnapshot, panelink_transport::SessionError> {
    panelink_transport::connect_peer(peer_id)
}

#[tauri::command]
fn disconnect_peer() -> SessionSnapshot {
    panelink_transport::disconnect_peer()
}

#[tauri::command]
fn ping_peer(
    peer_id: String,
) -> Result<panelink_transport::PingSample, panelink_transport::SessionError> {
    panelink_transport::ping_peer(peer_id)
}

#[tauri::command]
fn list_audio_devices() -> Vec<AudioDevice> {
    panelink_audio::list_devices()
}

#[tauri::command]
fn get_audio_route_catalog() -> panelink_audio::AudioRouteCatalog {
    panelink_audio::get_route_catalog()
}

#[tauri::command]
fn get_input_backend_report() -> panelink_input::InputBackendReport {
    panelink_input::backend_report()
}

#[tauri::command]
fn submit_input_batch(batch: panelink_input::InputEventBatch) -> panelink_input::InputBatchReceipt {
    panelink_input::accept_batch(batch)
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_peers,
            advertise_peer,
            issue_pairing_token,
            get_session_snapshot,
            get_transport_state,
            connect_peer,
            disconnect_peer,
            ping_peer,
            list_audio_devices,
            get_audio_route_catalog,
            get_input_backend_report,
            submit_input_batch,
            get_capabilities,
            get_permissions
        ])
        .run(tauri::generate_context!())
        .expect("failed to run PaneLink");
}
