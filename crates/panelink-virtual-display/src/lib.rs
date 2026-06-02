use serde::{Deserialize, Serialize};
use uuid::Uuid;

const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;
const DEFAULT_REFRESH_HZ: u16 = 60;
const MIN_WIDTH: u32 = 640;
const MIN_HEIGHT: u32 = 480;
const MAX_WIDTH: u32 = 8192;
const MAX_HEIGHT: u32 = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VirtualDisplayState {
    Available,
    DriverRequired,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDisplayBackendReport {
    pub backend: String,
    pub state: VirtualDisplayState,
    pub available: bool,
    pub requires_external_tool: bool,
    pub message: String,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDisplayRequest {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDisplaySession {
    pub id: String,
    pub active: bool,
    pub backend: String,
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u16,
    pub message: String,
}

pub fn backend_report() -> VirtualDisplayBackendReport {
    platform_backend_report()
}

pub fn create_virtual_display(
    request: VirtualDisplayRequest,
) -> Result<VirtualDisplaySession, String> {
    let request = normalize_request(request);
    let report = backend_report();

    if !report.available {
        return Err(report.message);
    }

    start_available_backend(&report)?;

    Ok(VirtualDisplaySession {
        id: Uuid::new_v4().to_string(),
        active: true,
        backend: report.backend,
        display_name: request.name,
        width: request.width,
        height: request.height,
        refresh_hz: request.refresh_hz,
        message: "Virtual display backend started. macOS may still need a moment to publish the new display.".into(),
    })
}

pub fn destroy_virtual_display(id: String) -> Result<VirtualDisplaySession, String> {
    if id.trim().is_empty() {
        return Err("Virtual display id is missing".into());
    }

    Ok(VirtualDisplaySession {
        id,
        active: false,
        backend: backend_report().backend,
        display_name: "PaneLink Virtual Display".into(),
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        refresh_hz: DEFAULT_REFRESH_HZ,
        message: "Virtual display release requested. If an external helper is active, close its PaneLink display from that helper.".into(),
    })
}

fn normalize_request(request: VirtualDisplayRequest) -> VirtualDisplayRequest {
    let name = request.name.trim();

    VirtualDisplayRequest {
        name: if name.is_empty() {
            "PaneLink Virtual Display".into()
        } else {
            name.into()
        },
        width: clamp_or_default(request.width, MIN_WIDTH, MAX_WIDTH, DEFAULT_WIDTH),
        height: clamp_or_default(request.height, MIN_HEIGHT, MAX_HEIGHT, DEFAULT_HEIGHT),
        refresh_hz: normalize_refresh_rate(request.refresh_hz),
    }
}

fn clamp_or_default(value: u32, min: u32, max: u32, default: u32) -> u32 {
    if value == 0 {
        default
    } else {
        value.clamp(min, max)
    }
}

fn normalize_refresh_rate(value: u16) -> u16 {
    match value {
        60 | 90 | 120 | 144 => value,
        _ => DEFAULT_REFRESH_HZ,
    }
}

fn platform_backend_report() -> VirtualDisplayBackendReport {
    #[cfg(target_os = "macos")]
    {
        if helper_exists("/Applications/BetterDisplay.app") {
            return VirtualDisplayBackendReport {
                backend: "BetterDisplay external helper".into(),
                state: VirtualDisplayState::Available,
                available: true,
                requires_external_tool: true,
                message:
                    "BetterDisplay is installed and can provide the real macOS virtual monitor."
                        .into(),
                actions: vec![
                    "Open BetterDisplay".into(),
                    "Create PaneLink virtual monitor".into(),
                ],
            };
        }

        if helper_exists("/Applications/SimpleDisplay.app") {
            return VirtualDisplayBackendReport {
                backend: "SimpleDisplay external helper".into(),
                state: VirtualDisplayState::Available,
                available: true,
                requires_external_tool: true,
                message:
                    "SimpleDisplay is installed and can provide the real macOS virtual monitor."
                        .into(),
                actions: vec![
                    "Open SimpleDisplay".into(),
                    "Create PaneLink virtual monitor".into(),
                ],
            };
        }

        return VirtualDisplayBackendReport {
            backend: "macOS CGVirtualDisplay backend".into(),
            state: VirtualDisplayState::DriverRequired,
            available: false,
            requires_external_tool: true,
            message: "A macOS virtual-display helper is required before PaneLink can add a real extended display. Install BetterDisplay or SimpleDisplay, then try again.".into(),
            actions: vec!["Install BetterDisplay or SimpleDisplay".into()],
        };
    }

    #[allow(unreachable_code)]
    VirtualDisplayBackendReport {
        backend: "No virtual-display backend".into(),
        state: VirtualDisplayState::DriverRequired,
        available: false,
        requires_external_tool: false,
        message: "This device cannot create the Mac-side virtual display. Run this action from the Mac that needs the extra monitor.".into(),
        actions: Vec::new(),
    }
}

#[cfg(target_os = "macos")]
fn helper_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

fn start_available_backend(_report: &VirtualDisplayBackendReport) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app_name = if _report.backend.starts_with("BetterDisplay") {
            "BetterDisplay"
        } else if _report.backend.starts_with("SimpleDisplay") {
            "SimpleDisplay"
        } else {
            return Ok(());
        };

        return std::process::Command::new("open")
            .arg("-a")
            .arg(app_name)
            .status()
            .map(|_| ())
            .map_err(|error| format!("Could not start {app_name}: {error}"));
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platforms_report_driver_required_without_actions() {
        let report = backend_report();

        if !cfg!(target_os = "macos") {
            assert_eq!(report.state, VirtualDisplayState::DriverRequired);
            assert!(!report.available);
            assert!(report.actions.is_empty());
        }
    }

    #[test]
    fn request_defaults_to_safe_monitor_mode() {
        let request = VirtualDisplayRequest {
            name: String::new(),
            width: 0,
            height: 0,
            refresh_hz: 0,
        };

        let normalized = normalize_request(request);

        assert_eq!(normalized.name, "PaneLink Virtual Display");
        assert_eq!(normalized.width, 1920);
        assert_eq!(normalized.height, 1080);
        assert_eq!(normalized.refresh_hz, 60);
    }

    #[test]
    fn request_clamps_to_reasonable_display_limits() {
        let request = VirtualDisplayRequest {
            name: "  Desk right  ".into(),
            width: 100_000,
            height: 120,
            refresh_hz: 240,
        };

        let normalized = normalize_request(request);

        assert_eq!(normalized.name, "Desk right");
        assert_eq!(normalized.width, 8192);
        assert_eq!(normalized.height, 480);
        assert_eq!(normalized.refresh_hz, 60);
    }
}
