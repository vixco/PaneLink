# PaneLink Architecture

PaneLink is built as a local-first desktop system. The frontend is a Tauri webview; Rust owns discovery, pairing, transport, capture, audio, input and OS permissions.

## Backend-first module layout

- `panelink-core`: stable shared models for peers, sessions, devices, capabilities and permission state.
- `panelink-discovery`: LAN discovery contract using `_panelink._udp.local`.
- `panelink-transport`: direct LAN QUIC channel plan with WebRTC reserved for future NAT traversal.
- `panelink-capture`: platform capture boundary.
- `panelink-audio`: audio device enumeration and future capture/playback/routing.
- `panelink-input`: keyboard and pointer event schema plus platform backend boundary.
- `src-tauri`: command bridge consumed by the desktop UI.

## Video path

Target production path:

1. Source machine captures frames.
2. Hardware encoder emits low-latency H.264 first, then HEVC/AV1 where supported.
3. Transport sends encoded frames over QUIC datagrams on direct LAN.
4. Receiver decodes and renders in the PaneLink window or fullscreen display host.

Platform capture targets:

- Windows: DXGI Desktop Duplication, then Windows Graphics Capture where useful.
- macOS: ScreenCaptureKit with Screen Recording permission.

Virtual display support is separate from screen capture. A true "Mac sees Windows monitor as a real external display" experience needs a virtual display component on macOS and display/output handling on Windows. That is driver-level work and cannot be implemented purely in React/Tauri UI code.

## Multi-screen sessions

PaneLink models each connected monitor as a remote screen slot. A Windows PC with three monitors can expose three target displays. The Mac can then add one or more screen slots and map them to those targets.

The intended connection flow:

1. Receiver advertises all physical displays with native resolution, refresh rate, safe scaling range and current layout.
2. Sender presses **Add screen** and chooses a target display.
3. PaneLink computes an auto-fit mode that preserves aspect ratio and picks the best supported resolution/refresh rate for the target.
4. PaneLink stores the previous display layout before applying the session.
5. On disconnect, crash recovery, timeout or user cancel, PaneLink restores the previous local and remote display layout.

The rollback snapshot is mandatory. No display mode change should be applied without a matching restore plan.

## Audio and microphone path

MVP foundation:

- enumerate input/output devices with `cpal`
- model default output and microphone selection
- expose routing state in the UI and Tauri commands

Target production path:

- Windows output capture: WASAPI loopback
- Windows microphone capture: WASAPI input capture
- macOS system/app audio capture: ScreenCaptureKit where available
- macOS microphone capture: Core Audio input
- packetize audio on a dedicated low-latency channel with jitter buffering

Full default speaker/mic takeover requires virtual audio devices:

- Windows: signed virtual audio driver based on the SysVAD model
- macOS: Core Audio driver extension or aggregate/virtual device approach

## Transport

Default LAN transport is QUIC:

- control: reliable stream
- input: reliable low-latency stream
- video: datagrams
- audio: datagrams with jitter buffer
- metrics: periodic reliable stream

WebRTC is planned for non-LAN or NAT traversal scenarios.

## Security

PaneLink is designed for trusted LAN use:

- mDNS discovery only advertises capabilities and pairing state
- pairing tokens establish trusted peers
- media/control channels should be encrypted
- remote input should stay permission-gated and visible to the user
