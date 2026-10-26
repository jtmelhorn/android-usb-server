# TICKET-008: USB/IP server session and event loop

- **Status:** done
- **Milestone:** 4
- **Priority:** P0
- **Dependencies:** 006, 007

## Problem
Requests from the Windows client must be turned into USB transfers and completions turned back into
replies with minimal latency, correct ordering, and correct semantics for special requests.

## Intended outcome
`core/src/server.rs` + `core/src/session.rs`: a TCP listener (default port 3240) and a
single-threaded `poll()` loop over {client socket, usb fd, wake eventfd/pipe}.

## Scope
- OP_REQ_DEVLIST → exactly one exported device (the selected one).
- OP_REQ_IMPORT → only the configured busid; one client at a time; others get status 1 / busy.
- CMD_SUBMIT: ep0 control, interrupt, bulk. Intercepts (like Linux `stub_rx.c`):
  - SET_CONFIGURATION: succeed without touching the device if value equals active config.
  - SET_INTERFACE: `USBDEVFS_SETINTERFACE`.
  - CLEAR_FEATURE(ENDPOINT_HALT): `USBDEVFS_CLEAR_HALT`.
  - Hub SET_FEATURE(PORT_RESET) (`bmRequestType 0x23`): acknowledged without resetting (a real reset
    could make Android re-bind `xpad`); logged.
- Isochronous or unknown endpoint → RET_SUBMIT with `-EINVAL`/`-EXDEV`; no crash.
- CMD_UNLINK: discard pending URB; reply `-ECONNRESET` after the kernel returns it and suppress its
  RET_SUBMIT; if not pending, reply `0`.
- Non-blocking socket with an output queue; `TCP_NODELAY`; replies written as soon as reaped.

## Exclusions
- Multiple simultaneous clients/devices.

## Acceptance criteria
1. Host tests with a mock USB backend cover: devlist, import (good / wrong busid / second client),
   control IN/OUT, interrupt IN/OUT, SET_CONFIGURATION/SET_INTERFACE/CLEAR_HALT/PORT_RESET intercepts,
   unlink of pending (→ -ECONNRESET, no RET_SUBMIT), unlink after completion (→ 0), malformed client
   data (→ session closed, pending URBs discarded), device disappearing (→ session closed).
2. No heap allocation per interrupt transfer in steady state (buffers reused; verified by code review
   and a counting-allocator test on the hot path).
3. **[HW]** proven with usbip-win2 in TICKET-010.

## Validation plan
- `cargo test`; HW via TICKET-010.

## Required evidence
- Test output; TICKET-010 evidence.

## Implementation notes / affected files
- `core/src/session.rs`, `core/src/server.rs`, `core/src/backend.rs` (trait + mock).

## Remaining uncertainties / blockers
- Interop with Windows' `xusb22` driver unverified until TICKET-010.

## Evidence so far
- Criterion 1: `session::tests` (24 tests) and `engine::tests` (13 tests) pass on host and Shield.
- Criterion 2: `steady_state_interrupt_loop_reuses_capacity` (2000 cycles, no growth of output or
  receive buffers); usbfs slots/buffers reused by design (`usbfs.rs`).
- Socket-level driver test `server::tests::devlist_import_stream_unlink_reconnect_stop` passes **on the
  Shield** (500 interrupt reports over TCP, unlink → -ECONNRESET, client vanish + reconnect, stop
  while attached < 2 s).
- Criterion 3 (usbip-win2 interop) pending TICKET-010.

## Hardware evidence (2026-10-26)
- `tools/usbip-client-test.py` from the PC (192.168.15.133) against the Shield server with the real
  adapter: DEVLIST (17.9 ms), IMPORT, device/config/string descriptors, SET_CONFIGURATION intercept,
  vendor request, interrupt OUT, interrupt IN queueing, UNLINK — all as specified
  (`evidence/005-client-test.txt`). Server log: `client 192.168.15.133 attached 1-1` …
  `detached` … `discarded 3 pending transfer(s)`.

## Criterion 3 (2026-10-26)
PASS — interoperates with usbip-win2 0.9.8.1 and Windows `xusb22` (TICKET-010).
