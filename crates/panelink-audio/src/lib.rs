use cpal::traits::{DeviceTrait, HostTrait};
use panelink_core::{AudioDevice, AudioDeviceKind};

pub fn list_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_output = host
        .default_output_device()
        .and_then(|device| device.name().ok());
    let default_input = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let mut devices = Vec::new();

    if let Ok(outputs) = host.output_devices() {
        for (index, device) in outputs.enumerate() {
            let name = device
                .name()
                .unwrap_or_else(|_| format!("Output Device {}", index + 1));
            devices.push(AudioDevice {
                id: format!("output-{index}"),
                is_default: default_output.as_deref() == Some(name.as_str()),
                name,
                kind: AudioDeviceKind::Output,
                available: true,
            });
        }
    }

    if let Ok(inputs) = host.input_devices() {
        for (index, device) in inputs.enumerate() {
            let name = device
                .name()
                .unwrap_or_else(|_| format!("Input Device {}", index + 1));
            devices.push(AudioDevice {
                id: format!("input-{index}"),
                is_default: default_input.as_deref() == Some(name.as_str()),
                name,
                kind: AudioDeviceKind::Input,
                available: true,
            });
        }
    }

    if devices.is_empty() {
        devices.extend([
            AudioDevice {
                id: "default-output".into(),
                name: "System Default Output".into(),
                kind: AudioDeviceKind::Output,
                is_default: true,
                available: false,
            },
            AudioDevice {
                id: "default-input".into(),
                name: "System Default Microphone".into(),
                kind: AudioDeviceKind::Input,
                is_default: true,
                available: false,
            },
        ]);
    }

    devices
}
