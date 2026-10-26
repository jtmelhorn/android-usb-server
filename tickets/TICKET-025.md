# TICKET-025: BUG: claiming the adapter during host detection leaves it sending no input

- **Status:** in_progress
- **Milestone:** 7
- **Priority:** P0
- **Dependencies:** 024

## Problem
On plug-in the 8BitDo adapter enumerates in several identities while it detects the host (first
`2dc8:3107`, then `045e:028e` XInput; device numbers advance by 2 per cycle). On 2026-10-26 the app
auto-selected the transient `2dc8:3107` identity and sharing was started on it. About 28 s later the
adapter switched to XInput and from then on produced **no input reports at all**: none to this app,
none to Android's own `xpad` driver, none to Windows over USB/IP. Controller off/on and a USB port reset
(`USBDEVFS_RESET`) did not recover it. **Unplugging and replugging the adapter did**; afterwards input
flowed normally to Android and through USB/IP to Windows (TICKET-010).

Reproduction (suspected, not yet repeated deliberately):
1. Plug the adapter in while the app is open with no selection (or a stale one).
2. Auto-select picks `2dc8:3107`; start sharing within the detection phase.
3. After the switch to XInput, no reports arrive on `0x81`; `getevent` on the xpad node shows nothing.

## Intended outcome
The app never claims the adapter while it is still detecting the host, so the stuck state cannot be
triggered by the app.

## Scope
- Auto-select only XInput-class devices (`ff/5d/01` on interface 0) — implemented in
  `MainActivity.refreshDevices()` / `UsbDevices.isXInput()`.
- Service only (re)starts sharing for the selected VID:PID (already the case), so a transient
  identity is never claimed automatically.
- Document "if no input after plugging in: unplug, wait 5 s, replug, and start sharing only after the
  controller works in the Shield menus" in `docs/shield-setup.md`.

## Exclusions
- Adapter firmware behaviour itself.

## Acceptance criteria
1. With the app open while the adapter is plugged in, the selection never becomes `2dc8:*`.
2. Plug → wait until the controller works in the Shield UI → start sharing → Windows input works
   (repeat 3×).
3. Deliberate reproduction attempt recorded (does claiming during detection really cause it?).

## Validation plan
- Hardware procedure above; record VID:PID sequence with the attach-activity log.

## Required evidence
- Logs/screenshots per run.

## Implementation notes / affected files
- `MainActivity.kt`, `UsbDevices.kt`, `docs/shield-setup.md`.
- Built 2026-10-26; **not yet installed or validated on the Shield** (installation deferred to avoid
  interrupting the attached test session).

## Remaining uncertainties / blockers
- Root cause inside the adapter is unknown; the causal link to the app's claim is a hypothesis based on
  timing (claim at 11:13:30, switch to XInput at 11:13:58, no input afterwards until power cycle).
- Needs a user-present hardware session.

## Update (2026-10-26 13:15)
- **`2dc8:3107` is the adapter's idle identity**, reported by Android as product name **"8BitDo IDLE"**
  with a single HID interface (`03/00/00`). It appears whenever no controller is connected (e.g. the
  DualShock went to sleep) and switches to XInput `045e:028e` when a controller connects. It is not a
  host-detection mode as first assumed; the hypothesis above is revised accordingly.
- The original stuck state therefore followed sharing being started on the IDLE device (claimed with
  force) and the adapter then switching to XInput while claimed. Root cause still unconfirmed.
- Criterion 1 observed on device: with the controller asleep, the app listed `8BitDo IDLE [2dc8:3107]`
  and did **not** auto-select it (selection stayed `045e:028e`).
- Sustained run before the controller slept: 493,374 interrupt-IN reports forwarded, 1 error total
  (the known vendor-request stall), report interval p50 4.00 / p99 8.01 / max 8.02 ms, server
  turnaround p50 0.03 / p99 0.06 / max 0.13 ms (verbose off). Supports TICKET-017.

