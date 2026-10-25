# TICKET-005: [HW] 8BitDo descriptor and transfer inspection (XInput mode)

- **Status:** blocked
- **Milestone:** 3 — USB descriptor and transfer inspection
- **Priority:** P0
- **Dependencies:** 004

## Problem
The server must support exactly the transfers the adapter and Windows' driver use. These are not
known for this adapter firmware.

## Intended outcome
Recorded device/config/interface/endpoint descriptors and a trace of every USB/IP request Windows
issues during enumeration and play, with any unsupported request identified.

## Scope
- Descriptor dump from the Probe (raw hex + decoded).
- The server's verbose URB trace (`adb logcat -s UsbIpCore`) during Windows attach and input.
- Note interface count, endpoint types/intervals, vendor control requests (e.g. Xbox 360 security
  interface requests), string descriptors.

## Exclusions
- Isochronous/audio interfaces (rejected with `-EXDEV`/`-EINVAL`, documented).

## Acceptance criteria
1. Decoded descriptor table committed in `docs/8bitdo-descriptors.md`.
2. URB trace from one full attach → play → detach recorded.
3. Each request type in the trace is classified supported / intercepted / rejected, and no request
   needed for input fails.

## Validation plan
- Run probe (TICKET-004 procedure) and an attach with trace enabled (TICKET-010 procedure).

## Required evidence
- `evidence/005-descriptors.txt`, `evidence/005-urb-trace.log`, `docs/8bitdo-descriptors.md`.

## Implementation notes / affected files
- Trace toggle: *Verbose URB log* in Settings → logs one line per submit/complete (off by default
  to keep the input path quiet).

## Remaining uncertainties / blockers
- Blocked by TICKET-004 (Shield access).
