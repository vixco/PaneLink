use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InputEvent {
    PointerMove {
        x: f64,
        y: f64,
        #[serde(default)]
        screen: Option<u8>,
    },
    PointerButton {
        button: PointerButton,
        pressed: bool,
        #[serde(default)]
        screen: Option<u8>,
    },
    PointerWheel {
        delta_x: f64,
        delta_y: f64,
        #[serde(default)]
        screen: Option<u8>,
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

#[cfg(any(test, target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq)]
struct PointerDisplay {
    id: u32,
    primary: bool,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

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

pub fn set_pointer_target_display(display_id: Option<u32>) {
    set_pointer_target_display_for_screen(1, display_id);
}

pub fn set_pointer_target_display_for_screen(screen_index: u8, display_id: Option<u32>) {
    let mut targets = pointer_target_slot()
        .lock()
        .expect("pointer target mutex should not be poisoned");
    let screen_index = screen_index.clamp(1, 3);
    if let Some(display_id) = display_id {
        targets.insert(screen_index, display_id);
    } else {
        targets.remove(&screen_index);
    }
}

pub fn clear_pointer_target_display() {
    pointer_target_slot()
        .lock()
        .expect("pointer target mutex should not be poisoned")
        .clear();
}

fn pointer_target_slot() -> &'static Mutex<HashMap<u8, u32>> {
    static TARGET: OnceLock<Mutex<HashMap<u8, u32>>> = OnceLock::new();
    TARGET.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "macos")]
fn active_pointer_target_display(screen_index: Option<u8>) -> Option<u32> {
    let targets = pointer_target_slot()
        .lock()
        .expect("pointer target mutex should not be poisoned");
    let screen_index = screen_index.unwrap_or(1).clamp(1, 3);
    targets
        .get(&screen_index)
        .copied()
        .or_else(|| targets.get(&1).copied())
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
            status: macos_accessibility_status(),
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

fn macos_accessibility_status() -> InputPermissionStatus {
    #[cfg(target_os = "macos")]
    {
        if macos::accessibility_trusted() {
            InputPermissionStatus::Granted
        } else {
            InputPermissionStatus::Required
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        InputPermissionStatus::Unsupported
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

#[cfg(any(test, target_os = "macos"))]
fn pointer_target_display_for_id(
    displays: &[PointerDisplay],
    target_id: Option<u32>,
) -> Option<PointerDisplay> {
    if let Some(target_id) = target_id {
        if let Some(display) = displays
            .iter()
            .copied()
            .find(|display| display.id == target_id)
        {
            return Some(display);
        }
    }

    displays
        .iter()
        .copied()
        .filter(|display| !display.primary)
        .max_by(|left, right| {
            left.x
                .partial_cmp(&right.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.width
                        .partial_cmp(&right.width)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .or_else(|| displays.first().copied())
}

#[cfg(any(test, target_os = "macos"))]
fn normalized_point_on_display(display: PointerDisplay, x: f64, y: f64) -> (f64, f64) {
    (
        display.x + x.clamp(0.0, 1.0) * display.width.max(1.0),
        display.y + y.clamp(0.0, 1.0) * display.height.max(1.0),
    )
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        macos_key_code_for_physical, normalized_point_on_display, pointer_target_display_for_id,
        InputEvent, PointerButton, PointerDisplay,
    };
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

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> CGDirectDisplayID;
        fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
        fn CGGetActiveDisplayList(
            max_displays: u32,
            active_displays: *mut CGDirectDisplayID,
            display_count: *mut u32,
        ) -> i32;
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

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(value: CFTypeRef);
    }

    pub fn inject_events(events: &[InputEvent]) -> bool {
        if !accessibility_trusted() {
            return false;
        }

        let mut ok = true;

        for event in events {
            ok &= inject_event(event);
        }

        ok
    }

    pub fn accessibility_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    fn inject_event(event: &InputEvent) -> bool {
        match event {
            InputEvent::PointerMove { x, y, screen } => post_mouse(
                K_CG_EVENT_MOUSE_MOVED,
                point(*x, *y, *screen),
                K_CG_MOUSE_BUTTON_LEFT,
            ),
            InputEvent::PointerButton {
                button,
                pressed,
                screen,
            } => {
                let (event_type, mouse_button) = mouse_button_event(button, *pressed);
                let point = if let Some(screen) = screen {
                    last_point_for_screen(*screen)
                } else {
                    last_point()
                };
                post_mouse(event_type, point, mouse_button)
            }
            InputEvent::PointerWheel {
                delta_x: _,
                delta_y,
                screen: _,
            } => post_scroll(*delta_y),
            InputEvent::Key { code, pressed, .. } => macos_key_code_for_physical(&code.physical)
                .is_some_and(|key_code| post_key(key_code, *pressed)),
            InputEvent::Text { .. } => false,
        }
    }

    fn point(x: f64, y: f64, screen: Option<u8>) -> CGPoint {
        let display = pointer_target_display_for_id(
            &active_displays(),
            super::active_pointer_target_display(screen),
        )
        .unwrap_or(PointerDisplay {
            id: 0,
            primary: true,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        let (x, y) = normalized_point_on_display(display, x, y);
        let point = CGPoint { x, y };

        *last_point_state(screen.unwrap_or(1))
            .lock()
            .expect("last pointer state should not be poisoned") = point;

        point
    }

    fn active_displays() -> Vec<PointerDisplay> {
        let mut ids = [0_u32; 16];
        let mut count = 0_u32;
        let result =
            unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
        if result != 0 {
            return Vec::new();
        }

        let primary = unsafe { CGMainDisplayID() };
        ids.into_iter()
            .take(count as usize)
            .map(|id| {
                let bounds = unsafe { CGDisplayBounds(id) };
                PointerDisplay {
                    id,
                    primary: id == primary,
                    x: bounds.origin.x,
                    y: bounds.origin.y,
                    width: bounds.size.width,
                    height: bounds.size.height,
                }
            })
            .collect()
    }

    fn last_point() -> CGPoint {
        *last_point_state(1)
            .lock()
            .expect("last pointer state should not be poisoned")
    }

    fn last_point_for_screen(screen: u8) -> CGPoint {
        *last_point_state(screen)
            .lock()
            .expect("last pointer state should not be poisoned")
    }

    fn last_point_state(screen: u8) -> &'static Mutex<CGPoint> {
        match screen.clamp(1, 3) {
            2 => {
                static LAST_POINT_2: OnceLock<Mutex<CGPoint>> = OnceLock::new();
                LAST_POINT_2.get_or_init(|| Mutex::new(CGPoint { x: 0.0, y: 0.0 }))
            }
            3 => {
                static LAST_POINT_3: OnceLock<Mutex<CGPoint>> = OnceLock::new();
                LAST_POINT_3.get_or_init(|| Mutex::new(CGPoint { x: 0.0, y: 0.0 }))
            }
            _ => {
                static LAST_POINT_1: OnceLock<Mutex<CGPoint>> = OnceLock::new();
                LAST_POINT_1.get_or_init(|| Mutex::new(CGPoint { x: 0.0, y: 0.0 }))
            }
        }
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
            events: vec![
                InputEvent::PointerMove {
                    x: 0.0,
                    y: 0.0,
                    screen: None,
                };
                MAX_EVENTS_PER_BATCH + 1
            ],
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

    #[test]
    fn deserializes_pointer_input_with_screen_index() {
        let event: InputEvent =
            serde_json::from_str(r#"{"type":"pointerMove","x":0.25,"y":0.75,"screen":2}"#)
                .expect("event should deserialize");

        assert_eq!(
            event,
            InputEvent::PointerMove {
                x: 0.25,
                y: 0.75,
                screen: Some(2),
            }
        );
    }

    #[test]
    fn pointer_input_targets_extended_display_to_the_right() {
        let displays = [
            PointerDisplay {
                id: 1,
                primary: true,
                x: 0.0,
                y: 0.0,
                width: 1512.0,
                height: 982.0,
            },
            PointerDisplay {
                id: 2,
                primary: false,
                x: 1512.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ];

        let target =
            pointer_target_display_for_id(&displays, None).expect("display should be selected");
        let point = normalized_point_on_display(target, 0.5, 0.5);

        assert_eq!(target.id, 2);
        assert_eq!(point, (2472.0, 540.0));
    }

    #[test]
    fn pointer_input_prefers_explicit_virtual_display_id() {
        let displays = [
            PointerDisplay {
                id: 1,
                primary: true,
                x: 0.0,
                y: 0.0,
                width: 1512.0,
                height: 982.0,
            },
            PointerDisplay {
                id: 2,
                primary: false,
                x: 1512.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            PointerDisplay {
                id: 3,
                primary: false,
                x: 3432.0,
                y: 0.0,
                width: 2560.0,
                height: 1440.0,
            },
        ];

        let target =
            pointer_target_display_for_id(&displays, Some(2)).expect("display should be selected");
        let point = normalized_point_on_display(target, 0.5, 0.5);

        assert_eq!(target.id, 2);
        assert_eq!(point, (2472.0, 540.0));
    }
}
