use serde::{Deserialize, Serialize};
use std::{
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

pub const VIDEO_SIGNALING_PORT: u16 = 48170;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoBackendReport {
    pub backend: String,
    pub state: VideoBackendState,
    pub available: bool,
    pub can_start_source_stream: bool,
    pub transport: String,
    pub codec: String,
    pub hardware_accelerated: bool,
    pub message: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VideoBackendState {
    Available,
    PermissionRequired,
    ReceiverOnly,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSessionRequest {
    pub source_peer_id: String,
    pub receiver_peer_id: String,
    pub screen_count: u8,
    pub quality: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u16,
    pub control_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSession {
    pub id: String,
    pub active: bool,
    pub endpoint: String,
    pub control_address: String,
    pub transport: String,
    pub codec: String,
    pub quality: String,
    pub target_fps: u16,
    pub target_bitrate_mbps: u16,
    pub screen_count: u8,
    pub width: u32,
    pub height: u32,
    pub message: String,
}

pub fn backend_report() -> VideoBackendReport {
    if cfg!(target_os = "macos") {
        return VideoBackendReport {
            backend: "ScreenCaptureKit + VideoToolbox".into(),
            state: VideoBackendState::PermissionRequired,
            available: false,
            can_start_source_stream: false,
            transport: "WebRTC/RTP over PaneLink LAN signaling".into(),
            codec: "H.264 hardware encode; HEVC for Sharp when available".into(),
            hardware_accelerated: true,
            message: "Native remote-desktop video engine is not installed yet; PaneLink will not start a fake screenshot stream.".into(),
            actions: vec![
                "Install the PaneLink native video engine".into(),
                "Then allow Screen Recording and Accessibility for PaneLink".into(),
            ],
        };
    }

    if cfg!(target_os = "windows") {
        return VideoBackendReport {
            backend: "WebRTC hardware receiver".into(),
            state: VideoBackendState::ReceiverOnly,
            available: true,
            can_start_source_stream: false,
            transport: "WebRTC/RTP over PaneLink LAN signaling".into(),
            codec: "H.264 hardware decode".into(),
            hardware_accelerated: true,
            message: "Receiver is ready, but this device cannot start the source video engine."
                .into(),
            actions: vec!["Allow PaneLink through Windows Firewall for LAN control".into()],
        };
    }

    VideoBackendReport {
        backend: "Unsupported platform".into(),
        state: VideoBackendState::Unsupported,
        available: false,
        can_start_source_stream: false,
        transport: "unavailable".into(),
        codec: "unavailable".into(),
        hardware_accelerated: false,
        message: "PaneLink remote-desktop video requires macOS source or Windows receiver.".into(),
        actions: Vec::new(),
    }
}

pub fn start_video_session(request: VideoSessionRequest) -> Result<VideoSession, String> {
    validate_request(&request)?;
    let backend = backend_report();
    if !backend.can_start_source_stream {
        return Err(backend.message);
    }

    let session = plan_video_session(request)?;
    *session_slot()
        .lock()
        .expect("video session mutex should not be poisoned") = Some(session.clone());

    Ok(session)
}

pub fn plan_video_session(request: VideoSessionRequest) -> Result<VideoSession, String> {
    validate_request(&request)?;

    let quality = normalize_quality(&request.quality);
    let target_fps = target_fps_for_quality(quality);
    let target_bitrate_mbps = target_bitrate_for_quality(quality, request.screen_count);
    let codec = codec_for_quality(quality);
    let id = format!("video-{}-{}", request.source_peer_id, now_unix_ms());
    let endpoint = format!(
        "webrtc+rtp://{}/panelink/{}?screens={}&codec={}&fps={}",
        request.receiver_peer_id,
        id,
        request.screen_count,
        percent_encode(codec),
        target_fps
    );
    let session = VideoSession {
        id,
        active: true,
        endpoint,
        control_address: request.control_address,
        transport: "WebRTC/RTP".into(),
        codec: codec.into(),
        quality: quality.into(),
        target_fps,
        target_bitrate_mbps,
        screen_count: request.screen_count,
        width: request.width,
        height: request.height,
        message: "Native remote-desktop video session negotiated; PNG frame polling disabled."
            .into(),
    };

    Ok(session)
}

pub fn current_video_session() -> Option<VideoSession> {
    session_slot()
        .lock()
        .expect("video session mutex should not be poisoned")
        .clone()
}

pub fn stop_video_session() -> Option<VideoSession> {
    session_slot()
        .lock()
        .expect("video session mutex should not be poisoned")
        .take()
        .map(|mut session| {
            session.active = false;
            session.message = "Native remote-desktop video session stopped.".into();
            session
        })
}

fn validate_request(request: &VideoSessionRequest) -> Result<(), String> {
    if request.source_peer_id.trim().is_empty() {
        return Err("Video session is missing source peer id".into());
    }
    if request.receiver_peer_id.trim().is_empty() {
        return Err("Video session is missing receiver peer id".into());
    }
    if !(1..=3).contains(&request.screen_count) {
        return Err("Video session supports one to three remote displays".into());
    }
    if request.control_address.trim().is_empty() {
        return Err("Video session is missing input control address".into());
    }
    if request.control_address.contains("/frame") || request.control_address.contains("48171") {
        return Err(
            "Input control must use the PaneLink control server, not the PNG frame server".into(),
        );
    }

    Ok(())
}

fn session_slot() -> &'static Mutex<Option<VideoSession>> {
    static SESSION: OnceLock<Mutex<Option<VideoSession>>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(None))
}

fn normalize_quality(quality: &str) -> &'static str {
    match quality {
        "Sharp" => "Sharp",
        "Balanced" => "Balanced",
        _ => "Low latency",
    }
}

fn target_fps_for_quality(quality: &str) -> u16 {
    match quality {
        "Sharp" => 60,
        "Balanced" => 90,
        _ => 120,
    }
}

fn target_bitrate_for_quality(quality: &str, screen_count: u8) -> u16 {
    let per_screen = match quality {
        "Sharp" => 52,
        "Balanced" => 36,
        _ => 28,
    };

    per_screen * u16::from(screen_count.max(1))
}

fn codec_for_quality(quality: &str) -> &'static str {
    match quality {
        "Sharp" => "HEVC VideoToolbox",
        _ => "H.264 VideoToolbox",
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
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
    fn video_session_contract_is_not_png_polling() {
        let session = plan_video_session(VideoSessionRequest {
            source_peer_id: "mac".into(),
            receiver_peer_id: "windows".into(),
            screen_count: 2,
            quality: "Low latency".into(),
            width: 2560,
            height: 1440,
            refresh_hz: 120,
            control_address: "http://192.168.1.24:48170".into(),
        })
        .expect("video session should start");

        assert_eq!(session.transport, "WebRTC/RTP");
        assert!(session.endpoint.starts_with("webrtc+rtp://"));
        assert!(session.codec.contains("VideoToolbox"));
        assert!(!session.endpoint.to_lowercase().contains("png"));
        assert!(!session.endpoint.to_lowercase().contains("/frame"));
        assert!(!session.control_address.contains("48171"));
    }

    #[test]
    fn quality_modes_map_to_video_targets() {
        let low = plan_video_session(VideoSessionRequest {
            source_peer_id: "mac".into(),
            receiver_peer_id: "windows".into(),
            screen_count: 1,
            quality: "Low latency".into(),
            width: 1920,
            height: 1080,
            refresh_hz: 120,
            control_address: "http://192.168.1.24:48170".into(),
        })
        .expect("low latency should start");
        let sharp = plan_video_session(VideoSessionRequest {
            quality: "Sharp".into(),
            ..VideoSessionRequest {
                source_peer_id: "mac".into(),
                receiver_peer_id: "windows".into(),
                screen_count: 1,
                quality: "Low latency".into(),
                width: 1920,
                height: 1080,
                refresh_hz: 120,
                control_address: "http://192.168.1.24:48170".into(),
            }
        })
        .expect("sharp should start");

        assert_eq!(low.target_fps, 120);
        assert_eq!(sharp.target_fps, 60);
        assert!(sharp.target_bitrate_mbps > low.target_bitrate_mbps);
        assert!(sharp.codec.contains("HEVC"));
    }

    #[test]
    fn control_address_rejects_frame_server() {
        let error = start_video_session(VideoSessionRequest {
            source_peer_id: "mac".into(),
            receiver_peer_id: "windows".into(),
            screen_count: 1,
            quality: "Low latency".into(),
            width: 1920,
            height: 1080,
            refresh_hz: 120,
            control_address: "http://192.168.1.24:48171/frame".into(),
        })
        .expect_err("frame server must not be used for input control");

        assert!(error.contains("control server"));
    }

    #[test]
    fn start_video_session_does_not_fake_a_missing_source_engine() {
        let error = start_video_session(VideoSessionRequest {
            source_peer_id: "mac".into(),
            receiver_peer_id: "windows".into(),
            screen_count: 1,
            quality: "Low latency".into(),
            width: 1920,
            height: 1080,
            refresh_hz: 120,
            control_address: "http://192.168.1.24:48170".into(),
        })
        .expect_err("missing native video engine must not create an active session");

        assert!(error.contains("video engine"));
    }
}
