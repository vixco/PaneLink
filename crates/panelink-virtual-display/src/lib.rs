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

    let id = create_platform_virtual_display(&request, &report)?;

    Ok(VirtualDisplaySession {
        id,
        active: true,
        backend: report.backend,
        display_name: request.name,
        width: request.width,
        height: request.height,
        refresh_hz: request.refresh_hz,
        message:
            "Native virtual display created. macOS may need a moment to publish the new display."
                .into(),
    })
}

pub fn destroy_virtual_display(id: String) -> Result<VirtualDisplaySession, String> {
    if id.trim().is_empty() {
        return Err("Virtual display id is missing".into());
    }

    release_platform_virtual_display(&id)?;

    Ok(VirtualDisplaySession {
        id,
        active: false,
        backend: backend_report().backend,
        display_name: "PaneLink Virtual Display".into(),
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        refresh_hz: DEFAULT_REFRESH_HZ,
        message: "Native virtual display released.".into(),
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
        return native_macos_backend_report();
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

#[cfg(any(test, target_os = "macos"))]
fn native_macos_backend_report() -> VirtualDisplayBackendReport {
    VirtualDisplayBackendReport {
        backend: "PaneLink CGVirtualDisplay".into(),
        state: VirtualDisplayState::Available,
        available: true,
        requires_external_tool: false,
        message:
            "PaneLink will create a native macOS virtual monitor with CoreGraphics CGVirtualDisplay."
                .into(),
        actions: vec!["Create native PaneLink virtual monitor".into()],
    }
}

fn create_platform_virtual_display(
    _request: &VirtualDisplayRequest,
    _report: &VirtualDisplayBackendReport,
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return macos::create_virtual_display(_request);
    }

    #[allow(unreachable_code)]
    Ok(Uuid::new_v4().to_string())
}

fn release_platform_virtual_display(_id: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        return macos::release_virtual_display(_id);
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::VirtualDisplayRequest;
    use std::{
        collections::HashMap,
        ffi::{c_char, c_void, CString},
        sync::{Mutex, OnceLock},
    };
    use uuid::Uuid;

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    struct NativeDisplay {
        display: usize,
    }

    #[link(name = "Foundation", kind = "framework")]
    extern "C" {}

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {}

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    extern "C" {
        fn dispatch_get_global_queue(identifier: isize, flags: usize) -> Id;
    }

    pub fn create_virtual_display(request: &VirtualDisplayRequest) -> Result<String, String> {
        let display = unsafe { create_native_display(request)? };
        let id = Uuid::new_v4().to_string();

        displays()
            .lock()
            .map_err(|_| "Virtual display registry is unavailable".to_string())?
            .insert(
                id.clone(),
                NativeDisplay {
                    display: display as usize,
                },
            );

        Ok(id)
    }

    pub fn release_virtual_display(id: &str) -> Result<(), String> {
        let display = displays()
            .lock()
            .map_err(|_| "Virtual display registry is unavailable".to_string())?
            .remove(id);

        if let Some(display) = display {
            unsafe {
                msg_send_void(display.display as Id, "release");
            }
        }

        Ok(())
    }

    fn displays() -> &'static Mutex<HashMap<String, NativeDisplay>> {
        static DISPLAYS: OnceLock<Mutex<HashMap<String, NativeDisplay>>> = OnceLock::new();
        DISPLAYS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    unsafe fn create_native_display(request: &VirtualDisplayRequest) -> Result<Id, String> {
        let descriptor_class = class("CGVirtualDisplayDescriptor")?;
        let display_class = class("CGVirtualDisplay")?;
        let settings_class = class("CGVirtualDisplaySettings")?;
        let mode_class = class("CGVirtualDisplayMode")?;

        let descriptor = msg_send_id(msg_send_id(descriptor_class, "alloc"), "init");
        let settings = msg_send_id(msg_send_id(settings_class, "alloc"), "init");
        let queue = dispatch_get_global_queue(2, 0);
        let name = ns_string(&request.name)?;
        let ppi = 110.0;
        let width = request.width;
        let height = request.height;

        msg_send_void_id(descriptor, "setQueue:", queue);
        msg_send_void_id(descriptor, "setName:", name);
        msg_send_void_u32(descriptor, "setVendorID:", 0x504c);
        msg_send_void_u32(descriptor, "setProductID:", 0x1001);
        msg_send_void_u32(descriptor, "setSerialNum:", 1);
        msg_send_void_u32(descriptor, "setMaxPixelsWide:", width);
        msg_send_void_u32(descriptor, "setMaxPixelsHigh:", height);
        msg_send_void_cgsize(
            descriptor,
            "setSizeInMillimeters:",
            CGSize {
                width: 25.4 * f64::from(width) / ppi,
                height: 25.4 * f64::from(height) / ppi,
            },
        );
        msg_send_void_cgpoint(
            descriptor,
            "setWhitePoint:",
            CGPoint {
                x: 0.3125,
                y: 0.3291,
            },
        );
        msg_send_void_cgpoint(
            descriptor,
            "setRedPrimary:",
            CGPoint {
                x: 0.6797,
                y: 0.3203,
            },
        );
        msg_send_void_cgpoint(
            descriptor,
            "setGreenPrimary:",
            CGPoint {
                x: 0.2559,
                y: 0.6983,
            },
        );
        msg_send_void_cgpoint(
            descriptor,
            "setBluePrimary:",
            CGPoint {
                x: 0.1494,
                y: 0.0557,
            },
        );

        let display = msg_send_id_id(
            msg_send_id(display_class, "alloc"),
            "initWithDescriptor:",
            descriptor,
        );

        if display.is_null() {
            return Err("CGVirtualDisplay could not initialize a native display".into());
        }

        let mode = msg_send_id_u32_u32_f64(
            msg_send_id(mode_class, "alloc"),
            "initWithWidth:height:refreshRate:",
            width,
            height,
            f64::from(request.refresh_hz),
        );
        let modes = ns_array_with_object(mode)?;

        msg_send_void_u32(settings, "setHiDPI:", 0);
        msg_send_void_id(settings, "setModes:", modes);

        if !msg_send_bool_id(display, "applySettings:", settings) {
            msg_send_void(display, "release");
            return Err("CGVirtualDisplay rejected the requested display mode".into());
        }

        Ok(display)
    }

    unsafe fn class(name: &str) -> Result<Id, String> {
        let name = CString::new(name).map_err(|error| error.to_string())?;
        let class = objc_getClass(name.as_ptr());
        if class.is_null() {
            Err(format!(
                "Objective-C class {} is not available",
                name.to_string_lossy()
            ))
        } else {
            Ok(class)
        }
    }

    unsafe fn sel(name: &str) -> Sel {
        let name = CString::new(name).expect("selector names are static and contain no nul bytes");
        sel_registerName(name.as_ptr())
    }

    unsafe fn ns_string(value: &str) -> Result<Id, String> {
        let class = class("NSString")?;
        let value = CString::new(value).map_err(|error| error.to_string())?;
        let send: extern "C" fn(Id, Sel, *const c_char) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        Ok(send(class, sel("stringWithUTF8String:"), value.as_ptr()))
    }

    unsafe fn ns_array_with_object(value: Id) -> Result<Id, String> {
        let class = class("NSArray")?;
        let send: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        Ok(send(class, sel("arrayWithObject:"), value))
    }

    unsafe fn msg_send_id(receiver: Id, selector: &str) -> Id {
        let send: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector))
    }

    unsafe fn msg_send_id_id(receiver: Id, selector: &str, value: Id) -> Id {
        let send: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value)
    }

    unsafe fn msg_send_id_u32_u32_f64(
        receiver: Id,
        selector: &str,
        width: u32,
        height: u32,
        refresh_hz: f64,
    ) -> Id {
        let send: extern "C" fn(Id, Sel, u32, u32, f64) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), width, height, refresh_hz)
    }

    unsafe fn msg_send_bool_id(receiver: Id, selector: &str, value: Id) -> bool {
        let send: extern "C" fn(Id, Sel, Id) -> i8 = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value) != 0
    }

    unsafe fn msg_send_void(receiver: Id, selector: &str) {
        let send: extern "C" fn(Id, Sel) = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector));
    }

    unsafe fn msg_send_void_id(receiver: Id, selector: &str, value: Id) {
        let send: extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value);
    }

    unsafe fn msg_send_void_u32(receiver: Id, selector: &str, value: u32) {
        let send: extern "C" fn(Id, Sel, u32) = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value);
    }

    unsafe fn msg_send_void_cgpoint(receiver: Id, selector: &str, value: CGPoint) {
        let send: extern "C" fn(Id, Sel, CGPoint) = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value);
    }

    unsafe fn msg_send_void_cgsize(receiver: Id, selector: &str, value: CGSize) {
        let send: extern "C" fn(Id, Sel, CGSize) = std::mem::transmute(objc_msgSend as *const ());
        send(receiver, sel(selector), value);
    }
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

    #[test]
    fn macos_backend_contract_is_native_not_external_helper() {
        let report = native_macos_backend_report();

        assert_eq!(report.state, VirtualDisplayState::Available);
        assert!(report.available);
        assert!(!report.requires_external_tool);
        assert!(!report.backend.contains("BetterDisplay"));
        assert!(!report.backend.contains("SimpleDisplay"));
        assert!(!report.message.contains("BetterDisplay"));
        assert!(!report.message.contains("SimpleDisplay"));
    }
}
