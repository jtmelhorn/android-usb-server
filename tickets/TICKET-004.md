# TICKET-004: [HW] Shield USB access feasibility with the 8BitDo adapter

- **Status:** blocked
- **Milestone:** 2 — Shield USB access feasibility
- **Priority:** P0
- **Dependencies:** 002
- **Gates:** 009 (forwarding architecture), 014 (UI beyond basic controls)

## Problem
The whole design depends on an unrooted Shield letting an ordinary app open the 8BitDo adapter,
detach the kernel gamepad driver (`xpad` / HID) from its interfaces, and read its interrupt endpoint.
If Android forbids this, the architecture must change before more code is written.

## Intended outcome
Hardware evidence that, on the Shield's actual Android build and without root, the app can:
enumerate the adapter, obtain user permission, `claimInterface(force=true)` every interface,
read interrupt-IN reports while buttons are pressed, and that Android's own gamepad input stops
receiving those events while claimed.

## Scope
- The app's **Probe** screen (built into the APK): lists devices, requests permission, dumps raw
  descriptors, claims all interfaces with force, captures interrupt-IN reports for 10 s and shows
  count / interval statistics / last report hex. Results also go to logcat tag `UsbIpProbe`.
- Record Shield model, Android version, build fingerprint, ABI list, kernel version.

## Exclusions
- USB/IP networking (TICKET-008/010).

## Acceptance criteria
1. `getprop` facts recorded (model, `ro.build.version.release`, `ro.build.fingerprint`,
   `ro.product.cpu.abilist`, `uname -r`).
2. Adapter appears in the device list with VID:PID and interface table.
3. Permission dialog appears and is granted with the Shield remote.
4. `claimInterface(force=true)` returns true for every interface (logged per interface).
5. ≥100 interrupt-IN reports captured while pressing buttons, with changing payload bytes.
6. While claimed, a gamepad tester on the Shield (or Moonlight) no longer sees the controller;
   after release, it does again.
7. If any step fails: exact API, return value / exception, and logcat lines recorded, and the
   limitation explained before TICKET-009 proceeds further.

## Validation plan
Physical procedure (needs the user):
1. Enable Developer options + Network debugging on the Shield; `adb connect <shield-ip>:5555`.
2. `adb install -r app-debug.apk`; open **USB/IP Gamepad Bridge** → **Probe**.
3. Select the 8BitDo adapter → *Grant permission* → *Run probe*; press buttons/sticks for 10 s.
4. `adb logcat -d -s UsbIpProbe > evidence/004-probe.log`.

## Required evidence
- `evidence/004-device-props.txt`, `evidence/004-probe.log`, notes for criterion 6.

## Implementation notes / affected files
- `android/app/src/main/java/.../ProbeActivity.kt`, `UsbProbe.kt`.
- Expectation (unverified): in XInput mode the adapter enumerates as an Xbox 360-class device
  (interface class `0xFF`, subclass `0x5D`) bound to the kernel `xpad` driver; `claimInterface(force)`
  issues `USBDEVFS_DISCONNECT` + claim, which Android permits for apps holding device permission.
  Android 11+ blocks nothing here except devices with audio/MTP handled by system services.

## Remaining uncertainties / blockers
- **BLOCKED on physical action:** plug the 8BitDo USB Wireless Adapter 2 into the Shield with the
  controller connected (XInput mode if selectable). As of 2026-10-25 02:25 UTC the Shield showed only
  its three root hubs (`/dev/bus/usb/00{1,2,3}/001`).
- Whether the 8BitDo adapter's XInput mode is selectable when connected to the Shield (it may
  auto-select a mode based on host OS detection); the Probe records the VID:PID actually presented.

## Progress / evidence so far
- Shield reachable over network adb at `192.168.15.116:5555` (found via `adb mdns services`, already
  authorized). Criterion 1 done: `evidence/004-device-props.txt` — SHIELD Android TV (`mdarcy`),
  Android 11 / API 30, fingerprint `NVIDIA/mdarcy/mdarcy:11/RQ1A.210105.003/7825230_4387.0822`,
  ABIs `arm64-v8a,armeabi-v7a,armeabi`, kernel `4.9.141-tegra-gb6e5605a`.
- APK installed (`adb install -r` → Success) and launched; process running, no crash; native
  library loaded (`core: usbipcore 0.1.0` on screen) — `evidence/014-main-screen-no-device.png`.
- Criteria 2–7 pending the physical action above.
