# Architecture

```
DualShock ──BT──▶ 8BitDo adapter ──USB──▶ Shield
                                           │  usbdevfs fd (granted via UsbManager, no root)
                                           ▼
                                   Rust core (libusbipcore.so)
                                   one thread, poll() loop
                                           │  TCP 3240, USB/IP 1.1.1
                                           ▼
                           Windows PC: usbip-win2 (signed UDE driver)
                                           ▼
                           xusb22 / HID driver → games, via Sunshine as usual
Moonlight (on the Shield) keeps doing video and audio, and sees no controller.
```

## Components

| Part | Language | Responsibility |
|---|---|---|
| `MainActivity`, `SettingsActivity`, `ProbeActivity` | Kotlin | D-pad UI: select device, permission, start/stop, status, probe |
| `BridgeService` | Kotlin | Foreground service. Opens the device, `claimInterface(force=true)` on every interface (detaches Android's gamepad driver), wake lock and low-latency Wi-Fi lock, attach/detach handling, status polling |
| `UsbAttachActivity` | Kotlin | Target of `USB_DEVICE_ATTACHED`, so "always open" re-grants permission after re-enumeration |
| `NativeBridge` / `jni_bridge.rs` | Kotlin/Rust | JNI: start(fd, descriptors, …), stop, status |
| `server.rs` | Rust | Linux driver: `poll()` over wake pipe, listener, USB fd, client sockets |
| `engine.rs` | Rust | Connection manager: admission (ACL), single importer, same-PC eviction, completion routing, output queues, turnaround metric |
| `session.rs` | Rust | Per-connection USB/IP state machine (handshake → URB phase), control-request intercepts, UNLINK semantics |
| `usbfs.rs` | Rust | Async URBs: `SUBMITURB` / `REAPURBNDELAY` / `DISCARDURB`, reusable slots |
| `proto.rs` | Rust | Wire codec, incremental parser |
| `acl.rs`, `stats.rs`, `descriptors.rs` | Rust | Local-network policy, metrics, descriptor parsing |

## Input path (one interrupt report)

1. usbip-win2 keeps interrupt-IN `CMD_SUBMIT`s outstanding. Each one becomes a usbdevfs URB at
   once, so the kernel already has a transfer queued when the adapter produces a report.
2. When the URB completes, the usbdevfs fd becomes `POLLOUT`-ready. `poll()` wakes the server
   thread, which reaps every completed URB (non-blocking) and encodes each `RET_SUBMIT` into the
   client's output buffer.
3. In the same loop iteration the buffer is written to the TCP socket (`TCP_NODELAY`).

There are no inter-thread queues, no sleeps and no timed polling on this path. The 250 ms
`poll()` timeout only refreshes the status snapshot. Steady state allocates nothing per transfer:
URB slots, their buffers and the output buffers are reused. A test checks the output/rx
capacity does not grow.

## Protocol details that matter for compatibility

- `seqnum` is echoed unchanged; usbip-win2 encodes direction in its low bit.
- `RET_SUBMIT.number_of_packets = -1` (non-isochronous), as usbip-win2 validates.
- UNLINK: if the URB is still in flight it is discarded, and when the kernel returns it a
  `RET_UNLINK` with `-ECONNRESET` is sent and **no** `RET_SUBMIT`. If it already completed, the
  reply is `RET_UNLINK` with status 0 (Linux stub semantics).
- `SET_CONFIGURATION` is acknowledged without reconfiguring (the configuration is fixed while we
  hold the interfaces). `SET_INTERFACE` → `USBDEVFS_SETINTERFACE`,
  `CLEAR_FEATURE(ENDPOINT_HALT)` → `USBDEVFS_CLEAR_HALT`. Hub `SET_FEATURE(PORT_RESET)` is
  acknowledged without resetting, because a real reset could make Android re-bind its own driver.
- Isochronous transfers are rejected with `-EXDEV` (out of scope).
- Busid is always `1-1`, so the PC's automatic re-attach keeps working after re-enumeration.

## Lifecycle

| Event | Handling |
|---|---|
| CMD_UNLINK | discard → reply after reap (see above) |
| Client closes / resets / keepalive timeout (~16 s) | all its URBs discarded; their late completions are dropped by generation tag |
| Same PC re-imports while an old session lingers | old session evicted, new one accepted |
| Different PC imports while attached | `ST_DEV_BUSY` |
| Device unplug / re-enumeration | `POLLHUP`/`-ENODEV` → server stops, service waits for `USB_DEVICE_ATTACHED` of the same VID:PID and restarts |
| Stop | wake pipe → discard all → reap (bounded 1 s) → join → release interfaces → close fd |

Backend tags are `generation << 32 | seqnum`, so a late completion from a previous session can
never be mistaken for a new session's transfer with the same seqnum.

## Security

- Sharing is off by default and only runs between explicit Start and Stop.
- Only the selected device is exported. Only local-network peers are accepted (RFC 1918, link-local,
  loopback, IPv6 ULA/link-local); optionally a single PC IP. At most 4 connections, 1 importer.
- No UPnP / port mapping. Reply queues are capped (1 MiB) and transfers capped (1 MiB).
