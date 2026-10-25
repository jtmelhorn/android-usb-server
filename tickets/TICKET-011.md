# TICKET-011: [HW] Background operation during a Moonlight session

- **Status:** blocked
- **Milestone:** 6
- **Priority:** P1
- **Dependencies:** 010

## Problem
Moonlight takes the foreground on the Shield; Android may throttle or kill background work, and
Moonlight might also see the controller.

## Intended outcome
With sharing on, an unchanged Moonlight session streams video/audio while the controller input
arrives on the PC only via USB/IP.

## Scope
- Foreground service + wake/Wi-Fi locks (TICKET-009).
- Confirm Moonlight does not see the controller (it is claimed by our app) so there is no double
  input via Moonlight's virtual controller.

## Exclusions
- Changes to Moonlight or Sunshine settings beyond noting them.

## Acceptance criteria
1. 30-minute Moonlight session with sharing on: no drop of the USB/IP connection (status counters).
2. In-game input works; Sunshine shows no Moonlight-forwarded controller for this pad
   (or Moonlight's "controller" count stays 0 for it).
3. Screen-off / Moonlight pause behaviour noted.

## Validation plan
- Start sharing → attach on PC → launch Moonlight session → play 30 min → check app status and
  `adb logcat -s UsbIpCore UsbIpBridge`.

## Required evidence
- Log excerpt with session duration and counters; notes.

## Implementation notes / affected files
- None beyond TICKET-009 yet.

## Remaining uncertainties / blockers
- Blocked by TICKET-010.
- Shield "battery/background" policies differ by firmware; foreground service is expected to be
  sufficient (unverified).
