# USB/IP Gamepad Bridge for Android TV

A free, open-source (GPL-3.0-or-later) USB/IP **server** for Android, built to forward a game
controller's USB adapter from an NVIDIA Shield to a Windows PC over the home network:

```
DualShock ─BT─▶ 8BitDo USB Wireless Adapter 2 ─USB─▶ Shield (this app) ─LAN─▶ Windows PC (usbip-win2)
```

Moonlight keeps streaming video and audio. The controller's USB traffic bypasses Android's gamepad
stack and Moonlight's controller forwarding, and reaches the PC as a real USB device.

- No root. It uses Android's `UsbManager` permission and the device fd it grants.
- Standard USB/IP 1.1.1, so it works with the **signed** [usbip-win2](https://github.com/vadimgrn/usbip-win2)
  client. No custom Windows driver, no test signing.
- No accounts, subscriptions, license servers or hardware binding.
- Android TV UI usable with the Shield remote.

## Status

Honest summary. Details and evidence are in [`tickets/`](tickets/README.md).

| Area | State |
|---|---|
| Rust USB/IP core: protocol, sessions, unlink, lifecycle, ACL, metrics | Implemented. 62 host tests pass. 64 tests pass **on the Shield** (arm64 and armv7), including the real-socket server test |
| Android app: service, UI, probe | Implemented. Installs and runs on SHIELD (Android 11) |
| Shield can claim the 8BitDo adapter's interfaces (milestone 1) | **Verified on hardware**: all 4 interfaces force-claimed without root; server forwards real control/interrupt/unlink transfers to a LAN client (TICKET-004, 007, 008) |
| Adapter input reports on the Shield | **Not yet seen**: in XInput mode the adapter sent no reports, also not to Android's own driver; Windows-side test pending (TICKET-024) |
| Windows enumeration and controller input via usbip-win2 | **Not yet verified** (TICKET-010) |
| Moonlight background operation, soak, reconnection on hardware | **Not yet verified** (TICKET-011/012/017) |
| Latency vs. normal Shield/Moonlight handling | **Not measured.** No improvement is claimed (TICKET-018) |
| Rumble | Forwarded generically; unverified (TICKET-021) |

## Quick start

1. **Shield:** install the APK ([docs/shield-setup.md](docs/shield-setup.md)), plug in the
   adapter, select it, grant permission, then **Start sharing**.
2. **PC:** install the signed usbip-win2 release ([docs/windows-setup.md](docs/windows-setup.md)),
   then run:

   ```bash
   usbip attach -r <shield-ip> -b 1-1
   ```

3. Check the controller in `joy.cpl` or any gamepad tester, then start Moonlight as usual.

## Safety

USB/IP has no authentication or encryption. The app:

- shares only between an explicit Start and Stop;
- exports only the selected device;
- accepts only local-network clients, optionally a single PC IP.

**Never port-forward TCP 3240.**

## Measurements, not promises

The app shows **report interval** (how often the adapter delivers reports; *not* latency) and
**server turnaround** (time from completion to the socket). Network RTT and button-to-screen latency
are measured externally, as described in [docs/latency-method.md](docs/latency-method.md).

## Building from source

Prerequisites:

- JDK 17
- Android SDK with platform 34, build-tools 34.0.0 and NDK `27.2.12479018`
- Rust via [rustup](https://rustup.rs), with targets `aarch64-linux-android` and
  `armv7-linux-androideabi` (pinned in `core/rust-toolchain.toml`)

Set `ANDROID_HOME` (or `sdk.dir` in `android/local.properties`) and `JAVA_HOME`, then run one of:

```bash
pwsh tools/build.ps1
```

```bash
tools/build.sh
```

This runs the Rust tests, cross-compiles `libusbipcore.so` (Gradle task `cargoBuild`), builds the
APK and writes `dist/usbip-bridge-<version>-debug.apk` and `dist/SHA256SUMS`. For a release-signed
APK, set `USBIP_KEYSTORE`, `USBIP_KEYSTORE_PASSWORD`, `USBIP_KEY_ALIAS` and `USBIP_KEY_PASSWORD`,
then pass `-Release` / `--release`.

## Layout

```
core/       Rust USB/IP server core (protocol, sessions, usbdevfs backend, JNI)
android/    Kotlin Android TV app (Gradle)
docs/       Architecture, setup, testing and latency method
tickets/    Repository-local ticket system: backlog, status, evidence
evidence/   Logs, screenshots and measurements from real hardware
tools/      Build scripts, ticket checker
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md) for attributions.
VirtualHere is a trademark of its owner. This project contains no VirtualHere code and is not
affiliated with it.
