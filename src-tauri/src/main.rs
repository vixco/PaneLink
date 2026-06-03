use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use panelink_core::{
    AudioCapabilities, AudioDevice, Capabilities, CaptureState, DisplayCapabilities,
    PermissionState, PermissionStatus, RoutingState, SessionSnapshot, VirtualDisplayState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Cursor,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeSetupState {
    started: bool,
    platform: String,
    message: String,
    actions: Vec<String>,
    requires_restart: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteFrameResponse {
    ok: bool,
    status_code: u16,
    content_type: String,
    data_url: Option<String>,
    message: String,
}

#[derive(Debug)]
struct RawHttpResponse {
    status_code: u16,
    content_type: String,
    transfer_encoding: String,
    body: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteDisplayResponse {
    ok: bool,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostDisplayPrepareRequest {
    width: u32,
    height: u32,
    refresh_hz: u16,
    quality: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostDisplayPrepareResponse {
    ok: bool,
    frame_url: String,
    h264_stream: Option<panelink_video::H264StreamSession>,
    virtual_display: Option<panelink_virtual_display::VirtualDisplaySession>,
    message: String,
}

#[tauri::command]
fn list_peers() -> Vec<panelink_core::Peer> {
    panelink_discovery::scan_lan_peers(std::time::Duration::from_millis(600))
        .unwrap_or_else(|_| panelink_discovery::list_cached_peers())
}

#[tauri::command]
fn scan_peers() -> Vec<panelink_core::Peer> {
    panelink_discovery::scan_lan_peers(std::time::Duration::from_millis(1500))
        .unwrap_or_else(|_| panelink_discovery::list_cached_peers())
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
fn get_stream_state() -> panelink_transport::StreamState {
    panelink_transport::stream_state()
}

#[tauri::command]
fn get_frame_server_url() -> Result<String, String> {
    panelink_capture::start_frame_server().map(|port| format!("http://127.0.0.1:{port}/frame"))
}

#[tauri::command]
fn get_frame_server_lan_url() -> Result<String, String> {
    let port = panelink_capture::start_frame_server()?;
    let host = lan_host_for_peer(None).ok_or_else(|| {
        "Could not determine this device's LAN address for remote display streaming".to_string()
    })?;

    Ok(format!("http://{host}:{port}/frame"))
}

#[tauri::command]
fn get_frame_server_lan_url_for_peer(peer_address: String) -> Result<String, String> {
    let port = panelink_capture::start_frame_server()?;
    let host = lan_host_for_peer(Some(&peer_address)).ok_or_else(|| {
        format!("Could not determine this device's LAN address toward {peer_address}")
    })?;

    Ok(format!("http://{host}:{port}/frame"))
}

#[tauri::command]
fn get_control_server_lan_url() -> Result<String, String> {
    let host = lan_host_for_peer(None).ok_or_else(|| {
        "Could not determine this device's LAN address for remote display control".to_string()
    })?;

    Ok(format!(
        "http://{host}:{}",
        panelink_discovery::DEFAULT_PORT
    ))
}

#[tauri::command]
fn get_control_server_lan_url_for_peer(peer_address: String) -> Result<String, String> {
    let host = lan_host_for_peer(Some(&peer_address)).ok_or_else(|| {
        format!("Could not determine this device's LAN control address toward {peer_address}")
    })?;

    Ok(format!(
        "http://{host}:{}",
        panelink_discovery::DEFAULT_PORT
    ))
}

#[tauri::command]
fn fetch_remote_frame(url: String) -> RemoteFrameResponse {
    match fetch_http_bytes(&url, Duration::from_millis(1200)) {
        Ok(response) => response,
        Err(error) => RemoteFrameResponse {
            ok: false,
            status_code: 0,
            content_type: String::new(),
            data_url: None,
            message: error,
        },
    }
}

#[tauri::command]
fn open_remote_display_window(
    receiver_address: String,
    receiver_peer_id: Option<String>,
    request: DisplayWindowOpenRequest,
) -> RemoteDisplayResponse {
    let mut attempted_addresses = Vec::new();
    match request_receiver_display(&receiver_address, &request) {
        Ok(response) => return response,
        Err(error) => attempted_addresses.push(format!("{receiver_address}: {error}")),
    }

    if let Some(receiver_peer_id) = receiver_peer_id {
        if let Some(refreshed_address) =
            refreshed_receiver_address(&receiver_peer_id, &receiver_address)
        {
            match request_receiver_display(&refreshed_address, &request) {
                Ok(response) => return response,
                Err(error) => attempted_addresses.push(format!("{refreshed_address}: {error}")),
            }
        }
    }

    RemoteDisplayResponse {
        ok: false,
        message: format!(
            "Receiver is niet bereikbaar via LAN. Geprobeerd: {}. Controleer dat beide apparaten op hetzelfde netwerk zitten, VPN uit staat voor LAN, en Windows Firewall PaneLink poort {} toestaat.",
            attempted_addresses.join(" | "),
            panelink_discovery::DEFAULT_PORT
        ),
    }
}

fn request_receiver_display(
    receiver_address: &str,
    request: &DisplayWindowOpenRequest,
) -> Result<RemoteDisplayResponse, String> {
    let host = host_from_authority(receiver_address)
        .ok_or_else(|| "Receiver address is missing a LAN host".to_string())?;
    let peer_id = request
        .peer_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unknown");
    let peer_address = request
        .peer_address
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Remote display request is missing video endpoint".to_string())?;
    let url = format!(
        "http://{}:{}/open-display?peerId={}&peerAddress={}&controlAddress={}&videoSessionId={}&videoTransport={}&videoCodec={}&screens={}&quality={}",
        host,
        panelink_discovery::DEFAULT_PORT,
        percent_encode(peer_id),
        percent_encode(peer_address),
        percent_encode(request.control_address.as_deref().unwrap_or_default()),
        percent_encode(request.video_session_id.as_deref().unwrap_or_default()),
        percent_encode(
            request
                .video_transport
                .as_deref()
                .unwrap_or("H.264 LAN stream"),
        ),
        percent_encode(request.video_codec.as_deref().unwrap_or("H.264 OpenH264")),
        request.screen_count.unwrap_or(1).clamp(1, 3),
        percent_encode(request.quality.as_deref().unwrap_or("Low latency"))
    );

    match fetch_http_text(&url, Duration::from_millis(1600)) {
        Ok(response) if response.status_code == 200 => Ok(RemoteDisplayResponse {
            ok: true,
            message: response.body,
        }),
        Ok(response) => Ok(RemoteDisplayResponse {
            ok: false,
            message: if response.body.is_empty() {
                format!("Receiver returned HTTP {}", response.status_code)
            } else {
                response.body
            },
        }),
        Err(error) => Err(error),
    }
}

fn refreshed_receiver_address(receiver_peer_id: &str, original_address: &str) -> Option<String> {
    panelink_discovery::scan_lan_peers(Duration::from_millis(900))
        .ok()?
        .into_iter()
        .find(|peer| peer.id == receiver_peer_id)
        .map(|peer| peer.address)
        .filter(|address| address != original_address)
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
fn start_stream(
    request: Option<panelink_transport::StartStreamRequest>,
) -> Result<panelink_transport::StreamState, panelink_transport::SessionError> {
    panelink_transport::start_stream(request)
}

#[tauri::command]
fn stop_stream() -> panelink_transport::StreamState {
    panelink_transport::stop_stream()
}

#[tauri::command]
fn open_display_window(app: AppHandle, request: DisplayWindowOpenRequest) -> Result<(), String> {
    open_display_window_for_request(app, request)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisplayWindowOpenRequest {
    screen_count: Option<u8>,
    peer_id: Option<String>,
    peer_address: Option<String>,
    control_address: Option<String>,
    video_session_id: Option<String>,
    video_transport: Option<String>,
    video_codec: Option<String>,
    quality: Option<String>,
}

fn open_display_window_for_request(
    app: AppHandle,
    request: DisplayWindowOpenRequest,
) -> Result<(), String> {
    let screen_count = request.screen_count.unwrap_or(1).clamp(1, 3);
    let initial_width = if screen_count > 1 { 1440.0 } else { 1280.0 };
    let display_url = format!(
        "index.html?window=display&peerId={}&peerAddress={}&controlAddress={}&videoSessionId={}&videoTransport={}&videoCodec={}&screens={screen_count}&quality={}",
        percent_encode(&request.peer_id.unwrap_or_else(|| "unknown".into())),
        percent_encode(&request.peer_address.unwrap_or_default()),
        percent_encode(&request.control_address.unwrap_or_default()),
        percent_encode(&request.video_session_id.unwrap_or_default()),
        percent_encode(
            &request
                .video_transport
                .unwrap_or_else(|| "H.264 LAN stream".into()),
        ),
        percent_encode(
            &request
                .video_codec
                .unwrap_or_else(|| "H.264 OpenH264".into()),
        ),
        percent_encode(&request.quality.unwrap_or_else(|| "Low latency".into()))
    );

    if let Some(window) = app.get_webview_window("display") {
        let _ = window.close();
    }

    WebviewWindowBuilder::new(&app, "display", WebviewUrl::App(display_url.into()))
        .title("PaneLink Display")
        .inner_size(initial_width, 720.0)
        .min_inner_size(720.0, 420.0)
        .closable(true)
        .decorations(false)
        .fullscreen(true)
        .resizable(true)
        .visible(true)
        .build()
        .map_err(|error| error.to_string())?;

    Ok(())
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

fn host_from_authority(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value.starts_with('[') {
        let end = value.find(']')?;
        return Some(value[1..end].to_string());
    }

    Some(
        value
            .rsplit_once(':')
            .filter(|(_, port)| port.chars().all(|char| char.is_ascii_digit()))
            .map(|(host, _)| host)
            .unwrap_or(value)
            .to_string(),
    )
}

fn lan_host_for_peer(peer_address: Option<&str>) -> Option<String> {
    let peer_routed_host = peer_address.and_then(peer_routed_lan_host);
    let advertised_address = panelink_discovery::advertise_payload().address;

    stream_host_from_candidates(peer_routed_host, &advertised_address)
}

fn stream_host_from_candidates(
    peer_routed_host: Option<String>,
    advertised_address: &str,
) -> Option<String> {
    peer_routed_host
        .filter(|host| is_usable_lan_host(host))
        .or_else(|| host_from_authority(advertised_address).filter(|host| is_usable_lan_host(host)))
}

fn peer_routed_lan_host(peer_address: &str) -> Option<String> {
    let peer_host = host_from_authority(peer_address)?;
    let peer_ip = peer_host.parse::<IpAddr>().ok()?;
    if !is_usable_lan_host(&peer_ip.to_string()) {
        return None;
    }

    let bind_address = if peer_ip.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind_address).ok()?;
    socket
        .connect(SocketAddr::new(peer_ip, panelink_discovery::DEFAULT_PORT))
        .ok()?;

    socket
        .local_addr()
        .ok()
        .map(|address| address.ip().to_string())
        .filter(|host| is_usable_lan_host(host))
}

fn is_usable_lan_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }

    host.parse::<IpAddr>()
        .map(|ip| !ip.is_loopback() && !ip.is_unspecified())
        .unwrap_or(true)
}

#[derive(Debug)]
struct HttpTarget {
    authority: String,
    host: String,
    port: u16,
    path: String,
}

fn fetch_http_bytes(url: &str, timeout: Duration) -> Result<RemoteFrameResponse, String> {
    let response = fetch_raw_http_bytes(url, timeout, 64 * 1024 * 1024)?;
    remote_frame_response_from_raw(url, response)
}

fn fetch_raw_http_bytes(
    url: &str,
    timeout: Duration,
    max_bytes: u64,
) -> Result<RawHttpResponse, String> {
    let target = parse_http_url(url)?;
    let mut stream = connect_to_target(&target, timeout)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("Could not set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("Could not set write timeout: {error}"))?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: image/png,*/*\r\nCache-Control: no-cache\r\n\r\n",
        target.path, target.authority
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Could not request remote frame: {error}"))?;

    let bytes = read_http_response(&mut stream, max_bytes as usize)
        .map_err(|error| format!("Could not read remote frame: {error}"))?;

    parse_raw_http_response(&bytes)
}

#[derive(Debug)]
struct HttpTextResponse {
    status_code: u16,
    body: String,
}

fn fetch_http_text(url: &str, timeout: Duration) -> Result<HttpTextResponse, String> {
    let target = parse_http_url(url)?;
    let mut stream = connect_to_target(&target, timeout)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("Could not set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("Could not set write timeout: {error}"))?;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: text/plain\r\nCache-Control: no-cache\r\n\r\n",
        target.path, target.authority
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Could not request receiver display: {error}"))?;

    let bytes = read_http_response(&mut stream, 128 * 1024)
        .map_err(|error| format!("Could not read receiver display response: {error}"))?;

    parse_http_text_response(&bytes)
}

fn read_http_response(stream: &mut TcpStream, max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                bytes.extend_from_slice(&buffer[..read]);
                if bytes.len() > max_bytes {
                    return Err(format!("HTTP response exceeded {max_bytes} bytes"));
                }
                if http_response_complete(&bytes) {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                return Err(error.to_string());
            }
            Err(error) => return Err(error.to_string()),
        }
    }

    Ok(bytes)
}

fn http_response_complete(bytes: &[u8]) -> bool {
    let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let headers = header_text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    let body = &bytes[header_end + 4..];

    if header_value(&headers, "transfer-encoding")
        .unwrap_or_default()
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case("chunked"))
    {
        return chunked_body_complete(body);
    }

    header_value(&headers, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| body.len() >= length)
}

fn chunked_body_complete(bytes: &[u8]) -> bool {
    let mut index = 0;

    loop {
        let Some(line_end) = bytes[index..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|position| index + position)
        else {
            return false;
        };
        let size_text = String::from_utf8_lossy(&bytes[index..line_end]);
        let size_part = size_text.split(';').next().unwrap_or_default().trim();
        let Ok(size) = usize::from_str_radix(size_part, 16) else {
            return false;
        };
        index = line_end + 2;

        if size == 0 {
            return bytes.len() >= index + 2;
        }

        let Some(chunk_end) = index.checked_add(size) else {
            return false;
        };
        if bytes.len() < chunk_end + 2 {
            return false;
        }
        if &bytes[chunk_end..chunk_end + 2] != b"\r\n" {
            return false;
        }
        index = chunk_end + 2;
    }
}

fn parse_http_url(url: &str) -> Result<HttpTarget, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("PaneLink only supports http frame URLs for now: {url}"))?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".into()),
    };

    if authority.is_empty() {
        return Err("Frame URL is missing a host".into());
    }

    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| format!("Invalid IPv6 frame URL host: {authority}"))?;
        let host = authority[1..end].to_string();
        let port = authority[end + 1..]
            .strip_prefix(':')
            .map(|value| value.parse::<u16>())
            .transpose()
            .map_err(|error| format!("Invalid frame URL port: {error}"))?
            .unwrap_or(80);
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) if port.chars().all(|char| char.is_ascii_digit()) => (
                host.to_string(),
                port.parse::<u16>()
                    .map_err(|error| format!("Invalid frame URL port: {error}"))?,
            ),
            _ => (authority.to_string(), 80),
        }
    };

    if host.is_empty() {
        return Err("Frame URL is missing a host".into());
    }

    Ok(HttpTarget {
        authority: authority.into(),
        host,
        port,
        path,
    })
}

fn connect_to_target(target: &HttpTarget, timeout: Duration) -> Result<TcpStream, String> {
    let addresses: Vec<SocketAddr> = (target.host.as_str(), target.port)
        .to_socket_addrs()
        .map_err(|error| format!("Could not resolve {}:{}: {error}", target.host, target.port))?
        .collect();

    if addresses.is_empty() {
        return Err(format!(
            "No address found for {}:{}",
            target.host, target.port
        ));
    }

    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }

    Err(format!(
        "Could not connect to {}:{} ({})",
        target.host,
        target.port,
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "unknown error".into())
    ))
}

fn parse_raw_http_response(bytes: &[u8]) -> Result<RawHttpResponse, String> {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Remote frame response did not include HTTP headers".to_string())?;
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let body = &bytes[header_end + 4..];
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    let content_type = header_value(&headers, "content-type").unwrap_or_default();
    let transfer_encoding = header_value(&headers, "transfer-encoding").unwrap_or_default();
    let body = if transfer_encoding
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case("chunked"))
    {
        decode_chunked_body(body)?
    } else {
        body.to_vec()
    };

    Ok(RawHttpResponse {
        status_code,
        content_type,
        transfer_encoding,
        body,
    })
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

fn decode_chunked_body(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoded = Vec::new();
    let mut index = 0;

    loop {
        let line_end = bytes[index..]
            .windows(2)
            .position(|window| window == b"\r\n")
            .map(|position| index + position)
            .ok_or_else(|| "Chunked frame response ended before a chunk header".to_string())?;
        let size_text = String::from_utf8_lossy(&bytes[index..line_end]);
        let size_part = size_text.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size_part, 16)
            .map_err(|error| format!("Invalid chunk size in frame response: {error}"))?;
        index = line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = index
            .checked_add(size)
            .ok_or_else(|| "Chunked frame response size overflowed".to_string())?;
        if bytes.len() < chunk_end + 2 {
            return Err("Chunked frame response ended before chunk data completed".into());
        }
        if &bytes[chunk_end..chunk_end + 2] != b"\r\n" {
            return Err("Chunked frame response chunk was not CRLF terminated".into());
        }

        decoded.extend_from_slice(&bytes[index..chunk_end]);
        index = chunk_end + 2;
    }

    Ok(decoded)
}

fn remote_frame_response_from_raw(
    url: &str,
    response: RawHttpResponse,
) -> Result<RemoteFrameResponse, String> {
    if response.status_code == 200 && response.body.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(RemoteFrameResponse {
            ok: true,
            status_code: response.status_code,
            content_type: "image/png".into(),
            data_url: Some(format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode(response.body)
            )),
            message: format!("Frame loaded from {url}"),
        });
    }

    let message = if response.body.is_empty() {
        format!("Remote frame returned HTTP {}", response.status_code)
    } else if response.body.len() <= 4096
        && response
            .body
            .iter()
            .all(|byte| byte.is_ascii_graphic() || byte.is_ascii_whitespace())
    {
        String::from_utf8_lossy(&response.body).trim().to_string()
    } else {
        format!(
            "Remote frame was not a valid PNG (HTTP {}, content-type {}, transfer {}, {} bytes)",
            response.status_code,
            if response.content_type.is_empty() {
                "unknown"
            } else {
                &response.content_type
            },
            if response.transfer_encoding.is_empty() {
                "none"
            } else {
                &response.transfer_encoding
            },
            response.body.len()
        )
    };

    Ok(RemoteFrameResponse {
        ok: false,
        status_code: response.status_code,
        content_type: response.content_type,
        data_url: None,
        message,
    })
}

fn parse_http_text_response(bytes: &[u8]) -> Result<HttpTextResponse, String> {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Receiver response did not include HTTP headers".to_string())?;
    let header_text = String::from_utf8_lossy(&bytes[..header_end]);
    let body = &bytes[header_end + 4..];
    let mut lines = header_text.lines();
    let status_line = lines.next().unwrap_or_default();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);

    Ok(HttpTextResponse {
        status_code,
        body: String::from_utf8_lossy(body).trim().to_string(),
    })
}

fn start_remote_control_server(app: AppHandle) -> Result<(), String> {
    let server = Server::http(("0.0.0.0", panelink_discovery::DEFAULT_PORT)).map_err(|error| {
        format!(
            "Remote display control server could not bind port {}: {error}",
            panelink_discovery::DEFAULT_PORT
        )
    })?;

    thread::Builder::new()
        .name("panelink-remote-control".into())
        .spawn(move || run_remote_control_server(app, server))
        .map(|_| ())
        .map_err(|error| format!("Remote display control server could not start: {error}"))
}

fn run_remote_control_server(app: AppHandle, server: Server) {
    for mut request in server.incoming_requests() {
        let method = request.method().as_str().to_string();
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/");
        if matches!(method.as_str(), "GET" | "OPTIONS") && path == panelink_video::H264_STREAM_PATH
        {
            panelink_video::respond_h264_stream_request(request);
            continue;
        }

        let response = if method == "OPTIONS" {
            text_control_response("", StatusCode(204))
        } else if matches!(method.as_str(), "GET" | "POST") && path == "/open-display" {
            match display_request_from_url(&url) {
                Ok(display_request) => {
                    match open_display_window_for_request(app.clone(), display_request) {
                        Ok(()) => text_control_response(
                            "Display window opened on receiver",
                            StatusCode(200),
                        ),
                        Err(error) => text_control_response(error, StatusCode(500)),
                    }
                }
                Err(error) => text_control_response(error, StatusCode(400)),
            }
        } else if method == "GET" && path == "/frame-proxy" {
            match frame_proxy_url_from_request(&url) {
                Ok(frame_url) => proxied_frame_response(&frame_url),
                Err(error) => text_control_response(error, StatusCode(400)),
            }
        } else if matches!(method.as_str(), "GET" | "POST") && path == "/prepare-host-display" {
            match host_display_prepare_request_from_url(&url) {
                Ok(prepare_request) => host_display_prepare_response(prepare_request),
                Err(error) => text_control_response(error, StatusCode(400)),
            }
        } else if method == "POST" && path == "/input-events" {
            remote_input_response(&mut request)
        } else if path == "/health" {
            text_control_response("ok", StatusCode(200))
        } else {
            text_control_response("PaneLink remote display control", StatusCode(200))
        };

        let _ = request.respond(response);
    }
}

fn host_display_prepare_response(request: HostDisplayPrepareRequest) -> Response<Cursor<Vec<u8>>> {
    match prepare_host_display(request) {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json) => text_control_response(json, StatusCode(200))
                .with_header(header("Content-Type", "application/json")),
            Err(error) => text_control_response(
                format!("Could not encode host display response: {error}"),
                StatusCode(500),
            ),
        },
        Err(error) => text_control_response(error, StatusCode(500)),
    }
}

fn prepare_host_display(
    request: HostDisplayPrepareRequest,
) -> Result<HostDisplayPrepareResponse, String> {
    let virtual_display = ensure_host_virtual_display(&request)?;
    let port = panelink_capture::start_frame_server()?;
    let h264_stream =
        panelink_video::configure_h264_control_stream(panelink_video::H264StreamRequest {
            width: request.width,
            height: request.height,
            target_fps: request.refresh_hz.min(60),
            target_bitrate_mbps: h264_bitrate_for_quality(&request.quality),
            quality: request.quality.clone(),
        })?;

    Ok(HostDisplayPrepareResponse {
        ok: true,
        frame_url: format!("http://127.0.0.1:{port}/frame"),
        h264_stream: Some(h264_stream),
        virtual_display: Some(virtual_display),
        message: "PaneLink host virtual display is ready and H.264 stream server is running."
            .into(),
    })
}

fn h264_bitrate_for_quality(quality: &str) -> u16 {
    match quality {
        "Sharp" => 52,
        "Balanced" => 36,
        _ => 28,
    }
}

fn ensure_host_virtual_display(
    request: &HostDisplayPrepareRequest,
) -> Result<panelink_virtual_display::VirtualDisplaySession, String> {
    let mut slot = host_virtual_display_slot()
        .lock()
        .map_err(|_| "Host virtual display state is unavailable".to_string())?;

    if let Some(session) = slot.as_ref().filter(|session| session.active) {
        set_capture_target_for_virtual_display(session);
        return Ok(session.clone());
    }

    let session = panelink_virtual_display::create_virtual_display(
        panelink_virtual_display::VirtualDisplayRequest {
            name: "PaneLink Virtual Display".into(),
            width: request.width,
            height: request.height,
            refresh_hz: request.refresh_hz,
        },
    )?;
    set_capture_target_for_virtual_display(&session);
    *slot = Some(session.clone());

    Ok(session)
}

fn set_capture_target_for_virtual_display(
    session: &panelink_virtual_display::VirtualDisplaySession,
) {
    panelink_capture::set_capture_target(panelink_capture::CaptureTarget {
        display_id: session.platform_display_id,
        display_name: Some(session.display_name.clone()),
    });
    panelink_input::set_pointer_target_display(session.platform_display_id);
}

fn host_virtual_display_slot(
) -> &'static Mutex<Option<panelink_virtual_display::VirtualDisplaySession>> {
    static HOST_DISPLAY: OnceLock<Mutex<Option<panelink_virtual_display::VirtualDisplaySession>>> =
        OnceLock::new();
    HOST_DISPLAY.get_or_init(|| Mutex::new(None))
}

fn remote_input_response(request: &mut tiny_http::Request) -> Response<Cursor<Vec<u8>>> {
    let mut body = String::new();
    if let Err(error) = request.as_reader().read_to_string(&mut body) {
        return text_control_response(
            format!("Could not read input batch: {error}"),
            StatusCode(400),
        );
    }

    match serde_json::from_str::<panelink_input::InputEventBatch>(&body) {
        Ok(batch) => match serde_json::to_string(&panelink_input::accept_batch(batch)) {
            Ok(receipt) => text_control_response(receipt, StatusCode(200)),
            Err(error) => text_control_response(
                format!("Could not encode input receipt: {error}"),
                StatusCode(500),
            ),
        },
        Err(error) => {
            text_control_response(format!("Invalid input batch: {error}"), StatusCode(400))
        }
    }
}

fn frame_proxy_url_from_request(url: &str) -> Result<String, String> {
    let query = url
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| "Frame proxy request is missing query parameters".to_string())?;
    let values = parse_query(query);

    values
        .get("url")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| "Frame proxy request is missing url".to_string())
}

fn host_display_prepare_request_from_url(url: &str) -> Result<HostDisplayPrepareRequest, String> {
    let values = url
        .split_once('?')
        .map(|(_, query)| parse_query(query))
        .unwrap_or_default();

    Ok(HostDisplayPrepareRequest {
        width: values
            .get("width")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1920),
        height: values
            .get("height")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1080),
        refresh_hz: values
            .get("refreshHz")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(60),
        quality: values
            .get("quality")
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "Sharp".into()),
    })
}

fn proxied_frame_response(frame_url: &str) -> Response<Cursor<Vec<u8>>> {
    match fetch_raw_http_bytes(frame_url, Duration::from_millis(1400), 64 * 1024 * 1024) {
        Ok(response)
            if response.status_code == 200 && response.body.starts_with(b"\x89PNG\r\n\x1a\n") =>
        {
            binary_control_response(response.body, "image/png", StatusCode(200))
        }
        Ok(response) => text_control_response(
            format!(
                "Remote frame was not a valid PNG (HTTP {}, content-type {}, transfer {}, {} bytes)",
                response.status_code,
                if response.content_type.is_empty() {
                    "unknown"
                } else {
                    &response.content_type
                },
                if response.transfer_encoding.is_empty() {
                    "none"
                } else {
                    &response.transfer_encoding
                },
                response.body.len()
            ),
            StatusCode(502),
        ),
        Err(error) => text_control_response(error, StatusCode(502)),
    }
}

fn display_request_from_url(url: &str) -> Result<DisplayWindowOpenRequest, String> {
    let query = url
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| "Remote display request is missing query parameters".to_string())?;
    let values = parse_query(query);
    let peer_address = values
        .get("peerAddress")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| "Remote display request is missing peerAddress".to_string())?;
    let screen_count = values
        .get("screens")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(1)
        .clamp(1, 3);

    Ok(DisplayWindowOpenRequest {
        screen_count: Some(screen_count),
        peer_id: values.get("peerId").cloned(),
        peer_address: Some(peer_address),
        control_address: values.get("controlAddress").cloned(),
        video_session_id: values.get("videoSessionId").cloned(),
        video_transport: values.get("videoTransport").cloned(),
        video_codec: values.get("videoCodec").cloned(),
        quality: values.get("quality").cloned(),
    })
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();

    for part in query.split('&').filter(|part| !part.is_empty()) {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        values.insert(percent_decode(key), percent_decode(value));
    }

    values
}

fn percent_decode(value: &str) -> String {
    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                    if let Ok(byte) = u8::from_str_radix(hex, 16) {
                        output.push(byte);
                        index += 3;
                        continue;
                    }
                }
                output.push(bytes[index]);
                index += 1;
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn text_control_response(text: impl Into<String>, status: StatusCode) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(text.into())
        .with_status_code(status)
        .with_header(header("Content-Type", "text/plain; charset=utf-8"))
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
        .with_header(header("Access-Control-Allow-Headers", "*"))
        .with_header(header("Access-Control-Allow-Private-Network", "true"))
}

fn binary_control_response(
    data: Vec<u8>,
    content_type: &'static str,
    status: StatusCode,
) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(data)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_from_authority_accepts_peer_endpoints_and_bare_hosts() {
        assert_eq!(
            host_from_authority("192.168.1.42:48170").as_deref(),
            Some("192.168.1.42")
        );
        assert_eq!(
            host_from_authority("[fe80::1]:48170").as_deref(),
            Some("fe80::1")
        );
        assert_eq!(
            host_from_authority("panelink.local").as_deref(),
            Some("panelink.local")
        );
    }

    #[test]
    fn stream_host_prefers_peer_routed_lan_ip_over_advertised_vpn_ip() {
        let host = stream_host_from_candidates(Some("192.168.1.6".into()), "10.14.0.2")
            .expect("peer-routed LAN host should be selected");

        assert_eq!(host, "192.168.1.6");
    }

    #[test]
    fn display_request_from_url_decodes_remote_display_payload() {
        let request = display_request_from_url(
            "/open-display?peerId=mac%201&peerAddress=http%3A%2F%2F192.168.1.24%3A48170%2Fh264&controlAddress=http%3A%2F%2F192.168.1.24%3A48170&videoSessionId=video-1&videoTransport=H.264+LAN+stream&videoCodec=H.264+OpenH264&screens=2&quality=Low+latency",
        )
        .expect("remote display request should parse");

        assert_eq!(request.peer_id.as_deref(), Some("mac 1"));
        assert_eq!(
            request.peer_address.as_deref(),
            Some("http://192.168.1.24:48170/h264")
        );
        assert_eq!(
            request.control_address.as_deref(),
            Some("http://192.168.1.24:48170")
        );
        assert_eq!(request.video_session_id.as_deref(), Some("video-1"));
        assert_eq!(request.video_transport.as_deref(), Some("H.264 LAN stream"));
        assert_eq!(request.video_codec.as_deref(), Some("H.264 OpenH264"));
        assert_eq!(request.screen_count, Some(2));
        assert_eq!(request.quality.as_deref(), Some("Low latency"));
    }

    #[test]
    fn host_display_prepare_request_decodes_monitor_mode() {
        let request = host_display_prepare_request_from_url(
            "/prepare-host-display?width=2560&height=1440&refreshHz=60&quality=Sharp",
        )
        .expect("host display request should parse");

        assert_eq!(request.width, 2560);
        assert_eq!(request.height, 1440);
        assert_eq!(request.refresh_hz, 60);
        assert_eq!(request.quality, "Sharp");
    }

    #[test]
    fn raw_http_response_decodes_chunked_png_frames() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n\x89PNG\r\n\x1A\n\r\n1\r\nX\r\n0\r\n\r\n";
        let parsed = parse_raw_http_response(response).expect("chunked frame should parse");

        assert_eq!(parsed.status_code, 200);
        assert_eq!(parsed.content_type, "image/png");
        assert_eq!(parsed.transfer_encoding, "chunked");
        assert!(parsed.body.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(parsed.body, b"\x89PNG\r\n\x1a\nX");
    }

    #[test]
    fn chunked_decoder_rejects_truncated_chunks() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n\x89PNG\r\n";

        assert!(parse_raw_http_response(response).is_err());
    }

    #[test]
    fn http_response_complete_accepts_content_length_body() {
        assert!(http_response_complete(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello"
        ));
        assert!(!http_response_complete(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhe"
        ));
    }

    #[test]
    fn http_response_complete_accepts_finished_chunked_body() {
        assert!(http_response_complete(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n"
        ));
        assert!(!http_response_complete(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n"
        ));
    }
}

#[tauri::command]
fn close_display_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("display") {
        window.close().map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn add_remote_screen(
    _peer_id: Option<String>,
) -> Result<SessionSnapshot, panelink_transport::SessionError> {
    panelink_transport::add_remote_screen()
}

#[tauri::command]
fn remove_remote_screen(
    screen_id: String,
) -> Result<SessionSnapshot, panelink_transport::SessionError> {
    panelink_transport::remove_remote_screen(screen_id)
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
fn get_virtual_display_backend() -> panelink_virtual_display::VirtualDisplayBackendReport {
    panelink_virtual_display::backend_report()
}

#[tauri::command]
fn create_virtual_display(
    request: panelink_virtual_display::VirtualDisplayRequest,
) -> Result<panelink_virtual_display::VirtualDisplaySession, String> {
    let session = panelink_virtual_display::create_virtual_display(request)?;
    set_capture_target_for_virtual_display(&session);
    Ok(session)
}

#[tauri::command]
fn destroy_virtual_display(
    id: String,
) -> Result<panelink_virtual_display::VirtualDisplaySession, String> {
    let session = panelink_virtual_display::destroy_virtual_display(id)?;
    panelink_capture::clear_capture_target();
    panelink_input::clear_pointer_target_display();
    Ok(session)
}

#[tauri::command]
fn get_video_backend() -> panelink_video::VideoBackendReport {
    panelink_video::backend_report()
}

#[tauri::command]
fn start_video_session(
    request: panelink_video::VideoSessionRequest,
) -> Result<panelink_video::VideoSession, String> {
    panelink_video::start_video_session(request)
}

#[tauri::command]
fn get_current_video_session() -> Option<panelink_video::VideoSession> {
    panelink_video::current_video_session()
}

#[tauri::command]
fn stop_video_session() -> Option<panelink_video::VideoSession> {
    panelink_video::stop_video_session()
}

#[tauri::command]
fn get_capabilities() -> Capabilities {
    let capture = panelink_capture::current_capture_backend();
    let virtual_display = panelink_virtual_display::backend_report();
    let video = panelink_video::backend_report();

    Capabilities {
        app_version: env!("CARGO_PKG_VERSION").into(),
        peer_id: panelink_core::local_peer_id(),
        platform: std::env::consts::OS.into(),
        video_encoders: vec![
            "H.264 OpenH264".into(),
            "H.264 WebCodecs decode".into(),
            "H.264 hardware decode".into(),
        ],
        transport: vec![
            panelink_transport::default_transport_plan().primary,
            video.transport,
            "Debug PNG frame server is disabled for product display".into(),
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
            } else if capture.requires_permission {
                CaptureState::PermissionRequired
            } else {
                CaptureState::Stub
            },
            virtual_display: if virtual_display.available {
                VirtualDisplayState::Available
            } else {
                VirtualDisplayState::DriverRequired
            },
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

#[tauri::command]
fn run_native_setup() -> NativeSetupState {
    #[cfg(target_os = "macos")]
    {
        let screen_capture = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .status()
            .is_ok();
        let accessibility = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .status()
            .is_ok();

        NativeSetupState {
            started: screen_capture || accessibility,
            platform: "macos".into(),
            message: "macOS privacy setup opened. Allow Screen Recording and Accessibility for PaneLink, then restart PaneLink.".into(),
            actions: vec![
                "Open Screen Recording permission".into(),
                "Open Accessibility permission".into(),
            ],
            requires_restart: true,
        }
    }

    #[cfg(target_os = "windows")]
    {
        NativeSetupState {
            started: true,
            platform: "windows".into(),
            message: format!(
                "Windows capture is active. If the receiver stays black, allow PaneLink through Windows Firewall on port {}.",
                panelink_capture::FRAME_SERVER_PORT
            ),
            actions: vec![
                "Started native frame capture cache".into(),
                format!("Serving frames on port {}", panelink_capture::FRAME_SERVER_PORT),
            ],
            requires_restart: false,
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        NativeSetupState {
            started: false,
            platform: std::env::consts::OS.into(),
            message: "This platform is not supported by the PaneLink native setup assistant."
                .into(),
            actions: Vec::new(),
            requires_restart: false,
        }
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            if let Err(error) = start_remote_control_server(app.handle().clone()) {
                eprintln!("PaneLink remote control startup failed: {error}");
            }

            Ok(())
        })
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_peers,
            scan_peers,
            advertise_peer,
            issue_pairing_token,
            get_frame_server_url,
            get_frame_server_lan_url,
            get_frame_server_lan_url_for_peer,
            get_control_server_lan_url,
            get_control_server_lan_url_for_peer,
            fetch_remote_frame,
            open_remote_display_window,
            get_session_snapshot,
            get_transport_state,
            get_stream_state,
            connect_peer,
            disconnect_peer,
            start_stream,
            stop_stream,
            open_display_window,
            close_display_window,
            add_remote_screen,
            remove_remote_screen,
            ping_peer,
            list_audio_devices,
            get_audio_route_catalog,
            get_input_backend_report,
            submit_input_batch,
            get_virtual_display_backend,
            create_virtual_display,
            destroy_virtual_display,
            get_video_backend,
            start_video_session,
            get_current_video_session,
            stop_video_session,
            get_capabilities,
            get_permissions,
            run_native_setup
        ])
        .run(tauri::generate_context!())
        .expect("failed to run PaneLink");
}
