# TICKET-009: Android integration — service, permission, interface claim, JNI

- **Status:** in_progress
- **Milestone:** 4
- **Priority:** P0
- **Dependencies:** 004 (architecture gate), 008

## Problem
The Rust core needs a device fd that only the Android framework can grant, must keep running while
Moonlight is in the foreground, and must stop cleanly.

## Intended outcome
`BridgeService` (foreground service) that opens the selected device, claims all interfaces with
`force=true` (detaching kernel gamepad drivers so Android/Moonlight no longer consume the input),
hands the fd and raw descriptors to the Rust core over JNI, and releases everything on stop.

## Scope
- Foreground service with persistent notification (Stop action); `foregroundServiceType=connectedDevice`.
- Partial wake lock + `WifiManager.WIFI_MODE_FULL_LOW_LATENCY` lock (API 29+) while sharing.
- JNI: `nativeStart(fd, rawDescriptors, busNum, devNum, busId, port, allowNonPrivate, allowedClient,
  verbose) → handle`, `nativeStop(handle)`, `nativeStatus(handle) → String (JSON)`.
- Only the selected device (matched by VID/PID/serial/path) is ever opened.

## Exclusions
- Auto-start at boot (not requested; would conflict with explicit start/stop).

## Acceptance criteria
1. Starting sharing shows a notification and a listening status with the Shield's LAN IP and port.
2. Stop (UI or notification) releases interfaces, closes the connection, stops the listener, and
   Android's gamepad handling sees the controller again (**[HW]**).
3. Native crash-safety: core panics are caught at the JNI boundary and reported as errors.
4. **[HW]** Service keeps forwarding with Moonlight in the foreground (TICKET-011).

## Validation plan
- Build; install on Shield; procedure in TICKET-010/011.

## Required evidence
- Screenshots or `dumpsys activity services` excerpt; logcat.

## Implementation notes / affected files
- `android/app/src/main/java/io/github/jtmelhorn/usbipbridge/{BridgeService,NativeBridge,UsbDevices}.kt`
- `core/src/jni_bridge.rs`.

## Remaining uncertainties / blockers
- Gate: TICKET-004 must confirm `claimInterface(force)` works on the Shield; code is written against
  the documented API meanwhile and kept small until then.

## Evidence so far
- Implemented: `BridgeService.kt`, `NativeBridge.kt`, `jni_bridge.rs`. App installs and runs on the
  Shield; JNI library loads (version string shown). Start/stop on hardware pending TICKET-004.

## Hardware evidence (2026-10-26)
- *Start sharing* on the Shield: service opened the adapter, claimed all interfaces, started the
  native server (`USB/IP server listening on port 3240`, `server thread nice set to -10`), served a
  LAN client, and *Stop sharing* released it. Moonlight/background behaviour pending TICKET-011.
