# TICKET-018: [HW] Controlled latency comparison

- **Status:** blocked
- **Milestone:** 11
- **Priority:** P1
- **Dependencies:** 010, 016

## Problem
The goal is lower felt controller latency than Moonlight's own controller forwarding. That can only be
claimed after a controlled measurement.

## Intended outcome
Button-to-screen latency for three configurations, same game/test, same stream, same session settings:
- **A** controller → PC Bluetooth (AX211) directly (user's reference "feels faster").
- **B** controller → 8BitDo on Shield → Moonlight controller forwarding (current).
- **C** controller → 8BitDo on Shield → this app → USB/IP → PC.

## Scope
- Method: high-speed phone camera (≥240 fps) filming the controller button and the TV showing the
  Moonlight stream of a test that changes the screen on button press (e.g. a gamepad tester with a
  large visual indicator, or an in-game action with immediate response). ≥30 presses per configuration.
- Report median and p90 in ms; record frame rate, display mode, Moonlight/Sunshine settings, network
  link (Wi-Fi/Ethernet), and `ping` RTT PC↔Shield.
- Separately report: report interval (TICKET-016 metric) and network RTT — clearly labelled as
  *not* button-to-screen latency.

## Exclusions
- Claims about improvement without the measurement above.

## Acceptance criteria
1. Table of A/B/C with n, median, p90 and the conditions above.
2. Conclusion states measured difference and uncertainty (frame duration = ±4.2 ms at 240 fps).

## Validation plan
- Procedure above; raw per-press frame counts in `evidence/018-latency.csv`.

## Required evidence
- CSV, summary table, conditions.

## Implementation notes / affected files
- `docs/latency-method.md`.

## Remaining uncertainties / blockers
- Needs user's camera and physical presence; blocked by TICKET-010.
