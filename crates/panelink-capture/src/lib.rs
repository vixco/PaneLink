use serde::{Deserialize, Serialize};

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
            note: "Frame capture backend planned through the windows crate.".into(),
        };
    }

    #[cfg(target_os = "macos")]
    {
        return CaptureBackend {
            name: "ScreenCaptureKit".into(),
            available: true,
            requires_permission: true,
            note: "Requires Screen Recording permission before live capture.".into(),
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
