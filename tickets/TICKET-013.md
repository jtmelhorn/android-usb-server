# TICKET-013: Windows automatic re-attach (usbip-win2 built-in)

- **Status:** in_progress
- **Milestone:** 7
- **Priority:** P2
- **Dependencies:** 010

## Problem
USB/IP is client-initiated. After a Shield-side disconnect (adapter re-enumeration, Wi-Fi drop,
sharing restarted) the PC must attach again. The original plan assumed this needed a helper script.

## Intended outcome
Investigation (TICKET-003) showed usbip-win2 already re-attaches automatically after a dropped
connection. Outcome: rely on that built-in behaviour (no extra script), keep the server's busid stable
(`1-1`) so retries match after re-enumeration, and document how to shorten the first retry delay.

## Scope
- Stable busid in the server; same-PC re-import evicts a stale session (engine).
- `docs/windows-setup.md` section on `ReattachFirstDelay` / `ReattachMaxDelay` / `ReattachMaxAttempts`
  and `usbip attach -x`.
- The originally planned PowerShell polling script is dropped: it would duplicate the driver's
  retries and race with them.

## Exclusions
- Any Windows service/driver installation.

## Acceptance criteria
1. Server busid constant across restarts (code: `jni_bridge.rs BUSID`).
2. **[HW]** After toggling sharing off/on on the Shield, the controller returns on Windows without
   manual action, within the configured first delay + attach time (measure with default 30 s and with
   `ReattachFirstDelay=2`).

## Validation plan
- HW: toggle sharing 5× for each delay setting, record re-attach times.

## Required evidence
- HW timing notes.

## Implementation notes / affected files
- `docs/windows-setup.md`, `core/src/jni_bridge.rs`, `core/src/engine.rs`.
- Source: `usbip-win2/drivers/ude/usbip2_ude.inf` lines 78–80, `vhci.cpp init_constants`,
  `wsk_receive_events.cpp async_reattach`.

## Remaining uncertainties / blockers
- Registry location of the driver `Parameters` key inferred from the INF (`HKR, Parameters`);
  confirm on the installed system.
- Blocked on TICKET-010 for hardware validation.
