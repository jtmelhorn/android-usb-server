# TICKET-017: [HW] Sustained-operation and reconnection soak

- **Status:** blocked
- **Milestone:** 11
- **Priority:** P1
- **Dependencies:** 011, 012

## Problem
Short tests miss leaks, slow drift and rare lifecycle races.

## Intended outcome
Evidence of stable multi-hour forwarding and repeated recovery.

## Scope
- 2-hour continuous session (gameplay or tester running).
- 20 forced disconnects across types: controller off/on, adapter unplug/replug, Wi-Fi off/on on the
  Shield, `usbip detach`/attach, sharing stop/start.

## Exclusions
- Multi-day tests.

## Acceptance criteria
1. 2 h with no session drop; error counters stay 0 (or each error explained).
2. Shield app memory (`dumpsys meminfo <pkg>`) at end within +10 MB of start.
3. All 20 disconnects recover (with auto-reattach helper on Windows); time-to-recover recorded.

## Validation plan
- Procedure above; record in `evidence/017-soak.md`.

## Required evidence
- Soak log, meminfo before/after, recovery table.

## Implementation notes / affected files
- n/a.

## Remaining uncertainties / blockers
- Blocked by hardware tickets.
