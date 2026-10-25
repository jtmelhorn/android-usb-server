# TICKET-021: Rumble / force feedback (interrupt OUT)

- **Status:** backlog
- **Milestone:** 11 (follows reliable input)
- **Priority:** P3
- **Dependencies:** 010

## Problem
Rumble commands travel host → device on the interrupt-OUT endpoint. The generic forwarding path
already handles interrupt OUT, but it is unverified and could interfere with input timing.

## Intended outcome
Rumble works in a Windows tester/game, without degrading input report interval.

## Scope
- Verify interrupt-OUT URBs reach the adapter and the controller vibrates.
- Check whether the 8BitDo adapter forwards rumble to a DualShock 4 in XInput mode.

## Exclusions
- LED control, audio.

## Acceptance criteria
1. **[HW]** Vibration test in a tester triggers rumble on the controller.
2. Report-interval stats during rumble within 10% of without.

## Validation plan
- HW test with verbose URB log.

## Required evidence
- Notes + status snapshot.

## Implementation notes / affected files
- Expected: no code change; `session.rs` already forwards interrupt OUT.

## Remaining uncertainties / blockers
- Adapter firmware support for DS4 rumble in XInput mode is unknown.
