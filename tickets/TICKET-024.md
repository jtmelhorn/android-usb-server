# TICKET-024: [HW] Adapter sends no interrupt-IN reports on the Shield in XInput mode

- **Status:** blocked
- **Milestone:** 3
- **Priority:** P0
- **Dependencies:** 004
- **Gates:** 010 (input on Windows), 004 criterion 5

## Problem
With the DualShock paired (adapter and light bar both solid) and the user pressing buttons and moving
sticks, the adapter in XInput mode (`045e:028e`) produced **zero** interrupt-IN reports on the Shield:
- this app's probe (UsbRequest on `0x81`/`0x83`/`0x86`): 0 reports in four 10–30 s captures;
- Android's own kernel driver (`xpad`, `/dev/input/event6`, `getevent`): 0 events in 15 s;
- the USB/IP server driven from the PC (`tools/usbip-client-test.py`): 0 reports in 8 s (no one
  pressing in that run).
Sending the XInput init vendor request (`0xC1/0x01/0x0100`, reply `00 00`) and the player-1 LED
command (`01 03 06` on `0x02`, 3 bytes written) did not change this.

## Intended outcome
Know which host behaviour makes the adapter start reporting, and get reports flowing through the
forwarding path.

## Scope
Hypotheses, in order of test:
1. The adapter auto-detects the host and only reports to a real Windows XInput stack. Test: attach
   from Windows with usbip-win2 (TICKET-010) and watch the server's verbose URB log while pressing.
2. A different adapter mode (8BitDo's own `2dc8:3107`, or the DS4/Switch modes) reports on the Shield.
   Test: probe in each mode; record which modes Windows accepts.
3. A missing step of the Windows start-up sequence. Test: capture usbip-win2's request sequence
   (verbose log) and compare.

## Exclusions
- Firmware changes to the adapter.

## Acceptance criteria
1. Root cause identified with evidence, or a working mode found.
2. ≥100 interrupt-IN reports with changing payload pass through the server while buttons are pressed.

## Validation plan
- Hardware procedures above; record per-mode results.

## Required evidence
- `evidence/004-probe-input.log`, `evidence/004-getevent.log`, `evidence/005-client-test.txt`, plus
  logs from the Windows attach.

## Implementation notes / affected files
- Probe extended with the XInput init request and LED command (`UsbProbe.kt`).
- `tools/usbip-client-test.py` replicates host enumeration over USB/IP.
- Observed adapter re-enumerations: `2dc8:3107` at 11:11:55, `045e:028e` at 11:13:58, device
  number 015 → 017 by 11:32. Each re-enumeration ends a sharing session (handled by the service;
  Windows re-attach per TICKET-013).

## Remaining uncertainties / blockers
- **Blocked on:** the Windows-side test (usbip-win2 installed on the PC, user pressing buttons) to
  test hypothesis 1.
- This ticket was opened by the 2026-10-26 hardware session; the user previously reported the
  adapter worked on the Shield (with high latency), probably in a different mode.
