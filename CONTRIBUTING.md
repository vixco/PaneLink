# Contributing

PaneLink welcomes practical, focused contributions.

## Local setup

```bash
npm install
npm run build
cargo test --workspace
```

For desktop development:

```bash
npm run tauri:dev
```

## Code standards

- Keep platform-specific code behind `cfg(target_os = "...")`.
- Keep frontend controls real and stateful; avoid inert mock buttons unless clearly marked.
- Prefer small Rust crates with explicit ownership over a single large backend module.
- Do not add network relay behavior without an explicit security review.
- Keep user-facing copy direct and understandable.

## Pull requests

Good PRs should include:

- what changed
- what was tested
- platform impact
- any permission or driver implications

## Current high-value areas

- mDNS discovery implementation
- QUIC session bootstrap
- Windows DXGI capture prototype
- macOS ScreenCaptureKit prototype
- audio capture experiments
- driver research for virtual displays and virtual audio
