# TICKET-023: BUG: stale device selection after the adapter changes mode disables "Grant permission"

- **Status:** done
- **Milestone:** 8
- **Priority:** P1
- **Dependencies:** 014

## Problem
The 8BitDo adapter first attached as `2dc8:3107` and was auto-selected. It then re-enumerated as
`045e:028e` (XInput). The saved selection no longer matched, so the device row showed no ✓ and
*Grant permission* was disabled (not focusable). The remote user had no hint why. Focus also jumped
back to *Start sharing* whenever the list refreshed.

Reproduction (before fix):
1. Prefs contain `vid=11720 pid=12551` (2dc8:3107); only the XInput adapter (045e:028e) is connected.
2. Open the app: no ✓ on the device; D-pad RIGHT from *Start sharing* skips *Grant permission*.

## Intended outcome
With exactly one USB device connected, a non-matching selection is replaced automatically. Focus
stays on the device row after selecting it.

## Scope
- `MainActivity.refreshDevices()`: auto-select when exactly one device is present and the selection
  does not match; restore focus to the previously focused row after rebuilding the list.
- Longer probe window (30 s) so a person can reach the controller.

## Exclusions
- Multiple simultaneous devices (selection stays manual).

## Acceptance criteria
1. With stale prefs (`2dc8:3107`) and only `045e:028e` connected, opening the app rewrites the selection
   to `vid=1118 pid=654` and shows ✓.
2. Selecting a device row keeps focus on that row.

## Validation plan
- Regression: write stale prefs with `adb shell run-as … shared_prefs/bridge.xml`, force-stop, launch,
  read prefs back, screenshot.

## Required evidence
- Prefs before/after; screenshot.

## Implementation notes / affected files
- `android/app/src/main/java/io/github/jtmelhorn/usbipbridge/MainActivity.kt`,
  `ProbeActivity.kt`, `res/values/strings.xml`.

## Evidence (2026-10-26)
- Criterion 1: prefs written as `vid=11720 pid=12551`; after launch they read `vid=1118 pid=654`;
  `evidence/023-auto-reselect.png`.
- Criterion 2: code change reviewed; focus restoration only observed indirectly (row stayed
  selectable by D-pad during the probe session).

## Remaining uncertainties / blockers
- None within scope.
