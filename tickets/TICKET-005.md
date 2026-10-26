# TICKET-005: [HW] 8BitDo descriptor and transfer inspection (XInput mode)

- **Status:** done
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
- None.

## Evidence so far (2026-10-26)
- Criterion 1 done: `docs/8bitdo-descriptors.md` (decoded from raw bytes read through the USB/IP
  server, `evidence/005-client-test.txt`).
- Partial request classification recorded there (client-driven); the Windows-driven URB trace
  (criterion 2) is pending TICKET-010.

## Criteria 2–3 (2026-10-26)
- Windows-driven trace: `evidence/010-windows-enum-trace.log` (setup bytes and payloads per URB).
- Classification: every request is forwarded or intercepted as designed; the only failing request is
  vendor `C1 01 0100 0000 len 20`, which the adapter itself stalls (-EPIPE) and which xusb22 tolerates.
  Nothing needed for input fails.
