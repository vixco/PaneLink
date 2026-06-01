# Roadmap

## V0: Foundation

- Cross-platform Tauri app
- Professional desktop UI
- Rust workspace and stable shared models
- Tauri command bridge
- Audio device enumeration
- Platform-aware capture/input capability reporting
- GitHub Actions for frontend and Rust checks

## V1: Direct LAN Session

- mDNS advertise/browse implementation
- Pairing flow with trusted device storage
- QUIC connection bootstrap
- Ping/metrics stream
- Fake/test video source over transport
- Input event forwarding in a controlled test mode
- Multi-screen slot model with add/remove screen state
- Resolution auto-fit planning and rollback snapshots

## V2: Real Capture and Render

- Windows DXGI capture
- macOS ScreenCaptureKit capture
- H.264 hardware encoder abstraction
- Receiver render surface
- Fullscreen receiver mode
- Cursor capture and composition
- Apply and restore display layouts on connect/disconnect

## V3: Audio and Microphone Streaming

- WASAPI loopback on Windows
- Core Audio or ScreenCaptureKit audio on macOS
- Microphone capture
- Low-latency jitter buffer
- Device selection and reconnect behavior

## V4: Virtual Display and Default Routing

- macOS virtual display component
- Windows display host improvements
- Signed Windows virtual audio driver
- macOS virtual/aggregate audio device
- Default speaker and microphone handoff
- Installer and permission onboarding

## Product bar

PaneLink should feel simpler than a remote desktop app:

- one primary switch action
- clear device trust state
- visible latency and packet loss
- settings that stay understandable
- no relay unless explicitly enabled
