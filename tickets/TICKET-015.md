# TICKET-015: Local-network access controls and safe defaults

- **Status:** done
- **Milestone:** 9
- **Priority:** P0
- **Dependencies:** 008

## Problem
USB/IP has no authentication or encryption. A device exported to the wrong network is a remotely
usable input device.

## Intended outcome
Sharing is off by default, exports only the selected device, and only accepts LAN clients.

## Scope
- Listener only exists between explicit Start and Stop.
- Accept only peers in private/link-local ranges: IPv4 10/8, 172.16/12, 192.168/16, 169.254/16,
  127/8; IPv6 `fc00::/7`, `fe80::/10`, `::1`, and IPv4-mapped forms of the above. Others are closed
  immediately and logged.
- Optional single allowed client IP (recommended: the gaming PC).
- One client session at a time; a second import gets an error reply.
- No UPnP / NAT-PMP / port forwarding code; documentation warns never to port-forward 3240.
- Only the selected busid is listed/importable.

## Exclusions
- TLS/authentication (not part of USB/IP; would break usbip-win2 compatibility).

## Acceptance criteria
1. Unit tests for the address classifier (IPv4, IPv6, mapped, edge cases).
2. Session test: non-allowed peer is rejected before any protocol byte is processed.
3. Session test: second concurrent importer rejected; first unaffected.
4. Default settings: sharing stopped, allowed client empty (= any private address), port 3240.

## Validation plan
- `cargo test`; manual: attempt `usbip list` from a disallowed IP (if available) and observe log.

## Required evidence
- Test output.

## Implementation notes / affected files
- `core/src/acl.rs`, `core/src/server.rs`, `SettingsActivity.kt`.

## Remaining uncertainties / blockers
- Carrier-grade NAT (100.64/10) is treated as non-private (rejected); documented.

## Evidence (2026-10-24)
- Criterion 1: `acl::tests` (local ranges, public/edge ranges incl. 100.64/10, 172.15/172.32,
  mapped IPv6, `fec0::`, multicast; allowed-client policy).
- Criterion 2: `engine::tests::public_peer_rejected_before_any_protocol`, `allowed_client_only`.
- Criterion 3: `engine::tests::second_importer_from_other_machine_is_busy`.
- Criterion 4: `Prefs` defaults (`sharingEnabled=false`, `allowedClient=""`, `port=3240`).
- All pass on host and on the Shield.
