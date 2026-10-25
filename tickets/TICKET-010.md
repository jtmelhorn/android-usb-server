# TICKET-010: [HW] Windows USB/IP enumeration and controller input

- **Status:** blocked
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
- Blocked by TICKET-004/009 hardware access. Requires the user to install usbip-win2 (administrator
  prompt on Windows) — the installer modifies drivers, so it is the user's action.
