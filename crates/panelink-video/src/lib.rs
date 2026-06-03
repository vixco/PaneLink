use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};
use tiny_http::{Header, Response, Server, StatusCode};

use openh264::{
    encoder::{
        BitRate, Complexity, Encoder, EncoderConfig, FrameRate, RateControlMode, SpsPpsStrategy,
        UsageType,
    },
    formats::{RgbaSliceU8, YUVBuffer},
    OpenH264API,
};

pub const VIDEO_SIGNALING_PORT: u16 = 48170;
pub const H264_STREAM_PORT: u16 = 48172;
pub const H264_STREAM_PATH: &str = "/h264";
const DEFAULT_H264_TARGET_FPS: u16 = 60;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H264StreamRequest {
    pub width: u32,
    pub height: u32,
    pub target_fps: u16,
    pub target_bitrate_mbps: u16,
    pub quality: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct H264StreamSession {
    pub active: bool,
    pub endpoint: String,
    pub port: u16,
    pub transport: String,
    pub codec: String,
    pub target_fps: u16,
    pub target_bitrate_mbps: u16,
    pub message: String,
}

pub fn backend_report() -> VideoBackendReport {
    if cfg!(target_os = "macos") {
        return VideoBackendReport {
            backend: "ScreenCaptureKit + OpenH264".into(),
            state: VideoBackendState::Available,
            available: true,
            can_start_source_stream: true,
            transport: "H.264 Annex-B over PaneLink LAN HTTP stream".into(),
            codec: "H.264 OpenH264".into(),
            hardware_accelerated: false,
            message: "Native H.264 LAN video stream is available; allow Screen Recording and Accessibility for input.".into(),
            actions: vec![
                "Allow Screen Recording for PaneLink on macOS".into(),
                "Allow Accessibility for keyboard and mouse forwarding".into(),
            ],
        };
    }

    if cfg!(target_os = "windows") {
        return VideoBackendReport {
            backend: "WebRTC hardware receiver".into(),
            state: VideoBackendState::ReceiverOnly,
            available: true,
            can_start_source_stream: false,
            transport: "H.264 Annex-B over PaneLink LAN HTTP stream".into(),
            codec: "H.264 WebCodecs decode".into(),
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

    let mut session = plan_video_session(request)?;
    let stream = configure_h264_control_stream(H264StreamRequest {
        width: session.width,
        height: session.height,
        target_fps: session.target_fps,
        target_bitrate_mbps: session.target_bitrate_mbps,
        quality: session.quality.clone(),
    })?;
    session.endpoint = stream.endpoint;
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
        "http://127.0.0.1:{VIDEO_SIGNALING_PORT}{H264_STREAM_PATH}?session={}&screens={}&codec={}&fps={}",
        request.screen_count,
        percent_encode(&id),
        percent_encode(codec),
        target_fps
    );
    let session = VideoSession {
        id,
        active: true,
        endpoint,
        control_address: request.control_address,
        transport: "H.264 LAN stream".into(),
        codec: codec.into(),
        quality: quality.into(),
        target_fps,
        target_bitrate_mbps,
        screen_count: request.screen_count,
        width: request.width,
        height: request.height,
        message: "Native H.264 LAN video session negotiated; PNG frame polling disabled.".into(),
    };

    Ok(session)
}

pub fn start_h264_stream_server(request: H264StreamRequest) -> Result<H264StreamSession, String> {
    let request = set_active_h264_config(request)?;

    let port = h264_server_slot().get_or_init(start_h264_server).clone()?;
    Ok(H264StreamSession {
        active: true,
        endpoint: format!(
            "http://127.0.0.1:{port}/h264?fps={}&bitrateMbps={}&quality={}",
            request.target_fps,
            request.target_bitrate_mbps,
            percent_encode(&request.quality)
        ),
        port,
        transport: "H.264 LAN stream".into(),
        codec: "H.264 OpenH264".into(),
        target_fps: request.target_fps,
        target_bitrate_mbps: request.target_bitrate_mbps,
        message: "H.264 stream server is running.".into(),
    })
}

pub fn configure_h264_control_stream(
    request: H264StreamRequest,
) -> Result<H264StreamSession, String> {
    let request = set_active_h264_config(request)?;

    Ok(H264StreamSession {
        active: true,
        endpoint: format!(
            "http://127.0.0.1:{VIDEO_SIGNALING_PORT}{H264_STREAM_PATH}?fps={}&bitrateMbps={}&quality={}",
            request.target_fps,
            request.target_bitrate_mbps,
            percent_encode(&request.quality)
        ),
        port: VIDEO_SIGNALING_PORT,
        transport: "H.264 LAN stream".into(),
        codec: "H.264 OpenH264".into(),
        target_fps: request.target_fps,
        target_bitrate_mbps: request.target_bitrate_mbps,
        message: "H.264 stream is available on the PaneLink control server.".into(),
    })
}

pub fn respond_h264_stream_request(request: tiny_http::Request) {
    if request.method().as_str() == "OPTIONS" {
        let _ = request.respond(empty_response(StatusCode(204)));
        return;
    }

    match active_h264_config() {
        Ok(config) => {
            let _ = request.respond(h264_stream_response(config));
        }
        Err(error) => {
            let _ = request.respond(text_response(error, StatusCode(500)));
        }
    }
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

fn h264_config_slot() -> &'static Mutex<H264StreamRequest> {
    static CONFIG: OnceLock<Mutex<H264StreamRequest>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(default_h264_request()))
}

fn h264_server_slot() -> &'static OnceLock<Result<u16, String>> {
    static SERVER: OnceLock<Result<u16, String>> = OnceLock::new();
    &SERVER
}

fn default_h264_request() -> H264StreamRequest {
    H264StreamRequest {
        width: 1920,
        height: 1080,
        target_fps: DEFAULT_H264_TARGET_FPS,
        target_bitrate_mbps: 36,
        quality: "Sharp".into(),
    }
}

fn set_active_h264_config(request: H264StreamRequest) -> Result<H264StreamRequest, String> {
    let request = normalized_h264_request(request);
    *h264_config_slot()
        .lock()
        .map_err(|_| "H.264 stream configuration is unavailable".to_string())? = request.clone();

    Ok(request)
}

fn normalized_h264_request(request: H264StreamRequest) -> H264StreamRequest {
    H264StreamRequest {
        width: request.width.clamp(640, 7680),
        height: request.height.clamp(360, 4320),
        target_fps: request.target_fps.clamp(30, 60),
        target_bitrate_mbps: request.target_bitrate_mbps.clamp(8, 120),
        quality: normalize_quality(&request.quality).into(),
    }
}

fn start_h264_server() -> Result<u16, String> {
    let server = Server::http(("0.0.0.0", H264_STREAM_PORT)).map_err(|error| {
        format!("H.264 stream server could not bind port {H264_STREAM_PORT}: {error}")
    })?;

    thread::Builder::new()
        .name("panelink-h264-stream-server".into())
        .spawn(move || run_h264_server(server))
        .map_err(|error| format!("H.264 stream server could not start: {error}"))?;

    Ok(H264_STREAM_PORT)
}

fn run_h264_server(server: Server) {
    for request in server.incoming_requests() {
        let method = request.method().as_str().to_string();
        let path = request.url().split('?').next().unwrap_or("/");

        if method == "OPTIONS" {
            let _ = request.respond(empty_response(StatusCode(204)));
        } else if method == "GET" && path == H264_STREAM_PATH {
            match active_h264_config() {
                Ok(config) => {
                    let _ = request.respond(h264_stream_response(config));
                }
                Err(error) => {
                    let _ = request.respond(text_response(error, StatusCode(500)));
                }
            }
        } else if method == "GET" && path == "/health" {
            let _ = request.respond(text_response("ok h264=ready", StatusCode(200)));
        } else {
            let _ = request.respond(text_response(
                "PaneLink H.264 stream server",
                StatusCode(200),
            ));
        }
    }
}

fn active_h264_config() -> Result<H264StreamRequest, String> {
    h264_config_slot()
        .lock()
        .map_err(|_| "H.264 stream configuration is unavailable".to_string())
        .map(|config| config.clone())
}

fn h264_stream_response(config: H264StreamRequest) -> Response<H264StreamReader> {
    with_common_headers(
        Response::new(
            StatusCode(200),
            vec![header("Content-Type", "video/h264")],
            H264StreamReader::new(config),
            None,
            None,
        )
        .with_header(header("Transfer-Encoding", "chunked")),
    )
}

fn empty_response(status: StatusCode) -> Response<Cursor<Vec<u8>>> {
    with_common_headers(Response::from_data(Vec::new()).with_status_code(status))
}

fn text_response(text: impl Into<String>, status: StatusCode) -> Response<Cursor<Vec<u8>>> {
    with_common_headers(
        Response::from_string(text.into())
            .with_status_code(status)
            .with_header(header("Content-Type", "text/plain; charset=utf-8")),
    )
}

fn with_common_headers<R: Read>(response: Response<R>) -> Response<R> {
    response
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Methods", "GET, OPTIONS"))
        .with_header(header("Access-Control-Allow-Headers", "*"))
        .with_header(header("Access-Control-Allow-Private-Network", "true"))
        .with_header(header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate",
        ))
}

fn header(name: &'static str, value: &'static str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header should be valid")
}

struct H264StreamReader {
    config: H264StreamRequest,
    encoder: Option<Encoder>,
    pending: Vec<u8>,
    pending_offset: usize,
    frame_id: u64,
}

impl H264StreamReader {
    fn new(config: H264StreamRequest) -> Self {
        Self {
            config,
            encoder: None,
            pending: Vec::new(),
            pending_offset: 0,
            frame_id: 0,
        }
    }

    fn fill_pending(&mut self) -> std::io::Result<()> {
        if self.pending_offset < self.pending.len() {
            return Ok(());
        }

        let frame_duration =
            Duration::from_micros(1_000_000 / u64::from(self.config.target_fps.max(1)));
        thread::sleep(frame_duration);

        let frame = panelink_capture::capture_primary_rgba().map_err(io_error)?;
        let normalized = normalized_rgba_frame(frame)?;
        let source = RgbaSliceU8::new(
            &normalized.rgba,
            (normalized.width as usize, normalized.height as usize),
        );
        let yuv = YUVBuffer::from_rgb_source(source);
        let force_keyframe = self
            .frame_id
            .is_multiple_of(u64::from(self.config.target_fps.max(1)));
        let encoder = self.encoder()?;
        if force_keyframe {
            encoder.force_intra_frame();
        }
        let encoded = encoder
            .encode(&yuv)
            .map_err(|error| io_error(format!("Could not encode H.264 frame: {error}")))?
            .to_vec();

        self.frame_id = self.frame_id.saturating_add(1);
        self.pending.clear();
        self.pending
            .extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        self.pending.extend_from_slice(&encoded);
        self.pending_offset = 0;

        Ok(())
    }

    fn encoder(&mut self) -> std::io::Result<&mut Encoder> {
        if self.encoder.is_none() {
            let bitrate_bps = u32::from(self.config.target_bitrate_mbps) * 1_000_000;
            let config = EncoderConfig::new()
                .bitrate(BitRate::from_bps(bitrate_bps))
                .max_frame_rate(FrameRate::from_hz(f32::from(self.config.target_fps)))
                .rate_control_mode(RateControlMode::Bitrate)
                .usage_type(UsageType::ScreenContentRealTime)
                .sps_pps_strategy(SpsPpsStrategy::SpsPpsListing)
                .complexity(Complexity::Low)
                .skip_frames(false)
                .debug(false);
            let encoder =
                Encoder::with_api_config(OpenH264API::from_source(), config).map_err(|error| {
                    io_error(format!("Could not initialize OpenH264 encoder: {error}"))
                })?;
            self.encoder = Some(encoder);
        }

        Ok(self
            .encoder
            .as_mut()
            .expect("encoder was initialized above"))
    }
}

impl Read for H264StreamReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        self.fill_pending()?;
        let remaining = &self.pending[self.pending_offset..];
        let len = remaining.len().min(output.len());
        output[..len].copy_from_slice(&remaining[..len]);
        self.pending_offset += len;
        Ok(len)
    }
}

struct NormalizedRgbaFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn normalized_rgba_frame(
    frame: panelink_capture::CapturedFrame,
) -> std::io::Result<NormalizedRgbaFrame> {
    let width = frame.width - (frame.width % 2);
    let height = frame.height - (frame.height % 2);
    if width == 0 || height == 0 {
        return Err(io_error("Captured frame has invalid dimensions"));
    }

    if width == frame.width && height == frame.height {
        return Ok(NormalizedRgbaFrame {
            width,
            height,
            rgba: frame.rgba,
        });
    }

    let source_stride = frame.width as usize * 4;
    let target_stride = width as usize * 4;
    let mut rgba = Vec::with_capacity(target_stride * height as usize);
    for row in 0..height as usize {
        let start = row * source_stride;
        let end = start + target_stride;
        rgba.extend_from_slice(
            frame
                .rgba
                .get(start..end)
                .ok_or_else(|| io_error("Captured frame buffer is shorter than expected"))?,
        );
    }

    Ok(NormalizedRgbaFrame {
        width,
        height,
        rgba,
    })
}

fn io_error(error: impl Into<String>) -> std::io::Error {
    std::io::Error::other(error.into())
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
        "Balanced" => 60,
        _ => 60,
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
    let _ = quality;
    "H.264 OpenH264"
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
    fn video_session_contract_is_h264_lan_stream_not_png_polling() {
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

        assert_eq!(session.transport, "H.264 LAN stream");
        assert!(session.endpoint.starts_with("http://127.0.0.1:48170/h264"));
        assert_eq!(session.codec, "H.264 OpenH264");
        assert!(!session.endpoint.to_lowercase().contains("png"));
        assert!(!session.endpoint.to_lowercase().contains("/frame"));
        assert!(!session.control_address.contains("48171"));
    }

    #[test]
    fn control_stream_uses_existing_control_port_to_avoid_extra_firewall_hole() {
        let session = configure_h264_control_stream(H264StreamRequest {
            width: 1920,
            height: 1080,
            target_fps: 60,
            target_bitrate_mbps: 28,
            quality: "Low latency".into(),
        })
        .expect("control stream should configure");

        assert_eq!(session.port, VIDEO_SIGNALING_PORT);
        assert!(session.endpoint.starts_with("http://127.0.0.1:48170/h264"));
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

        assert_eq!(low.target_fps, 60);
        assert_eq!(sharp.target_fps, 60);
        assert!(sharp.target_bitrate_mbps > low.target_bitrate_mbps);
        assert_eq!(sharp.codec, "H.264 OpenH264");
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
    fn start_video_session_matches_platform_source_capability() {
        let result = start_video_session(VideoSessionRequest {
            source_peer_id: "mac".into(),
            receiver_peer_id: "windows".into(),
            screen_count: 1,
            quality: "Low latency".into(),
            width: 1920,
            height: 1080,
            refresh_hz: 120,
            control_address: "http://192.168.1.24:48170".into(),
        });

        if cfg!(target_os = "macos") {
            let session = result.expect("macOS should start the OpenH264 source stream");
            assert_eq!(session.transport, "H.264 LAN stream");
            assert_eq!(session.codec, "H.264 OpenH264");
            assert!(session.endpoint.contains(":48170/h264"));
            return;
        }

        let error = result.expect_err("receiver-only platforms must not start a source stream");

        assert!(
            error.contains("cannot start the source video engine")
                || error.contains("requires macOS source")
        );
    }
}
