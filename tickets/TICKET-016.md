# TICKET-016: Diagnostics and performance instrumentation

- **Status:** in_progress
- **Milestone:** 10
- **Priority:** P1
- **Dependencies:** 008, 009

## Problem
Latency claims need measurements that distinguish what is actually being measured.

## Intended outcome
Counters and timing histograms in the core, surfaced in the UI and logcat, each labelled with exactly
what it measures.

## Scope
Metrics (all monotonic-clock based, recorded without allocation):
- **Interrupt-IN completion interval** — time between consecutive completed interrupt-IN URBs that
  carried data. This is the *report interval* (device + host polling), **not latency**.
- **Server turnaround** — time from URB completion reaped on Shield to RET_SUBMIT fully written to the
  socket. Server-internal processing only.
- **Kernel queue time** — CMD_SUBMIT received → URB completed (includes waiting for the user to press
  a button; only meaningful for control transfers).
- Counters: submits, completions, unlinks, errors by status, bytes in/out, sessions, rejects.
- **Network RTT** is *not* measured by the server (USB/IP has no ping); measure with `ping` / `psping`
  from the PC and record separately.
- **Button-to-screen latency** can only be measured externally (TICKET-018).

## Exclusions
- On-device graphs.

## Acceptance criteria
1. Status JSON exposes the metrics with units and min/avg/p50/p99/max over the last N samples.
2. UI and docs label the interval metric "report interval (not latency)".
3. Unit tests for the histogram/percentile code.

## Validation plan
- `cargo test`; HW reading during TICKET-010.

## Required evidence
- Test output; HW status snapshot.

## Implementation notes / affected files
- `core/src/stats.rs`, `MainActivity.kt`.

## Remaining uncertainties / blockers
- Timestamp resolution on Shield (`CLOCK_MONOTONIC`, expected ns) unverified.

## Evidence so far
- Criteria 1–3 implemented: `stats.rs` (`reportIntervalUs`, `turnaroundUs` with n/min/p50/p99/max/avg),
  UI label "report interval (NOT latency)", `stats::tests` + `engine::tests::turnaround_recorded_on_flush`.
- Hardware reading pending TICKET-010.

## Hardware reading (2026-10-26, verbose logging on)
- Report interval (rate, not latency): min 0.19 / p50 4.00 / p99 8.01 / max 8.17 ms over 1024 samples.
- Server turnaround: p50 0.05 / p99 0.18 / max 0.51 ms.
- PC↔Shield ping (separate tool): 20/20, < 1 ms (Windows `ping` resolution 1 ms).
- `CLOCK_MONOTONIC` resolution adequate (sub-0.1 ms values observed).
