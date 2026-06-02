# PaneLink Technical Research

This note captures the implementation direction for making PaneLink a real extended-display product instead of a simple screen mirror.

## macOS Virtual Display

The current working stream captures the Mac's existing display. A real extra monitor requires creating a virtual display on the Mac, then capturing that virtual display and rendering it on the Windows monitor.

Research findings:

- SimpleDisplay creates macOS virtual monitors with Apple's private `CGVirtualDisplay` API and treats those displays as real macOS screens. It is GPL-3.0 and uses private frameworks, so PaneLink can study the behavior but should not copy code into the current Apache-2.0 codebase.
- ScreenCaptureKit is for capturing existing displays. It does not itself create an extended desktop target.
- The native PaneLink path should be a new macOS-only virtual-display module with a small typed API: create display, set mode, list displays, arrange display, destroy display, and restore previous layout.

Implementation direction:

1. Add `panelink-virtual-display` with platform-specific backends.
2. On macOS, prototype a private-API `CGVirtualDisplay` backend behind an explicit capability flag.
3. Keep a rollback snapshot before creating or arranging displays.
4. Capture the created virtual display, not the MacBook primary display.
5. Expose the capability clearly: `available`, `private-api`, `driver-required`, or `unsupported`.

Current implementation:

- `panelink-virtual-display` now exposes a typed backend report plus create/destroy requests for the UI and Tauri shell.
- On macOS, PaneLink uses its own native `CGVirtualDisplay` backend instead of BetterDisplay, SimpleDisplay, or another external helper.
- On non-macOS devices, PaneLink reports that virtual display creation must happen on the Mac source device.
- The connect/add-screen flow now blocks the "real extra monitor" path when no virtual-display backend is available, so users do not mistake a mirrored frame window for an extended display.

## Windows Monitor Selection

The Windows receiver must advertise every physical monitor, including size, scale, refresh rate, bounds, primary state, and whether it is available for a PaneLink session.

Implementation direction:

1. Add Windows display enumeration in a native crate.
2. Advertise receiver topology through discovery/control.
3. Let the Mac add a screen with one click by selecting the next free Windows monitor.
4. Add an advanced picker for manual monitor selection and arrangement.
5. Never apply layout changes without a matching rollback snapshot.

## Low-Latency Streaming

The current stream is PNG-over-HTTP. It is useful for proving capture and receiver display, but it is not the final high-FPS path.

Research findings:

- Sunshine and Moonlight show the expected production pattern: hardware capture/encoding, hardware decode, explicit quality modes, input forwarding, and latency metrics.
- Their code is GPL-3.0, so PaneLink should not copy code into the current Apache-2.0 repository. It can use the architecture as a reference.

Implementation direction:

1. Short term: make `Low latency`, `Balanced`, and `Sharp` affect frame cadence and receiver polling.
2. Medium term: replace PNG polling with an encoded video stream.
3. Long term: use platform hardware encoders: VideoToolbox on macOS and Media Foundation or GPU encoder paths on Windows.

## Remote Input

PaneLink already has typed pointer and keyboard input models, but they are not yet injected into the remote OS.

Implementation direction:

1. Add a receiver input endpoint for pointer and keyboard events.
2. Capture pointer/keyboard events in the fullscreen receiver window.
3. On macOS, inject through CoreGraphics events after Accessibility permission is granted.
4. Keep input permission visible and revocable.
5. Support direct mouse mode and captured pointer mode separately.

Current implementation:

- The fullscreen receiver sends pointer, wheel, and keyboard batches to the source device through `/input-events`.
- The source control server validates the typed batch and returns an input receipt through the existing `panelink-input` contract.
- OS-level injection is still a native backend task: the current backend accepts batches but does not yet call SendInput or CGEvent.

## License Notes

Do not copy GPL-3.0 source into PaneLink unless the project is intentionally relicensed to GPL-3.0. For now, use GPL projects only as behavioral references and implement PaneLink code independently.
