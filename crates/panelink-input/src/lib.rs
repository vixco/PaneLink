use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InputEvent {
    PointerMove { x: f64, y: f64 },
    PointerButton { button: String, pressed: bool },
    Key { code: String, pressed: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBackend {
    pub name: String,
    pub available: bool,
    pub requires_permission: bool,
}

pub fn current_input_backend() -> InputBackend {
    #[cfg(target_os = "windows")]
    {
        return InputBackend {
            name: "SendInput".into(),
            available: true,
            requires_permission: false,
        };
    }

    #[cfg(target_os = "macos")]
    {
        return InputBackend {
            name: "CoreGraphics CGEvent".into(),
            available: true,
            requires_permission: true,
        };
    }

    #[allow(unreachable_code)]
    InputBackend {
        name: "No-op input backend".into(),
        available: false,
        requires_permission: false,
    }
}
