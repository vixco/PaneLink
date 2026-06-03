# PaneLink

PaneLink is an open-source LAN desktop app for using a Windows desk setup with a MacBook.

Current MVP focus: **Mac host -> Windows client**.

The current MVP creates a macOS PaneLink virtual display, streams that display to a fullscreen Windows viewer over a local H.264 LAN stream, and forwards basic mouse and keyboard events back to the Mac over LAN. It does not pretend to be a finished Parsec replacement.

## What Works Today

- Tauri v2 desktop app for macOS and Windows.
- LAN peer discovery over UDP on port `48170`.
- Manual Mac host connect by IP from the Windows client.
- Mac PaneLink virtual display creation through CoreGraphics.
- Mac screen capture through the existing native capture backend, preferring the PaneLink virtual display.
- Windows fullscreen/borderless display window.
- H.264 OpenH264 frame streaming over LAN on port `48172`.
- PNG frame streaming fallback/debug path over LAN on port `48171`.
- Mouse move, click, wheel, and keyboard event forwarding to the Mac host control server.
- Basic pairing/trust UI, connect/disconnect UI, status messages, and setup actions.
- Fresh clone build with npm, Rust, and Tauri.

## Current Limitations

- MVP supports one streamed display reliably first. Three-monitor switching is not implemented yet.
- The current video path is H.264 over a simple LAN HTTP stream decoded with WebCodecs on Windows. It is not full WebRTC yet, so latency/jitter control is still basic.
- OpenH264 is software encoding in this release, not Apple VideoToolbox hardware encoding yet.
- macOS input forwarding requires Accessibility permission. If the permission is missing, macOS may block injected mouse/keyboard events.
- macOS screen capture requires Screen Recording permission.
- Audio and microphone routing are not implemented in the MVP.
- Discovery can still be affected by firewalls or VPNs, so the Windows app includes manual Mac IP entry.
- Release updater signing requires `TAURI_SIGNING_PRIVATE_KEY`; local app builds can use `npx tauri build --no-bundle`.

## Requirements

- Node.js 22+
- npm 11+
- Rust 1.94+
- Tauri platform prerequisites
- macOS host and Windows client on the same LAN
- Firewall access for:
  - UDP/TCP `48170` for discovery and control
  - TCP `48172` for H.264 video streaming
  - TCP `48171` for PNG fallback/debug frame streaming

## Fresh Clone Setup

```bash
git clone https://github.com/vixco/PaneLink.git
cd PaneLink
npm install
```

Run checks:

```bash
npm test
npm run build
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Build the desktop app without updater signing:

```bash
npx tauri build --no-bundle
```

Build signed updater artifacts only when the signing key is available:

```bash
$env:TAURI_SIGNING_PRIVATE_KEY="..."
npm run tauri:build
```

## Run The MVP

### 1. Start The Mac Host

On the MacBook:

```bash
npm install
npm run tauri:dev
```

Then open macOS settings and allow PaneLink:

- System Settings -> Privacy & Security -> Screen Recording
- System Settings -> Privacy & Security -> Accessibility

Restart PaneLink after changing permissions.

Find the Mac LAN IP:

```bash
ipconfig getifaddr en0
```

If you use Ethernet or another adapter, check:

```bash
ifconfig
```

### 2. Start The Windows Client

On Windows:

```powershell
npm install
npm run tauri:dev
```

Allow PaneLink through Windows Firewall when prompted.

If discovery finds the Mac, select it and click connect. If discovery does not find it, use **Manual Mac host** and enter the Mac IP, for example:

```text
192.168.1.42
```

Click **Add**, select the manual Mac peer, then click **Connect**.

### 3. Expected Result

The Windows app opens a fullscreen display window and loads H.264 chunks from:

```text
http://<mac-lan-ip>:48172/h264
```

Mouse and keyboard events from the Windows display window are sent back to:

```text
http://<mac-lan-ip>:48170/input-events
```

## Troubleshooting

If the Windows viewer says it cannot connect:

- Confirm both devices are on the same LAN.
- Disable VPN temporarily or allow local LAN bypass.
- Check Windows Firewall allows PaneLink.
- Check macOS Firewall allows incoming connections.
- Try manual Mac IP instead of discovery.
- On the Mac, confirm Screen Recording permission is granted.
- On the Mac, confirm Accessibility permission is granted for input forwarding.
- Confirm TCP `48172` is reachable from Windows to the Mac.

If frames load but input does not work:

- Recheck macOS Accessibility permission.
- Restart PaneLink after granting permission.
- Some keys may not map yet; common letters, arrows, modifiers, space, enter, escape, and tab are covered first.

## Architecture

PaneLink uses a Tauri v2 shell with a React/Vite frontend and Rust backend crates:

```text
crates/
  panelink-core             shared peer, session, capability and device models
  panelink-discovery        LAN discovery and peer cache
  panelink-transport        MVP session state and future transport planning
  panelink-capture          macOS/Windows screen capture and frame server
  panelink-input            input event schema and macOS CGEvent injection
  panelink-audio            audio device enumeration and future routing
  panelink-virtual-display  future virtual display backend boundary
  panelink-video            H.264 LAN stream server and native video session boundary
src-tauri/                  Tauri command bridge and LAN control server
src/                        React desktop UI
```

## Next Roadmap

1. Replace software OpenH264 with Apple VideoToolbox hardware encode on macOS.
2. Move the H.264 stream onto WebRTC/RTP or QUIC for better jitter, congestion, and reconnect behavior.
3. Add monitor selection on the Windows viewer for multi-monitor desks.
4. Harden input forwarding with permission detection, better key maps, clipboard support, and pointer scaling per display.
5. Add an end-to-end LAN smoke test that starts a host, starts a client, opens a display, decodes H.264, and submits an input batch.

## License

Apache-2.0. See [LICENSE](LICENSE).
