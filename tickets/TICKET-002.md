# TICKET-002: Toolchain assessment and project skeleton

- **Status:** done
- **Milestone:** 1
- **Priority:** P0
- **Dependencies:** 001

## Problem
The workspace was empty. A reproducible Android + native build must exist before any USB code can
be tested on the Shield.

## Intended outcome
A Gradle Android project (Kotlin app shell) plus a Rust crate (USB/IP core) that cross-compiles for the
Shield's ABIs and is packaged into the APK by one command.

## Scope
- Inventory of installed tools (JDK, Android SDK/NDK, Gradle, Rust, adb).
- Architecture decision: Kotlin shell + Rust core (rationale recorded below).
- `android/` Gradle project, `core/` Rust crate, Gradle task that runs the Rust cross-build.

## Exclusions
- CI service setup (no paid accounts); release signing (TICKET-019).

## Acceptance criteria
1. Toolchain versions recorded.
2. `gradlew assembleDebug` (via `tools/build.ps1`) produces an APK containing `libusbipcore.so` for
   `arm64-v8a` and `armeabi-v7a`.
3. `cargo test` runs host-side unit tests on Windows.

## Validation plan
- Run `tools/build.ps1`; inspect APK with `unzip -l`.
- Run `cargo test` in `core/`.

## Required evidence
- Tool versions, build log tail, APK file listing.

## Implementation notes / affected files
Workspace inspection (2026-10-24): `C:\Users\jtmel\Documents\android-to-usb` empty, not a git repo.
System `java` is Oracle 1.8 32-bit (unsuitable). Found a prior kit in
`%USERPROFILE%\Downloads\android-build`:
- OpenJDK 17.0.20.1 (Microsoft build), Gradle 8.7, SDK platforms 30 & 34, build-tools 30.0.3 & 34.0.0,
  platform-tools (adb 37.0.1), cmdline-tools 12.0.
- Installed for this project: NDK 27.2.12479018 (via sdkmanager), Rust 1.99.1 via rustup with targets
  `aarch64-linux-android`, `armv7-linux-androideabi`, host `x86_64-pc-windows-gnu`.

Architecture decision (Kotlin + Rust):
- Unrooted USB access is only available through `android.hardware.usb.UsbManager` (Java API), so the
  permission flow, interface claiming, foreground service and UI are Kotlin.
- After permission, `UsbDeviceConnection.getFileDescriptor()` exposes the usbdevfs fd. The Rust core
  drives it with `USBDEVFS_SUBMITURB` / `REAPURBNDELAY` / `DISCARDURB` (the same approach libusb uses on
  Android), giving a GC-free single-threaded input path and a 1:1 mapping of USB/IP UNLINK to kernel
  URB discard. The protocol code is unit-testable on the PC.
- Language choice is not expected to dominate latency (network and video pipeline are larger); this is
  an engineering choice, not a performance claim (see TICKET-018).

Files: `android/**`, `core/**`, `tools/build.ps1`, `.gitignore`.

## Remaining uncertainties / blockers
- None. (Shield ABI list since confirmed; both 32- and 64-bit ARM libraries are shipped.)

## Evidence (2026-10-24)
- `cargo test` (host, x86_64-pc-windows-gnu): `62 passed; 0 failed`.
- `tools/build.ps1`: `BUILD SUCCESSFUL`, `dist/usbip-bridge-0.1.0-debug.apk`.
- APK listing: `lib/arm64-v8a/libusbipcore.so` (461 KB), `lib/armeabi-v7a/libusbipcore.so` (317 KB);
  `llvm-nm -D` shows the four `Java_io_github_jtmelhorn_usbipbridge_NativeBridge_*` exports; NEEDED
  only `liblog.so libdl.so libc.so`.
- Shield ABI list confirmed: `arm64-v8a,armeabi-v7a,armeabi` (`evidence/004-device-props.txt`).
