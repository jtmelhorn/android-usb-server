# TICKET-010: [HW] Windows USB/IP enumeration and controller input

- **Status:** done
- **Milestone:** 5
- **Priority:** P0
- **Dependencies:** 009
- **Gates:** 014 polish, 011, 018, 021

## Problem
Only real enumeration on Windows proves the server is compatible with usbip-win2 and the Xbox 360
class driver.

## Intended outcome
Windows lists the Shield's exported device, attaches it with the unmodified signed usbip-win2
release, Device Manager shows an Xbox 360-class controller, and a controller tester shows working
buttons and sticks.

## Scope
- Install usbip-win2 latest signed release (no test signing).
- `usbip list -r <shield-ip>`, `usbip attach -r <shield-ip> -b <busid>`.
- Verify in `joy.cpl` and/or https://hardwaretester.com/gamepad (or Steam controller test).

## Exclusions
- Moonlight session (TICKET-011).

## Acceptance criteria
1. `usbip list -r` shows the device with correct VID:PID.
2. `usbip attach` succeeds; `usbip port` shows it; Device Manager shows the controller with no error
   code.
3. All buttons, triggers, both sticks and D-pad register in a tester.
4. `usbip detach` removes it cleanly; reattach works.
5. Secure Boot / driver signature enforcement unchanged (`bcdedit` shows no `testsigning Yes`).

## Validation plan
Physical (PC + Shield):
1. Install usbip-win2 per `docs/windows-setup.md`.
2. Start sharing in the app.
3. Run the commands above; capture output to `evidence/010-*.txt`.
4. Press every control in the tester; note results.

## Required evidence
- Command outputs, Device Manager screenshot/text, tester notes, Shield logcat.

## Implementation notes / affected files
- `docs/windows-setup.md`.

## Remaining uncertainties / blockers
- None for this ticket. Installed by the assistant at the user's request ("You do it"), with hash and
  signature verification and a restore point first.

## Hardware results (2026-10-26) — PASS
Setup: usbip-win2 **0.9.8.1** signed release (`USBip-0.9.8.1-x64.exe`, SHA-256
`38cad6d4…a8518a` matching GitHub, Authenticode valid, signer Cloudyne Systems via GlobalSign EV),
installed silently with `/NORESTART` after System Restore point #160. Emulated host controller driver
signed by *Microsoft Windows Hardware Compatibility Publisher*.

| # | Criterion | Result |
|---|---|---|
| 1 | `usbip list -r` shows the device | PASS — `1-1 : Microsoft Corp. : Xbox360 Controller (045e:028e)` with 4 interfaces (`evidence/010-windows-attach.txt`) |
| 2 | attach, `usbip port`, Device Manager without error | PASS — "successfully attached", port shows Full Speed; Device Manager: **Xbox 360 Controller for Windows** (`xusb22.inf`, problem code 0) + HID game controller; serial `E417D87E94C1` matches the adapter |
| 3 | all controls register in a tester | PASS — Windows XInput (`tools/xinput-monitor.py`, the API games use): 15/15 buttons (A/B/X/Y, D-pad ×4, LB/RB, Start/Back, LS/RS, Guide), LT/RT 0–255, both sticks −32768…32767 on all axes (`evidence/010-xinput-monitor-pass.txt`, `evidence/010-xinput-monitor-l3r3ps.txt`) |
| 4 | detach and re-attach | PASS — `usbip detach -a` → "all ports are detached"; re-attach succeeded (port 1) |
| 5 | signature enforcement unchanged | PASS — `SystemStartOptions` has no `TESTSIGNING`; no bcdedit changes made |

Server-side numbers during the run (`evidence/010-shield-stats-attached.png`): 26,771 interrupt-IN
reports forwarded; report interval p50 4.00 ms / p99 8.01 ms (adapter `bInterval` 4 — a rate, not
latency); server turnaround p50 0.05 ms / p99 0.18 ms / max 0.51 ms (verbose logging on).

Notes:
- The first attempts produced no input because the adapter was in a stuck state (TICKET-024/025);
  after re-plugging it, input flowed immediately.
- An `attach` issued while sharing was off started usbip-win2's background re-attach attempts, which
  later imported a second time; the engine's same-PC eviction handled it. `usbip attach … -x` stops
  such attempts.
- usbip-win2 0.9.8.x release notes warn of possible regressions after a large refactor; none observed.
