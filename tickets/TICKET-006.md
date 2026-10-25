# TICKET-006: USB/IP protocol codec (Rust)

- **Status:** done
- **Milestone:** 4 — Minimal Android USB forwarding service
- **Priority:** P0
- **Dependencies:** 002, 003

## Problem
The server must speak USB/IP 1.1.1 exactly as usbip-win2 expects, and must survive malformed or
hostile input from the network.

## Intended outcome
`core/src/proto.rs`: encoders/decoders for OP_REQ/REP_DEVLIST, OP_REQ/REP_IMPORT, CMD_SUBMIT,
RET_SUBMIT, CMD_UNLINK, RET_UNLINK and the 312-byte `usbip_usb_device` record, with an incremental
(streaming) parser that never panics.

## Scope
- Big-endian encoding per `Documentation/usb/usbip_protocol.rst`.
- Incremental parser: returns `NeedMore`, a complete message, or a protocol error.
- Limits: OUT payload ≤ 1 MiB (configurable const), unknown commands → error, isochronous
  `number_of_packets` > 0 → rejected at session level.

## Exclusions
- Isochronous packet descriptors (out of scope).

## Acceptance criteria
1. Byte-exact unit tests for every message against hand-built vectors from the spec.
2. Malformed input tests: truncated headers, bad version, unknown op/command, negative or oversized
   lengths, garbage — all produce errors, no panic.
3. A fuzz-style randomized test (fixed seed, ≥100k inputs) shows no panics.

## Validation plan
- `cargo test` in `core/`.

## Required evidence
- Test output summary.

## Implementation notes / affected files
- `core/src/proto.rs`, tests in-module.

## Remaining uncertainties / blockers
- Interop is proven only by TICKET-010.

## Evidence (2026-10-24)
`cargo test`: all `proto::tests` pass (device record layout, DEVLIST/IMPORT replies, spec vector for
CMD_SUBMIT, incremental parsing at every cut point, iso descriptor consumption, field validation,
RET_SUBMIT/RET_UNLINK bytes, 200 000 randomized inputs without panic). Same tests pass on the
Shield in both ABIs (`evidence/core-tests-on-shield.txt`).
