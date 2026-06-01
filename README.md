# PaneLink

PaneLink is an open-source desktop app for instant local display switching between a MacBook and a Windows desk setup.

The goal is simple: open the app, pick the trusted device on your LAN, and press **Switch Display**. PaneLink is designed around a direct local network path, low-latency video transport, audio output routing, microphone routing, and input forwarding.

> Status: early foundation. The repository currently contains the cross-platform Tauri app, Rust core modules, UI, device/session models, audio device enumeration, and platform-aware capture/input boundaries. Real virtual display drivers and full default audio/mic takeover still require native driver work.

## Why PaneLink

Existing remote desktop tools are powerful, but often feel heavy for one specific desk flow. PaneLink focuses on:

- one clear switch action
- LAN-first direct pairing
- low-latency transport design
- visible audio and microphone routes
- Windows and macOS builds from the start
- a clean open-source architecture

## Architecture

PaneLink uses a Tauri v2 shell with a React/Vite frontend and Rust backend crates:

```text
crates/
  panelink-core        shared peer, session, capability and device models
  panelink-discovery   LAN discovery shape for _panelink._udp.local
  panelink-transport   QUIC/WebRTC channel plan
  panelink-capture     Windows/macOS capture backend boundaries
  panelink-audio       audio device enumeration and future stream routing
  panelink-input       keyboard and pointer event schema/backends
src-tauri/             Tauri command bridge
src/                   React desktop UI
```

See [docs/architecture.md](docs/architecture.md) for the implementation path.

## Development

Requirements:

- Node.js 22+
- npm 11+
- Rust 1.94+
- platform dependencies required by Tauri

Install dependencies:

```bash
npm install
```

Run the frontend:

```bash
npm run dev
```

Run the desktop app:

```bash
npm run tauri:dev
```

Build checks:

```bash
npm run build
cargo test --workspace
```

## Roadmap

See [docs/roadmap.md](docs/roadmap.md).

## Releases and updates

PaneLink builds Windows and macOS downloads through GitHub Actions. Installed apps check GitHub Releases for signed updates. See [docs/release.md](docs/release.md).

## License

Apache-2.0. See [LICENSE](LICENSE).
