# Release and Auto-Update

PaneLink uses GitHub Releases plus the Tauri updater.

## User flow

1. Open the latest GitHub Release.
2. Download the Windows installer or macOS bundle.
3. Install and open PaneLink.
4. Future signed releases are discovered through `latest.json` in GitHub Releases.

## Maintainer flow

Create a version tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds Windows and macOS artifacts with `tauri-apps/tauri-action`.

## Required GitHub secrets

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Without these secrets, release artifacts may build but auto-update signatures will not be usable.

## macOS note

The workflow uses ad-hoc signing for early open-source builds. For a truly smooth public macOS install/update experience, PaneLink will need Apple Developer ID signing and notarization.
