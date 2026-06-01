use cpal::traits::{DeviceTrait, HostTrait};
use panelink_core::{AudioDevice, AudioDeviceKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioBackendReport {
    pub name: String,
    pub available: bool,
    pub default_output_available: bool,
    pub default_microphone_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRouteCatalog {
    pub backend: AudioBackendReport,
    pub devices: Vec<AudioDevice>,
    pub output: AudioRouteSelection,
    pub microphone: AudioRouteSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRouteSelection {
    pub role: AudioRouteRole,
    pub selected_option_id: String,
    pub options: Vec<AudioRouteOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioRouteRole {
    DefaultOutput,
    Microphone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRouteOption {
    pub id: String,
    pub label: String,
    pub target: AudioRouteTarget,
    pub device_id: Option<String>,
    pub is_default: bool,
    pub available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioRouteTarget {
    SystemDefault,
    SpecificDevice,
    Unavailable,
}

pub fn list_devices() -> Vec<AudioDevice> {
    enumerate_devices().devices
}

pub fn get_route_catalog() -> AudioRouteCatalog {
    route_catalog_from_inventory(enumerate_devices())
}

pub fn route_catalog_from_devices(devices: Vec<AudioDevice>) -> AudioRouteCatalog {
    let default_output_available = devices.iter().any(|device| {
        kind_matches(device.kind.clone(), AudioDeviceKind::Output)
            && device.is_default
            && device.available
    });
    let default_microphone_available = devices.iter().any(|device| {
        kind_matches(device.kind.clone(), AudioDeviceKind::Input)
            && device.is_default
            && device.available
    });

    route_catalog_from_inventory(AudioInventory {
        backend_name: "test".into(),
        default_output_available,
        default_microphone_available,
        devices,
    })
}

fn route_catalog_from_inventory(inventory: AudioInventory) -> AudioRouteCatalog {
    let backend = AudioBackendReport {
        name: inventory.backend_name,
        available: !inventory.devices.is_empty(),
        default_output_available: inventory.default_output_available,
        default_microphone_available: inventory.default_microphone_available,
    };
    let output = route_selection_for_kind(
        AudioRouteRole::DefaultOutput,
        AudioDeviceKind::Output,
        "system-default-output",
        "System Default Output",
        backend.default_output_available,
        &inventory.devices,
    );
    let microphone = route_selection_for_kind(
        AudioRouteRole::Microphone,
        AudioDeviceKind::Input,
        "system-default-microphone",
        "System Default Microphone",
        backend.default_microphone_available,
        &inventory.devices,
    );

    AudioRouteCatalog {
        backend,
        devices: inventory.devices,
        output,
        microphone,
    }
}

fn route_selection_for_kind(
    role: AudioRouteRole,
    kind: AudioDeviceKind,
    default_id: &str,
    default_label: &str,
    default_available: bool,
    devices: &[AudioDevice],
) -> AudioRouteSelection {
    let mut options = vec![AudioRouteOption {
        id: default_id.into(),
        label: default_label.into(),
        target: if default_available {
            AudioRouteTarget::SystemDefault
        } else {
            AudioRouteTarget::Unavailable
        },
        device_id: None,
        is_default: true,
        available: default_available,
    }];

    options.extend(
        devices
            .iter()
            .filter(|device| kind_matches(device.kind.clone(), kind.clone()))
            .map(|device| AudioRouteOption {
                id: format!("device:{}", device.id),
                label: device.name.clone(),
                target: AudioRouteTarget::SpecificDevice,
                device_id: Some(device.id.clone()),
                is_default: device.is_default,
                available: device.available,
            }),
    );

    let selected_option_id = options
        .iter()
        .find(|option| option.target == AudioRouteTarget::SystemDefault && option.available)
        .or_else(|| {
            options
                .iter()
                .find(|option| option.is_default && option.available)
        })
        .or_else(|| options.iter().find(|option| option.available))
        .map(|option| option.id.clone())
        .unwrap_or_else(|| default_id.into());

    AudioRouteSelection {
        role,
        selected_option_id,
        options,
    }
}

struct AudioInventory {
    backend_name: String,
    default_output_available: bool,
    default_microphone_available: bool,
    devices: Vec<AudioDevice>,
}

fn enumerate_devices() -> AudioInventory {
    let host = cpal::default_host();
    let default_output = host.default_output_device();
    let default_input = host.default_input_device();
    let default_output_name = default_output.as_ref().and_then(device_name);
    let default_input_name = default_input.as_ref().and_then(device_name);
    let mut devices = Vec::new();

    if let Ok(outputs) = host.output_devices() {
        for (index, device) in outputs.enumerate() {
            let name =
                device_name(&device).unwrap_or_else(|| format!("Output Device {}", index + 1));
            devices.push(AudioDevice {
                id: device_id(AudioDeviceKind::Output, index, &name),
                is_default: default_output_name.as_deref() == Some(name.as_str()),
                name,
                kind: AudioDeviceKind::Output,
                available: true,
            });
        }
    }

    if let Ok(inputs) = host.input_devices() {
        for (index, device) in inputs.enumerate() {
            let name =
                device_name(&device).unwrap_or_else(|| format!("Input Device {}", index + 1));
            devices.push(AudioDevice {
                id: device_id(AudioDeviceKind::Input, index, &name),
                is_default: default_input_name.as_deref() == Some(name.as_str()),
                name,
                kind: AudioDeviceKind::Input,
                available: true,
            });
        }
    }

    AudioInventory {
        backend_name: host.id().name().into(),
        default_output_available: default_output.is_some(),
        default_microphone_available: default_input.is_some(),
        devices,
    }
}

fn device_name(device: &cpal::Device) -> Option<String> {
    device.name().ok().filter(|name| !name.trim().is_empty())
}

fn device_id(kind: AudioDeviceKind, index: usize, name: &str) -> String {
    let prefix = match kind {
        AudioDeviceKind::Output => "output",
        AudioDeviceKind::Input => "input",
    };
    format!("{prefix}-{index}-{}", stable_name_hash(name))
}

fn kind_matches(left: AudioDeviceKind, right: AudioDeviceKind) -> bool {
    matches!(
        (left, right),
        (AudioDeviceKind::Output, AudioDeviceKind::Output)
            | (AudioDeviceKind::Input, AudioDeviceKind::Input)
    )
}

fn stable_name_hash(name: &str) -> u32 {
    name.bytes().fold(0x811c9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_route_catalog_without_hardware() {
        let catalog = route_catalog_from_devices(Vec::new());

        assert!(!catalog.backend.available);
        assert_eq!(catalog.output.selected_option_id, "system-default-output");
        assert_eq!(
            catalog.microphone.selected_option_id,
            "system-default-microphone"
        );
        assert_eq!(
            catalog.output.options[0].target,
            AudioRouteTarget::Unavailable
        );
    }

    #[test]
    fn includes_specific_devices_after_system_default() {
        let catalog = route_catalog_from_devices(vec![AudioDevice {
            id: "output-0-test".into(),
            name: "Desk Speakers".into(),
            kind: AudioDeviceKind::Output,
            is_default: true,
            available: true,
        }]);

        assert!(catalog.backend.default_output_available);
        assert_eq!(catalog.output.options[0].id, "system-default-output");
        assert_eq!(catalog.output.options[1].id, "device:output-0-test");
        assert_eq!(catalog.output.selected_option_id, "system-default-output");
    }

    #[test]
    fn device_ids_are_stable_for_same_name() {
        assert_eq!(
            device_id(AudioDeviceKind::Input, 2, "Studio Mic"),
            device_id(AudioDeviceKind::Input, 2, "Studio Mic")
        );
    }
}
