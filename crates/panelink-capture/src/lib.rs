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
            available: false,
            requires_permission: false,
            note: "Native frame capture is not wired yet; this build cannot show remote pixels."
                .into(),
        };
    }

    #[cfg(target_os = "macos")]
    {
        return CaptureBackend {
            name: "ScreenCaptureKit".into(),
            available: false,
            requires_permission: true,
            note: "ScreenCaptureKit permission is required, but frame transport is not wired yet."
                .into(),
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
