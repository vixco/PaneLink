use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InputEvent {
    PointerMove {
        x: f64,
        y: f64,
    },
    PointerButton {
        button: PointerButton,
        pressed: bool,
    },
    PointerWheel {
        delta_x: f64,
        delta_y: f64,
    },
    Key {
        code: KeyCode,
        pressed: bool,
        modifiers: KeyModifiers,
    },
    Text {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Auxiliary,
    Back,
    Forward,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCode {
    pub physical: String,
    pub logical: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputEventBatch {
    pub batch_id: String,
    pub sequence: u64,
    pub source_peer_id: Option<String>,
    pub events: Vec<InputEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBatchReceipt {
    pub batch_id: String,
    pub accepted: bool,
    pub accepted_events: usize,
    pub backend: InputBackend,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBackendReport {
    pub backend: InputBackend,
    pub permissions: Vec<InputPermission>,
    pub batching: InputBatching,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBackend {
    pub name: String,
    pub available: bool,
    pub requires_permission: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPermission {
    pub key: String,
    pub label: String,
    pub status: InputPermissionStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputPermissionStatus {
    Granted,
    Required,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBatching {
    pub supported: bool,
    pub max_events_per_batch: usize,
}

pub const MAX_EVENTS_PER_BATCH: usize = 128;

pub fn current_input_backend() -> InputBackend {
    backend_report().backend
}

pub fn backend_report() -> InputBackendReport {
    let backend = platform_backend();

    InputBackendReport {
        permissions: platform_permissions(),
        backend,
        batching: InputBatching {
            supported: true,
            max_events_per_batch: MAX_EVENTS_PER_BATCH,
        },
    }
}

pub fn accept_batch(batch: InputEventBatch) -> InputBatchReceipt {
    let accepted_events = batch.events.len().min(MAX_EVENTS_PER_BATCH);

    InputBatchReceipt {
        batch_id: batch.batch_id,
        accepted: accepted_events == batch.events.len(),
        accepted_events,
        backend: current_input_backend(),
    }
}

fn platform_backend() -> InputBackend {
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

    #[cfg(target_os = "linux")]
    {
        return InputBackend {
            name: "uinput planned".into(),
            available: false,
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

fn platform_permissions() -> Vec<InputPermission> {
    if cfg!(target_os = "macos") {
        vec![InputPermission {
            key: "accessibility".into(),
            label: "Accessibility".into(),
            status: InputPermissionStatus::Required,
            detail: "Required before CGEvent can control pointer and keyboard input.".into(),
        }]
    } else if cfg!(target_os = "linux") {
        vec![InputPermission {
            key: "uinput".into(),
            label: "uinput".into(),
            status: InputPermissionStatus::Unsupported,
            detail: "Linux input injection is planned; no backend is active yet.".into(),
        }]
    } else {
        vec![InputPermission {
            key: "input-control".into(),
            label: "Input control".into(),
            status: InputPermissionStatus::Granted,
            detail: "The current platform does not require an app permission prompt.".into(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_typed_key_event_batch() {
        let batch = InputEventBatch {
            batch_id: "batch-1".into(),
            sequence: 7,
            source_peer_id: Some("peer-1".into()),
            events: vec![InputEvent::Key {
                code: KeyCode {
                    physical: "KeyA".into(),
                    logical: Some("a".into()),
                },
                pressed: true,
                modifiers: KeyModifiers {
                    shift: true,
                    ..KeyModifiers::default()
                },
            }],
        };

        let json = serde_json::to_string(&batch).expect("batch serializes");

        assert!(json.contains("\"type\":\"key\""));
        assert!(json.contains("\"physical\":\"KeyA\""));
    }

    #[test]
    fn rejects_oversized_batch_without_dropping_shape() {
        let batch = InputEventBatch {
            batch_id: "too-large".into(),
            sequence: 1,
            source_peer_id: None,
            events: vec![InputEvent::PointerMove { x: 0.0, y: 0.0 }; MAX_EVENTS_PER_BATCH + 1],
        };

        let receipt = accept_batch(batch);

        assert!(!receipt.accepted);
        assert_eq!(receipt.accepted_events, MAX_EVENTS_PER_BATCH);
    }
}
