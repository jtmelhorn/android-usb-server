# TICKET-012: Cancellation, disconnect, reconnection, re-enumeration

- **Status:** in_progress
- **Milestone:** 7
- **Priority:** P0
- **Dependencies:** 008, 009

## Problem
Controllers sleep, adapters re-enumerate (e.g. mode switches), Wi-Fi blips, and Windows cancels
transfers routinely. Any of these must not wedge the service or leak claimed interfaces.

## Intended outcome
Defined behaviour for every lifecycle event, implemented and tested.

| Event | Behaviour |
|---|---|
| CMD_UNLINK | discard URB; RET_UNLINK per spec (TICKET-008) |
| Client TCP close/reset | discard all pending URBs, reap them, return to listening |
| Malformed client data | close session as above, log reason |
| Device unplug / re-enumeration (fd `POLLHUP`, URB `-ENODEV`/`-ESHUTDOWN`) | close session, stop core, Kotlin waits for `USB_DEVICE_ATTACHED` of the same VID/PID(/serial) and restarts automatically if permission is still held |
| Stop requested | wake loop via eventfd/pipe, discard, close, join thread (bounded wait) |
| Network change (Wi-Fi reconnect / IP change) | listener remains bound to `0.0.0.0`; client re-attaches |
| Client silent for long time | TCP keepalive (idle 10 s, 3 probes) closes dead sessions |

## Scope
- Rust session/server lifecycle for every event in the table above.
- Kotlin attach/detach receivers and automatic restart for the selected device.

## Exclusions
- Server-initiated reconnect to Windows (USB/IP clients connect to servers) — handled by TICKET-013.

## Acceptance criteria
1. Host tests: unlink pending/completed, client disconnect with N pending URBs, malformed data,
   device-gone mid-session, stop while client connected, rapid reconnect ×100 — no leaks
   (pending map empty, slots returned), no panics, loop exits within 1 s on stop.
2. **[HW]** Unplug/replug adapter while attached: service recovers, Windows can re-attach.
3. **[HW]** Controller power-off/on while attached: input resumes without user action on the Shield.
4. **[HW]** Kill Windows client (`usbip detach`) and re-attach 10×: works each time.

## Validation plan
- `cargo test` (lifecycle tests); HW steps above recorded in TICKET-017.

## Required evidence
- Test output; HW logs.

## Implementation notes / affected files
- `core/src/session.rs`, `core/src/server.rs`, `BridgeService.kt` (attach/detach receivers).

## Remaining uncertainties / blockers
- HW criteria blocked by TICKET-004/010.

## Evidence so far
- Criterion 1 (host + Shield): `engine::tests::client_disconnect_with_pending_transfers_cleans_up`,
  `malformed_client_data_closes_session`, `device_gone_during_session`,
  `rapid_reconnect_100_times_no_leaks`, `shutdown_aborts_everything`,
  `same_machine_reimport_evicts_old_session_and_ignores_its_stale_transfers`,
  `session::tests::unlink_*`, and the on-Shield socket test (stop while attached returns < 2 s).
- Hardware criteria 2–4 blocked by TICKET-004/010.

## Hardware observations (2026-10-26)
- Adapter re-enumeration while shared (controller slept at 11:47:34): server stopped with
  `USB device error -19`, service logged `selected device detached`, and when the adapter returned at
  11:53:51 the service **restarted sharing automatically** (permission retained). Windows did not
  re-attach on its own within the observation window (default first retry 30 s with back-off) and was
  re-attached manually. Criterion 2 partially observed; 3 and 4 pending a dedicated run.
