# Enma Patcher Desktop

Desktop app (Tauri 2 + React) to patch **your own copy** of Yo-kai Watch 1
Smartphone with translation mods (Spanish) and export it ready to install:
signed `.apk` on Android and sideload-ready `.ipa` on iOS. It also ships a
**CLI** with the same features.

> Not affiliated with Level-5. You need your own copy of the game: this repo
> contains no game files, APKs, IPAs, `.drmb` files or certificates.

## Features

- **Android**: patches a full `.apks` (base + splits + asset pack) or a lone
  `.apk` with the mod files, applies smali bypass via apktool, merges splits
  into a single APK, strips split requirements from the manifest and signs
  (v1/v2/v3).
- **iOS**: patches a decrypted `.ipa` (`assets/` → `data/` mapping),
  preserves the executable bit, cleans stale signatures, removes the
  `UISupportedDevices` restriction and can **sign with zsign** using your identity.
- **Multiple Apple accounts**: store several Apple IDs and pick which one to
  use when exporting/installing; each account can hold its signing identity
  (`.p12` + `.mobileprovision`).
- **Self-installing tools**: apktool, uber-apk-signer, platform-tools (adb),
  Temurin JRE and zsign download themselves from the Tools tab.
- **CLI**: `patch-android`, `patch-ios`, `sign-ios`, inspection, adb install
  and tool status.

## Requirements

- Your `.apks` (ideally exported with SAI or Swift Backup: base + splits +
  asset pack), your own `.drmb` for the bypass, and the mod (GitHub repo or
  ZIP with `enmapatcher.cfg.json`).
- For iOS: a decrypted `.ipa` and, to install, an Apple account with a
  development certificate (free ones revoke every 7 days).
- No need to install Java: the app downloads its own JRE if none is found.

## Quick start (GUI)

**Android**: Android tab → pick `.apks` → pick `.drmb` → (optional) display
name → Patch → Export/Install via adb.

**iOS**: iOS tab → pick `.ipa` → Patch → pick account → *Sign with zsign*
(requires an identity imported under Accounts) → Export or *Try installing*
(pymobiledevice3/ideviceinstaller if available). Without an identity, export
the unsigned `.ipa` for Sideloadly/AltStore/Scarlet.

**Display name**: when the field is empty, the mod's `appName` from
`enmapatcher.cfg.json` is used.

## CLI

```bash
# Inspect
enma-patcher-desktop inspect-apks game.apks
enma-patcher-desktop inspect-drmb mine.drmb
enma-patcher-desktop inspect-ipa game.ipa

# Patch Android (use D: as scratch for big merges)
enma-patcher-desktop patch-android --apks game.apks --drmb mine.drmb \
  --zip mods.zip --out game_patched.apk --work-dir D:\work

# Patch iOS
enma-patcher-desktop patch-ios --ipa game.ipa --zip mods.zip

# Sign an IPA with an account's identity
enma-patcher-desktop sign-ios --ipa game_patched.ipa --account acc-20240101

# Tools and adb
enma-patcher-desktop tool-status
enma-patcher-desktop download-tool adb
enma-patcher-desktop android-install game_patched.apk
```

## `enmapatcher.cfg.json` (for mod authors)

```json
{
  "appName": "Yo-kai Watch 1",
  "renameAssets": true,
  "exclude": ["*.txt", "*.cfg", "LICENSE", "README.md", "*.png", "img/"],
  "exclude_android": [],
  "exclude_ios": ["assets/android/**"],
  "include_android": [],
  "include_ios": [],
  "platforms": { "Android": true, "iOS": true },
  "recommended_version": "1.0.13",
  "tested_versions": ["1.0.11", "1.0.13"],
  "incompatible_versions": [],
  "ai_content": false,
  "License": "MIT License"
}
```

- `appName`: visible name the app gives the game (editable in the UI).
- `renameAssets` (default `true`): on iOS maps `assets/X` → `data/X`.
  Set it to `false` for exact paths.
- `include/exclude` (plus `_android`/`_ios` variants): `**/*.txt`-style
  globs; with no `include`, everything is in except exclusions. On iOS,
  `assets/android/**` (Android-only audio) is excluded by default.
- `platforms`: restrict a mod to one platform.
- `tested_versions` only warns when your game is another version.

## Signing

- **Android**: uber-apk-signer with its own keystore (zipalign included —
  without it the APK won't install on Android 11+). If signing fails, the
  patch errors out instead of handing out a broken APK.
- **iOS**: zsign v1.1.2 with your `.p12` + `.mobileprovision` (import them
  under Accounts). Free certificates expire every 7 days.

## Common issues

| Symptom | Likely cause |
|---|---|
| `INSTALL_FAILED_MISSING_SPLIT` | Unmerged APK or split leftovers in the manifest (the app strips them) |
| Won't install on Android 11+ | Misaligned `resources.arsc`: only install app-signed outputs |
| SideStore dies installing (iOS) | ~2 GB IPA + on-device re-sign exceeds the write budget; use PC-based Sideloadly/AltStore |
| Sideloadly "invalid file" | C: drive full (needs GBs of temp) or outdated Sideloadly |
| `uber-apk-signer failed ... zipalign` | Full disk or antivirus blocking the freshly extracted exe |
| "not available on this device" (iOS) | Stale `UISupportedDevices` (the app removes it when patching) |
| Still in Japanese | The mod replaces `_ja` text: play with the language set to Japanese |

## Development

```bash
bun install        # frontend deps
bun run tauri dev  # dev app
bun run build      # frontend only (tsc + vite)

cd src-tauri
cargo test         # tests (a few need sample fixtures)
cargo fmt --check
cargo clippy       # advisory (some pre-existing lints remain)
```

Layout: `src/` (React), `src-tauri/src/main.rs` (whole backend: Android/iOS
patching, signing, accounts, tools, CLI), `src/i18n/locales/` (6 languages).

Linux prerequisites (Debian/Ubuntu) for `tauri dev` / `tauri build`:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## CI and releases

Every push/PR builds on GitHub Actions (Windows + macOS + Linux). To
publish: push a `vX.Y.Z` tag and CI attaches the installers (MSI, NSIS .exe,
standalone `.exe`, DMG, deb, AppImage) to an automatic draft Release. See
`.github/workflows/build.yml`.

## Security

Nothing leaves your machine except tool and mod downloads. Passwords are
stored obfuscated and `.p12`/`.mobileprovision` files stay in local data.
Never commit to the repo: `*.p12`, `*.mobileprovision`, `*.keystore`,
accounts or game files (see `.gitignore`).

## License

MIT — see `LICENSE`.
