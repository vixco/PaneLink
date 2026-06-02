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
    let injected = inject_events(&batch.events[..accepted_events]);

    InputBatchReceipt {
        batch_id: batch.batch_id,
        accepted: accepted_events == batch.events.len() && injected,
        accepted_events,
        backend: current_input_backend(),
    }
}

fn inject_events(events: &[InputEvent]) -> bool {
    #[cfg(target_os = "macos")]
    {
        return macos::inject_events(events);
    }

    #[allow(unreachable_code)]
    {
        let _ = events;
        true
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

#[cfg(any(test, target_os = "macos"))]
fn macos_key_code_for_physical(physical: &str) -> Option<u16> {
    Some(match physical {
        "KeyA" => 0,
        "KeyS" => 1,
        "KeyD" => 2,
        "KeyF" => 3,
        "KeyH" => 4,
        "KeyG" => 5,
        "KeyZ" => 6,
        "KeyX" => 7,
        "KeyC" => 8,
        "KeyV" => 9,
        "KeyB" => 11,
        "KeyQ" => 12,
        "KeyW" => 13,
        "KeyE" => 14,
        "KeyR" => 15,
        "KeyY" => 16,
        "KeyT" => 17,
        "Digit1" => 18,
        "Digit2" => 19,
        "Digit3" => 20,
        "Digit4" => 21,
        "Digit6" => 22,
        "Digit5" => 23,
        "Equal" => 24,
        "Digit9" => 25,
        "Digit7" => 26,
        "Minus" => 27,
        "Digit8" => 28,
        "Digit0" => 29,
        "BracketRight" => 30,
        "KeyO" => 31,
        "KeyU" => 32,
        "BracketLeft" => 33,
        "KeyI" => 34,
        "KeyP" => 35,
        "Enter" => 36,
        "KeyL" => 37,
        "KeyJ" => 38,
        "Quote" => 39,
        "KeyK" => 40,
        "Semicolon" => 41,
        "Backslash" => 42,
        "Comma" => 43,
        "Slash" => 44,
        "KeyN" => 45,
        "KeyM" => 46,
        "Period" => 47,
        "Tab" => 48,
        "Space" => 49,
        "Backquote" => 50,
        "Backspace" => 51,
        "Escape" => 53,
        "MetaRight" => 54,
        "MetaLeft" => 55,
        "ShiftLeft" => 56,
        "CapsLock" => 57,
        "AltLeft" => 58,
        "ControlLeft" => 59,
        "ShiftRight" => 60,
        "AltRight" => 61,
        "ControlRight" => 62,
        "ArrowLeft" => 123,
        "ArrowRight" => 124,
        "ArrowDown" => 125,
        "ArrowUp" => 126,
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{macos_key_code_for_physical, InputEvent, PointerButton};
    use std::{
        ffi::c_void,
        sync::{Mutex, OnceLock},
    };

    type CGDirectDisplayID = u32;
    type CGEventRef = *mut c_void;
    type CFTypeRef = *const c_void;

    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
    const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
    const K_CG_MOUSE_BUTTON_LEFT: u32 = 0;
    const K_CG_MOUSE_BUTTON_RIGHT: u32 = 1;
    const K_CG_MOUSE_BUTTON_CENTER: u32 = 2;
    const K_CG_SCROLL_EVENT_UNIT_PIXEL: u32 = 0;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> CGDirectDisplayID;
        fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> usize;
        fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> usize;
        fn CGEventCreateMouseEvent(
            source: *const c_void,
            mouse_type: u32,
            mouse_cursor_position: CGPoint,
            mouse_button: u32,
        ) -> CGEventRef;
        fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> CGEventRef;
        fn CGEventCreateScrollWheelEvent(
            source: *const c_void,
            units: u32,
            wheel_count: u32,
            wheel1: i32,
            ...
        ) -> CGEventRef;
        fn CGEventPost(tap: u32, event: CGEventRef);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: CFTypeRef);
    }

    pub fn inject_events(events: &[InputEvent]) -> bool {
        let mut ok = true;

        for event in events {
            ok &= inject_event(event);
        }

        ok
    }

    fn inject_event(event: &InputEvent) -> bool {
        match event {
            InputEvent::PointerMove { x, y } => post_mouse(
                K_CG_EVENT_MOUSE_MOVED,
                point(*x, *y),
                K_CG_MOUSE_BUTTON_LEFT,
            ),
            InputEvent::PointerButton { button, pressed } => {
                let (event_type, mouse_button) = mouse_button_event(button, *pressed);
                post_mouse(event_type, last_point(), mouse_button)
            }
            InputEvent::PointerWheel {
                delta_x: _,
                delta_y,
            } => post_scroll(*delta_y),
            InputEvent::Key { code, pressed, .. } => macos_key_code_for_physical(&code.physical)
                .is_some_and(|key_code| post_key(key_code, *pressed)),
            InputEvent::Text { .. } => false,
        }
    }

    fn point(x: f64, y: f64) -> CGPoint {
        let display = unsafe { CGMainDisplayID() };
        let width = unsafe { CGDisplayPixelsWide(display) }.max(1) as f64;
        let height = unsafe { CGDisplayPixelsHigh(display) }.max(1) as f64;
        let point = CGPoint {
            x: x.clamp(0.0, 1.0) * width,
            y: y.clamp(0.0, 1.0) * height,
        };

        *last_point_state()
            .lock()
            .expect("last pointer state should not be poisoned") = point;

        point
    }

    fn last_point() -> CGPoint {
        *last_point_state()
            .lock()
            .expect("last pointer state should not be poisoned")
    }

    fn last_point_state() -> &'static Mutex<CGPoint> {
        static LAST_POINT: OnceLock<Mutex<CGPoint>> = OnceLock::new();
        LAST_POINT.get_or_init(|| Mutex::new(CGPoint { x: 0.0, y: 0.0 }))
    }

    fn mouse_button_event(button: &PointerButton, pressed: bool) -> (u32, u32) {
        match button {
            PointerButton::Primary => (
                if pressed {
                    K_CG_EVENT_LEFT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_LEFT_MOUSE_UP
                },
                K_CG_MOUSE_BUTTON_LEFT,
            ),
            PointerButton::Secondary => (
                if pressed {
                    K_CG_EVENT_RIGHT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_RIGHT_MOUSE_UP
                },
                K_CG_MOUSE_BUTTON_RIGHT,
            ),
            _ => (
                if pressed {
                    K_CG_EVENT_OTHER_MOUSE_DOWN
                } else {
                    K_CG_EVENT_OTHER_MOUSE_UP
                },
                K_CG_MOUSE_BUTTON_CENTER,
            ),
        }
    }

    fn post_mouse(event_type: u32, point: CGPoint, button: u32) -> bool {
        unsafe {
            let event = CGEventCreateMouseEvent(std::ptr::null(), event_type, point, button);
            post_event(event)
        }
    }

    fn post_scroll(delta_y: f64) -> bool {
        let delta = (-delta_y).round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        unsafe {
            let event = CGEventCreateScrollWheelEvent(
                std::ptr::null(),
                K_CG_SCROLL_EVENT_UNIT_PIXEL,
                1,
                delta,
            );
            post_event(event)
        }
    }

    fn post_key(key_code: u16, pressed: bool) -> bool {
        unsafe {
            let event = CGEventCreateKeyboardEvent(std::ptr::null(), key_code, pressed);
            post_event(event)
        }
    }

    unsafe fn post_event(event: CGEventRef) -> bool {
        if event.is_null() {
            return false;
        }

        CGEventPost(K_CG_HID_EVENT_TAP, event);
        CFRelease(event as CFTypeRef);
        true
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

    #[test]
    fn maps_common_web_keys_to_macos_keycodes() {
        assert_eq!(macos_key_code_for_physical("KeyA"), Some(0));
        assert_eq!(macos_key_code_for_physical("Space"), Some(49));
        assert_eq!(macos_key_code_for_physical("ArrowRight"), Some(124));
        assert_eq!(macos_key_code_for_physical("UnknownKey"), None);
    }
}
