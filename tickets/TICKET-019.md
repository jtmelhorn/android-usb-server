# TICKET-019: Reproducible APK build and release packaging

- **Status:** done
- **Milestone:** 12
- **Priority:** P0
- **Dependencies:** 002

## Problem
Users need a sideloadable APK and anyone must be able to rebuild it from source with free tools.

## Intended outcome
`tools/build.ps1` (Windows) and `tools/build.sh` (Linux/macOS) that build the Rust core and the APK;
a release APK in `dist/` with SHA-256 checksum.

## Scope
- Pinned versions: Gradle wrapper 8.7, AGP, Kotlin, NDK 27.2.12479018, Rust toolchain via
  `rust-toolchain.toml`.
- Release signing: user-provided keystore via environment variables; without them the build produces
  a debug-signed APK (clearly named `-debug`). No keys committed.
- `minSdk` 26 (Shield runs Android 9/11), `targetSdk` 34.

## Exclusions
- Play Store publishing.

## Acceptance criteria
1. Clean checkout + documented prerequisites → `tools/build.ps1` produces `dist/usbip-bridge-<ver>-debug.apk`.
2. `apksigner verify` passes; `aapt dump badging` shows package, minSdk, leanback feature.
3. `dist/SHA256SUMS` generated.
4. APK installs on Shield via `adb install` (**[HW]**, part of TICKET-004).

## Validation plan
- Run build; run `apksigner verify --print-certs`; `aapt2 dump badging`.

## Required evidence
- Build log tail, verification output.

## Implementation notes / affected files
- `tools/build.ps1`, `tools/build.sh`, `android/gradle/wrapper/*`, `core/rust-toolchain.toml`.

## Remaining uncertainties / blockers
- Bit-for-bit reproducibility not guaranteed (timestamps in native libs); "reproducible" here means
  rebuildable from source with pinned tools.

## Evidence (2026-10-24)
- `tools/build.ps1` → tests pass, `BUILD SUCCESSFUL`, `dist/usbip-bridge-0.1.0-debug.apk`,
  `dist/SHA256SUMS`.
- `apksigner verify --print-certs` → v2 signature OK (Android Debug certificate).
- `aapt2 dump badging`: package `io.github.jtmelhorn.usbipbridge`, `sdkVersion:'26'`, target 34,
  `native-code: 'arm64-v8a' 'armeabi-v7a'`, leanback launchable, usb.host required.
- Criterion 4: `adb install -r` on the Shield → `Success`.
- Limitation: release signing path (env-provided keystore) is implemented but was not exercised (no
  keystore); the published APK is debug-signed and named `-debug`.
