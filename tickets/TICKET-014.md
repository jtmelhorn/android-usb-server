# TICKET-014: Android TV interface and connection controls

- **Status:** in_progress
- **Milestone:** 8
- **Priority:** P1
- **Dependencies:** 009 (basic controls); 010 (polish beyond basics)

## Problem
The Shield has only a D-pad remote. The user needs to select the device, grant permission, start/stop
sharing and see status/errors without a touchscreen or keyboard.

## Intended outcome
A single main screen, fully D-pad navigable, with a Leanback launcher entry.

## Scope (basic — allowed before TICKET-010)
- Device list (name, VID:PID, interface classes); selected device remembered (VID/PID/serial).
- Buttons: *Grant permission*, *Start sharing* / *Stop sharing*, *Probe* (TICKET-004), *Settings*.
- Status panel: state (Stopped / Listening on IP:port / Client connected from IP / Device detached —
  waiting / Error), counters (URBs, reports, errors), last error with actionable text.
- Settings: port, allow only a specific client IP, verbose URB log.

## Polish (after TICKET-010)
- Focus visuals, larger type, notification actions, help text with the Windows command line.

## Exclusions
- Touch-optimised layouts, theming.

## Acceptance criteria
1. Every control reachable and operable with D-pad + OK/Back only (manual check on Shield).
2. Leanback launcher shows the app (`LEANBACK_LAUNCHER` category + banner).
3. Errors (no permission, claim failed, port in use, device detached) show human-readable text.
4. Status updates at least once per second while visible; no polling when not visible.

## Validation plan
- Manual walk-through on the Shield with the remote; screenshots via `adb exec-out screencap -p`.

## Required evidence
- Screenshots in `evidence/014-*.png`; notes.

## Implementation notes / affected files
- `MainActivity.kt`, `SettingsActivity.kt`, `res/layout/*.xml`, `res/drawable/banner.xml`.

## Remaining uncertainties / blockers
- Manual check requires Shield access (blocked with TICKET-004).

## Evidence so far
- `evidence/014-main-screen-no-device.png`: main screen on the Shield (1080p TV), focus ring on
  *Start sharing*, attach command with the Shield's real IP, empty-device message.
- Leanback launcher entry: `aapt2 dump badging` shows `leanback-launchable-activity` and banner.
- Remote walk-through with a device present pending TICKET-004.

## Hardware use (2026-10-26)
- Full flow operated with D-pad key events only: select device, Grant permission (system dialog),
  Probe, Start/Stop sharing. Found and fixed TICKET-023 (stale selection / disabled button).
