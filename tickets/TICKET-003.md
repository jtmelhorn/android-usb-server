# TICKET-003: Licensing and existing-code assessment

- **Status:** done
- **Milestone:** 1
- **Priority:** P0
- **Dependencies:** 001

## Problem
The project must be free and open source, reuse suitable code with correct attribution, never use
VirtualHere code, and rely on the existing usbip-win2 client without a new Windows kernel driver.

## Intended outcome
A documented assessment of cgutman/USBIPServerForAndroid and vadimgrn/usbip-win2, a chosen license,
and `LICENSE` + `NOTICE.md` in the repository.

## Scope
- License review of both projects.
- Protocol compatibility review of usbip-win2 as a client (what it sends and what it validates).
- Decide what to reuse.

## Exclusions
- Any VirtualHere binaries, protocol reverse engineering, or code.

## Acceptance criteria
1. License of each reviewed project recorded; project license chosen and compatible.
2. `LICENSE` (full text) and `NOTICE.md` (attributions, references) committed.
3. usbip-win2 client behaviours relevant to the server are listed with source references.
4. Statement on whether a new Windows driver is needed, with reasoning.

## Validation plan
- Source review of cloned repositories (commit dates recorded).
- Later confirmed empirically by TICKET-010.

## Required evidence
- The findings below.

## Implementation notes / affected files
Reviewed 2026-10-24 (shallow clones):
- **cgutman/USBIPServerForAndroid** — GPL-3.0. Java. Server for unrooted Android using
  `UsbDeviceConnection`. Design: synchronous `bulkTransfer`/`controlTransfer` with a thread pool per
  URB; UNLINK cannot cancel an in-flight Java transfer. Useful as a reference for OP_REQ_DEVLIST /
  OP_REQ_IMPORT handling and for the special-casing of SET_CONFIGURATION / SET_INTERFACE /
  CLEAR_FEATURE(HALT). **No source copied**; our implementation is new Rust/Kotlin written against the
  kernel protocol documentation. Credited in NOTICE as a design reference.
- **vadimgrn/usbip-win2** — **BSD-2-Clause** (`LICENSE.txt`; corrected from an earlier assumption of GPL-3.0). Windows UDE client with signed (WHLK/attestation) release
  drivers. Used unmodified as the Windows client. Findings from `drivers/ude`:
  - Requires server protocol version 1.1.1 (`0x0111`).
  - `wsk_receive.cpp: validate()` accepts only RET_SUBMIT / RET_UNLINK; rejects `actual_length < 0`;
    requires `number_of_packets` = `0xFFFFFFFF` (non-isoch) or a valid count; derives direction from the
    low bit of the echoed seqnum, so servers must echo `seqnum` exactly; direction field may be 0.
  - RET_UNLINK semantics: `-ECONNRESET` if the URB was unlinked, `0` if CMD_UNLINK arrived after
    RET_SUBMIT (matches kernel `usbip_protocol.rst`).
  - Sends SET_CONFIGURATION, SET_INTERFACE, CLEAR_FEATURE(ENDPOINT_HALT) and hub-class
    SET_FEATURE(PORT_RESET) (`bmRequestType 0x23`) as ordinary EP0 CMD_SUBMITs
    (`device_ioctl.cpp`). The server must intercept these like the Linux stub
    (`stub_rx.c: tweak_special_requests`).
  - Automatic re-attach: when the TCP connection drops, the driver itself schedules re-attach
    attempts (`wsk_receive_events.cpp: async_reattach` → `persistent.cpp`), first after
    `ReattachFirstDelay` (INF default 30 s, min 1 s), backing off to `ReattachMaxDelay` (480 s), up to
    `ReattachMaxAttempts` (20). `usbip attach -x` stops attempts, `--once` disables them → TICKET-013.
- **Project license: GPL-3.0-or-later** — compatible with both references (GPL-3.0 and BSD-2-Clause) and keeps the option of
  reusing GPL-3.0 code later.
- **Windows driver:** none needed. usbip-win2 release builds ship signed drivers; no test-signing mode.

Files: `LICENSE`, `NOTICE.md`.

## Remaining uncertainties / blockers
- Whether Windows' `xusb22.sys` initialises the forwarded 8BitDo (XInput) over UDE without quirks —
  empirical, TICKET-010.

## Evidence
- `LICENSE` is the unmodified GPLv3 text; `NOTICE.md` lists design references and every shipped
  third-party crate with its license (all MIT / Apache-2.0 / Unlicense — GPL-compatible), from
  `cargo tree --target aarch64-linux-android -e normal`.
