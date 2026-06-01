use serde::{Deserialize, Serialize};
use std::{io::Cursor, sync::OnceLock, thread};
use tiny_http::{Header, Response, Server, StatusCode};
use xcap::{image::ImageFormat, Monitor};

pub const FRAME_SERVER_PORT: u16 = 48171;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBackend {
    pub name: String,
    pub available: bool,
    pub requires_permission: bool,
    pub note: String,
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

            thread::Builder::new()
                .name("panelink-frame-server".into())
                .spawn(move || run_frame_server(server))
                .map_err(|error| format!("Frame server could not start: {error}"))?;

            Ok(FRAME_SERVER_PORT)
        })
        .clone()
}

pub fn capture_primary_png() -> Result<Vec<u8>, String> {
    let monitor = Monitor::all()
        .map_err(|error| format!("Could not list monitors: {error}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "No monitor found to capture".to_string())?;
    let image = monitor
        .capture_image()
        .map_err(|error| format!("Could not capture monitor: {error}"))?;
    let mut bytes = Cursor::new(Vec::new());

    image
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|error| format!("Could not encode captured frame: {error}"))?;

    Ok(bytes.into_inner())
}

fn run_frame_server(server: Server) {
    for request in server.incoming_requests() {
        let method = request.method().as_str().to_string();
        let path = request.url().split('?').next().unwrap_or("/");
        let response = if method == "OPTIONS" {
            empty_response(StatusCode(204))
        } else {
            match path {
                "/frame" => match capture_primary_png() {
                    Ok(frame) => binary_response(frame, "image/png", StatusCode(200)),
                    Err(error) => text_response(error, StatusCode(503)),
                },
                "/health" => match capture_primary_png() {
                    Ok(_) => text_response("ok", StatusCode(200)),
                    Err(error) => text_response(error, StatusCode(503)),
                },
                _ => text_response("PaneLink frame server", StatusCode(200)),
            }
        };

        let _ = request.respond(response);
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
}
