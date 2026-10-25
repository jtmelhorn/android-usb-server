# TICKET-020: Documentation — README, Windows setup, architecture

- **Status:** in_progress
- **Milestone:** 12
- **Priority:** P0
- **Dependencies:** 010 (final verification), 019

## Problem
Users must be able to install, configure and troubleshoot without reading code, and docs must match
actual behaviour.

## Intended outcome
- `README.md`: what it is, status (what is verified vs not), quick start, safety notes.
- `docs/windows-setup.md`: usbip-win2 signed release install, firewall notes, attach/detach,
  auto-reattach helper, uninstall.
- `docs/architecture.md`: data path, threading, lifecycle, protocol intercepts.
- `docs/shield-setup.md`: sideloading, network debugging, permissions.
- `docs/latency-method.md` (shared with TICKET-018).

## Scope
- The documents listed above, kept in sync with ticket evidence.

## Exclusions
- Video tutorials.

## Acceptance criteria
1. Every command in the docs has been run at least once (or marked *unverified* inline).
2. README's status section matches ticket statuses at release time.
3. Windows guide never instructs enabling test signing or disabling Secure Boot.

## Validation plan
- Review against tickets; run commands that can be run on the PC.

## Required evidence
- Review checklist in this ticket.

## Implementation notes / affected files
- `README.md`, `docs/*.md`.

## Remaining uncertainties / blockers
- Windows steps unverified until the user installs usbip-win2 (TICKET-010).
