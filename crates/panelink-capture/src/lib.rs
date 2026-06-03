use serde::{Deserialize, Serialize};
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock, RwLock,
    },
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server, StatusCode};
use xcap::{image::ImageFormat, Monitor};

pub const FRAME_SERVER_PORT: u16 = 48171;
const DEFAULT_FRAME_INTERVAL_MS: u64 = 66;
const PANELINK_DISPLAY_NAME: &str = "panelink";
static FRAME_INTERVAL_MS: AtomicU64 = AtomicU64::new(DEFAULT_FRAME_INTERVAL_MS);

#[derive(Debug, Default)]
struct FrameCache {
    frame: Option<Vec<u8>>,
    error: Option<String>,
    sequence: u64,
}

type SharedFrameCache = Arc<RwLock<FrameCache>>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonitorDescriptor {
    index: usize,
    name: String,
    is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBackend {
    pub name: String,
    pub available: bool,
    pub requires_permission: bool,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn current_capture_backend() -> CaptureBackend {
    #[cfg(target_os = "windows")]
    {
        return CaptureBackend {
            name: "DXGI Desktop Duplication".into(),
            available: true,
            requires_permission: false,
            note: format!("Native frame capture server listens on port {FRAME_SERVER_PORT}."),
        };
    }

    #[cfg(target_os = "macos")]
    {
        return CaptureBackend {
            name: "ScreenCaptureKit".into(),
            available: true,
            requires_permission: true,
            note: format!(
                "Screen Recording permission is required; frame server listens on port {FRAME_SERVER_PORT}."
            ),
        };
    }

    #[allow(unreachable_code)]
    CaptureBackend {
        name: "FakeFrameSource".into(),
        available: false,
        requires_permission: false,
        note: "Unsupported platform; test frame source only.".into(),
    }
}

pub fn start_frame_server() -> Result<u16, String> {
    static STARTED: OnceLock<Result<u16, String>> = OnceLock::new();

    STARTED
        .get_or_init(|| {
            let server = Server::http(("0.0.0.0", FRAME_SERVER_PORT)).map_err(|error| {
                format!("Frame server could not bind port {FRAME_SERVER_PORT}: {error}")
            })?;
            let cache = Arc::new(RwLock::new(FrameCache::default()));

            start_capture_loop(Arc::clone(&cache))?;

            thread::Builder::new()
                .name("panelink-frame-server".into())
                .spawn(move || run_frame_server(server, cache))
                .map_err(|error| format!("Frame server could not start: {error}"))?;

            Ok(FRAME_SERVER_PORT)
        })
        .clone()
}

fn start_capture_loop(cache: SharedFrameCache) -> Result<(), String> {
    thread::Builder::new()
        .name("panelink-frame-capture".into())
        .spawn(move || loop {
            let delay = match capture_primary_png() {
                Ok(frame) => {
                    let mut state = cache.write().expect("frame cache should not be poisoned");
                    state.frame = Some(frame);
                    state.error = None;
                    state.sequence = state.sequence.saturating_add(1);
                    Duration::from_millis(requested_frame_interval_ms())
                }
                Err(error) => {
                    let mut state = cache.write().expect("frame cache should not be poisoned");
                    let permission_related = is_permission_related_capture_error(&error);
                    state.error = Some(error);
                    if permission_related {
                        Duration::from_secs(30)
                    } else {
                        Duration::from_millis(1000)
                    }
                }
            };

            thread::sleep(delay);
        })
        .map(|_| ())
        .map_err(|error| format!("Frame capture loop could not start: {error}"))
}

fn is_permission_related_capture_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "permission",
        "privacy",
        "screen recording",
        "authorized",
        "authorised",
        "denied",
        "record",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

pub fn capture_primary_png() -> Result<Vec<u8>, String> {
    let image = capture_primary_image()?;
    let mut bytes = Cursor::new(Vec::new());

    image
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| format!("Could not encode captured frame: {error}"))?;

    Ok(bytes.into_inner())
}

pub fn capture_primary_rgba() -> Result<CapturedFrame, String> {
    let image = capture_primary_image()?;
    let width = image.width();
    let height = image.height();

    Ok(CapturedFrame {
        width,
        height,
        rgba: image.into_raw(),
    })
}

fn capture_primary_image() -> Result<xcap::image::RgbaImage, String> {
    let monitors = Monitor::all().map_err(|error| format!("Could not list monitors: {error}"))?;
    let descriptors = monitor_descriptors(&monitors);
    let monitor_index = preferred_monitor_index(&descriptors)
        .or_else(|| (!monitors.is_empty()).then_some(0))
        .ok_or_else(|| "No monitor found to capture".to_string())?;
    let monitor = monitors
        .get(monitor_index)
        .ok_or_else(|| format!("Selected monitor index {monitor_index} is unavailable"))?;
    let image = monitor
        .capture_image()
        .map_err(|error| format!("Could not capture monitor: {error}"))?;

    Ok(image)
}

fn monitor_descriptors(monitors: &[Monitor]) -> Vec<MonitorDescriptor> {
    monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| MonitorDescriptor {
            index,
            name: monitor.name().unwrap_or_default(),
            is_primary: monitor.is_primary().unwrap_or(false),
        })
        .collect()
}

fn preferred_monitor_index(monitors: &[MonitorDescriptor]) -> Option<usize> {
    monitors
        .iter()
        .find(|monitor| {
            monitor
                .name
                .to_ascii_lowercase()
                .contains(PANELINK_DISPLAY_NAME)
        })
        .or_else(|| monitors.iter().find(|monitor| !monitor.is_primary))
        .or_else(|| monitors.first())
        .map(|monitor| monitor.index)
}

fn run_frame_server(server: Server, cache: SharedFrameCache) {
    for request in server.incoming_requests() {
        let method = request.method().as_str().to_string();
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/");
        let response = if method == "OPTIONS" {
            empty_response(StatusCode(204))
        } else {
            match path {
                "/frame" => {
                    update_requested_frame_interval(&url);
                    cached_frame_response(&cache)
                }
                "/health" => cached_health_response(&cache),
                _ => text_response("PaneLink frame server", StatusCode(200)),
            }
        };

        let _ = request.respond(response);
    }
}

fn requested_frame_interval_ms() -> u64 {
    FRAME_INTERVAL_MS.load(Ordering::Relaxed)
}

fn update_requested_frame_interval(url: &str) {
    let quality = url
        .split_once('?')
        .and_then(|(_, query)| query_value(query, "quality"))
        .unwrap_or_default();
    let interval_ms = frame_interval_for_quality(&quality);

    FRAME_INTERVAL_MS.store(interval_ms, Ordering::Relaxed);
}

fn frame_interval_for_quality(quality: &str) -> u64 {
    match quality {
        "Low latency" => 33,
        "Sharp" => 16,
        _ => 66,
    }
}

fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (part_key, value) = part.split_once('=')?;
        (part_key == key).then(|| percent_decode(value))
    })
}

fn percent_decode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut bytes = value.as_bytes().iter().copied();

    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => output.push(' '),
            b'%' => {
                let Some(high) = bytes.next() else {
                    output.push('%');
                    break;
                };
                let Some(low) = bytes.next() else {
                    output.push('%');
                    output.push(high as char);
                    break;
                };
                match hex_byte(high, low) {
                    Some(decoded) => output.push(decoded as char),
                    None => {
                        output.push('%');
                        output.push(high as char);
                        output.push(low as char);
                    }
                }
            }
            _ => output.push(byte as char),
        }
    }

    output
}

fn hex_byte(high: u8, low: u8) -> Option<u8> {
    Some(hex_value(high)? * 16 + hex_value(low)?)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn cached_frame_response(cache: &SharedFrameCache) -> Response<Cursor<Vec<u8>>> {
    let state = cache.read().expect("frame cache should not be poisoned");

    match &state.frame {
        Some(frame) => binary_response(frame.clone(), "image/png", StatusCode(200)),
        None => text_response(
            state
                .error
                .clone()
                .unwrap_or_else(|| "Waiting for first captured frame".into()),
            StatusCode(503),
        ),
    }
}

fn cached_health_response(cache: &SharedFrameCache) -> Response<Cursor<Vec<u8>>> {
    let state = cache.read().expect("frame cache should not be poisoned");

    if state.frame.is_some() {
        text_response(format!("ok frame={}", state.sequence), StatusCode(200))
    } else {
        text_response(
            state
                .error
                .clone()
                .unwrap_or_else(|| "Waiting for first captured frame".into()),
            StatusCode(503),
        )
    }
}

fn empty_response(status: StatusCode) -> Response<Cursor<Vec<u8>>> {
    with_common_headers(Response::from_data(Vec::new()).with_status_code(status))
}

fn text_response(text: impl Into<String>, status: StatusCode) -> Response<Cursor<Vec<u8>>> {
    with_common_headers(
        Response::from_string(text.into())
            .with_status_code(status)
            .with_header(content_type("text/plain; charset=utf-8")),
    )
}

fn binary_response(
    data: Vec<u8>,
    content_type_value: &'static str,
    status: StatusCode,
) -> Response<Cursor<Vec<u8>>> {
    with_common_headers(
        Response::from_data(data)
            .with_status_code(status)
            .with_header(content_type(content_type_value)),
    )
}

fn with_common_headers(response: Response<Cursor<Vec<u8>>>) -> Response<Cursor<Vec<u8>>> {
    response
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Methods", "GET, OPTIONS"))
        .with_header(header("Access-Control-Allow-Headers", "*"))
        .with_header(header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate",
        ))
}

fn content_type(value: &'static str) -> Header {
    header("Content-Type", value)
}

fn header(name: &'static str, value: &'static str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_reports_capture_available_on_desktop_platforms() {
        let backend = current_capture_backend();

        if cfg!(any(target_os = "windows", target_os = "macos")) {
            assert!(backend.available);
        }
    }

    #[test]
    #[ignore = "captures the real desktop and needs an interactive logged-in session"]
    fn capture_primary_png_smoke() {
        let frame = capture_primary_png().expect("primary monitor should capture");

        assert!(frame.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(frame.len() > 1024);
    }

    #[test]
    fn detects_permission_related_capture_errors() {
        assert!(is_permission_related_capture_error(
            "Could not capture monitor: Screen Recording permission denied"
        ));
        assert!(!is_permission_related_capture_error(
            "Could not encode captured frame: invalid data"
        ));
    }

    #[test]
    fn frame_interval_tracks_quality_modes() {
        assert_eq!(frame_interval_for_quality("Low latency"), 33);
        assert_eq!(frame_interval_for_quality("Balanced"), 66);
        assert_eq!(frame_interval_for_quality("Sharp"), 16);
    }

    #[test]
    fn query_value_decodes_quality_names() {
        assert_eq!(
            query_value("quality=Low%20latency&x=1", "quality").as_deref(),
            Some("Low latency")
        );
        assert_eq!(
            query_value("x=1&quality=Sharp", "quality").as_deref(),
            Some("Sharp")
        );
    }

    #[test]
    fn capture_prefers_panelink_virtual_display() {
        let monitors = [
            MonitorDescriptor {
                index: 0,
                name: "Built-in Retina Display".into(),
                is_primary: true,
            },
            MonitorDescriptor {
                index: 1,
                name: "Dell U2723QE".into(),
                is_primary: false,
            },
            MonitorDescriptor {
                index: 2,
                name: "PaneLink Virtual Display".into(),
                is_primary: false,
            },
        ];

        assert_eq!(preferred_monitor_index(&monitors), Some(2));
    }

    #[test]
    fn capture_falls_back_to_non_primary_display_before_primary() {
        let monitors = [
            MonitorDescriptor {
                index: 0,
                name: "Built-in Retina Display".into(),
                is_primary: true,
            },
            MonitorDescriptor {
                index: 1,
                name: "Extended Display".into(),
                is_primary: false,
            },
        ];

        assert_eq!(preferred_monitor_index(&monitors), Some(1));
    }
}
