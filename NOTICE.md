# Notices and attributions

USB/IP Gamepad Bridge — Copyright (C) 2026 Justin Melhorn and contributors.

This program is free software: you can redistribute it and/or modify it under the terms of the GNU
General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. See [LICENSE](LICENSE).

## Design references (no code copied)

- **USB/IP protocol** — Linux kernel documentation, `Documentation/usb/usbip_protocol.rst`, and the
  behaviour of the Linux stub driver (`drivers/usb/usbip/stub_rx.c`, `stub_tx.c`), used as the
  specification for message layout, RET_UNLINK semantics and the special handling of
  SET_CONFIGURATION / SET_INTERFACE / CLEAR_FEATURE(ENDPOINT_HALT) / port reset.
- **cgutman/USBIPServerForAndroid** (GPL-3.0) — https://github.com/cgutman/USBIPServerForAndroid —
  prior art showing that a USB/IP server can run on unrooted Android with `UsbDeviceConnection`.
  Reviewed for design; this project's code is an independent implementation.
- **vadimgrn/usbip-win2** (BSD-2-Clause) — https://github.com/vadimgrn/usbip-win2 — the Windows
  USB/IP client this project targets, used unmodified. Its source was reviewed to confirm protocol
  expectations (seqnum direction bit, `number_of_packets = -1` for non-isochronous transfers,
  RET_UNLINK status values, automatic re-attach behaviour).
- **Linux `<linux/usbdevice_fs.h>`** — usbdevfs ioctl and `struct usbdevfs_urb` definitions.

No VirtualHere code, binaries or protocol material is used.

## Third-party components shipped in the APK

| Component | License | Use |
|---|---|---|
| Kotlin standard library (JetBrains) | Apache-2.0 | Android app runtime |
| `jni` 0.21 (jni-rs) | MIT OR Apache-2.0 | JNI bindings in the Rust core |
| `jni-sys` | MIT OR Apache-2.0 | JNI bindings |
| `libc` | MIT OR Apache-2.0 | Linux syscalls / ioctls |
| `cesu8` | MIT OR Apache-2.0 | dependency of `jni` |
| `combine` | MIT | dependency of `jni` |
| `bytes` | MIT | dependency of `combine` |
| `memchr` | Unlicense OR MIT | dependency of `combine` |
| `thiserror` | MIT OR Apache-2.0 | dependency of `jni` |
| `log` | MIT OR Apache-2.0 | dependency of `jni` |
| `cfg-if` | MIT OR Apache-2.0 | dependency of `jni` |

Build-time only (not shipped): Android Gradle Plugin, Android SDK/NDK, Rust toolchain, `syn`,
`quote`, `proc-macro2`, `unicode-ident` (MIT OR Apache-2.0).
