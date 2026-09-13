# Changelog

All notable changes to Enma Patcher Desktop are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.0] - 2026-09-13

### Added

- Android patching: full `.apks` (base + splits + asset pack) merged into a
  single signed `.apk`, with apktool smali bypass and manifest split defuse.
- iOS patching: decrypted `.ipa` support with `assets/` to `data/` mapping,
  executable-bit preservation, stale signature cleanup and
  `UISupportedDevices` removal.
- iOS signing with zsign using per-account `.p12` + `.mobileprovision`
  identities, plus a `sign-ios` command.
- Multiple Apple accounts with picker at export/install time.
- CLI covering every GUI action: patch, sign, inspect, tools, adb install.
- Display name for the installed game, from the mod's `appName` or override.
- Patching without mods (bypass/cleanup only) behind a confirm dialog.
- Smali risk dialog: 5-second hold-to-accept when mods contain smali code.
- IPS binary patches via mod `patches/` folders, with skip warnings.
- Nested mods via `mods/mods.json` (recursive, depth 3, cycle-safe).
- `console` tag for Switch/3DS mod identification and `enmaignore` mod blocking.
- `enmapatcher.cfg.json`: `renameAssets`, platform include/exclude filters,
  version warnings.
- Tools self-install: apktool, uber-apk-signer, adb, Temurin JRE, zsign.
- Auto-updater: release notification banner, one-click install with progress,
  signed artifacts and generated `latest.json` in CI.
- Interface: first-run intro, subtle sounds, animated tabs, custom logo,
  6 languages (English default), dark/light themes.
- GitHub Actions CI (Windows, macOS, Linux) with draft releases on tags,
  project wiki and this changelog.

### Changed

- Signing is mandatory and loud: uber-apk-signer with persistent zipalign,
  no more v1-only or unaligned fallback builds.
- Merge copies entries byte-for-byte instead of recompressing ~2 GB.
- uber-apk-signer works from the work directory, keeping outputs clean.
- Release binaries ship without a console window; CLI output still works.

### Fixed

- `INSTALL_FAILED_MISSING_SPLIT` via structural manifest defuse.
- Unaligned `resources.arsc` installs failing on Android 11+.
- iOS executable bit loss and Android-only files leaking into IPAs.
- `split/` prefixes leaking into merged APK paths.
- Disk-full and antivirus failures now report actionable errors.
- CRLF/LF normalization via `.gitattributes`.

## [0.1.0] - 2026-09-06

- Initial Tauri 2 + React scaffold.
