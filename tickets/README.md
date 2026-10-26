# Ticket system

Repository-local, Markdown-only work tracking. No external service or account is needed.

## Workflow

1. A ticket starts in `backlog`. It moves to `ready` once its dependencies are `done` (or explicitly
   waived in the ticket with a reason) and its acceptance criteria are concrete.
2. Whoever starts a ticket sets `in_progress` and updates **Implementation notes** as work proceeds.
3. If work cannot continue, set `blocked` and write the exact blocker (for example a physical action
   on the Shield) under **Remaining uncertainties / blockers**. Keep working on independent tickets.
4. A ticket is `done` only when the Definition of Done below holds. Record evidence in the ticket
   (or under `evidence/` and link it). Failures and limitations are evidence too.
5. Bugs found during work become new tickets (prefix title with `BUG:`) with reproduction steps and
   a regression-validation plan.
6. When investigation exposes missing work, add a new ticket and list it in the index below.

## Status definitions

| Status | Meaning |
|---|---|
| `backlog` | Known work; dependencies not met or criteria not final. |
| `ready` | Dependencies met; can be started. |
| `in_progress` | Actively being worked. |
| `blocked` | Cannot progress without an external action/information (named in the ticket). |
| `done` | Definition of Done satisfied; evidence recorded. |

## Definition of Done

- Acceptance criteria are satisfied.
- Relevant validation has been executed (not merely written).
- Evidence is recorded, including failures and limitations.
- Source and documentation agree with the delivered behavior.
- No unresolved blocker remains within the ticket's scope.

Rules that override convenience:

- Compilation alone never completes an integration ticket.
- Simulated / host-side tests never complete a hardware-validation ticket (tagged **[HW]**).
- Polling/report interval is never reported as input latency. Latency improvement is never marked
  achieved without a controlled comparison (TICKET-018).

## Ordered ticket index

| # | Title | Milestone | Status | Priority | Depends on |
|---|---|---|---|---|---|
| [001](TICKET-001.md) | Ticket system and initial backlog | 1 | done | P0 | — |
| [002](TICKET-002.md) | Toolchain assessment and project skeleton | 1 | done | P0 | 001 |
| [003](TICKET-003.md) | Licensing and existing-code assessment | 1 | done | P0 | 001 |
| [004](TICKET-004.md) | **[HW]** Shield USB access feasibility with the 8BitDo adapter | 2 | blocked | P0 | 002 |
| [005](TICKET-005.md) | **[HW]** 8BitDo descriptor and transfer inspection (XInput mode) | 3 | in_progress | P0 | 004 |
| [006](TICKET-006.md) | USB/IP protocol codec (Rust) | 4 | done | P0 | 002, 003 |
| [007](TICKET-007.md) | usbdevfs URB engine (submit / reap / discard) | 4 | in_progress | P0 | 002 |
| [008](TICKET-008.md) | USB/IP server session and event loop | 4 | in_progress | P0 | 006, 007 |
| [009](TICKET-009.md) | Android integration: service, permission, interface claim, JNI | 4 | in_progress | P0 | 004 (architecture gate), 008 |
| [010](TICKET-010.md) | **[HW]** Windows USB/IP enumeration and controller input | 5 | blocked | P0 | 009 |
| [011](TICKET-011.md) | **[HW]** Background operation during a Moonlight session | 6 | blocked | P1 | 010 |
| [012](TICKET-012.md) | Cancellation, disconnect, reconnection, re-enumeration | 7 | in_progress | P0 | 008, 009 |
| [013](TICKET-013.md) | Windows automatic re-attach (usbip-win2 built-in) | 7 | in_progress | P2 | 010 |
| [014](TICKET-014.md) | Android TV interface and connection controls | 8 | in_progress | P1 | 009 (basic); 010 (polish) |
| [015](TICKET-015.md) | Local-network access controls and safe defaults | 9 | done | P0 | 008 |
| [016](TICKET-016.md) | Diagnostics and performance instrumentation | 10 | in_progress | P1 | 008, 009 |
| [017](TICKET-017.md) | **[HW]** Sustained-operation and reconnection soak | 11 | blocked | P1 | 011, 012 |
| [018](TICKET-018.md) | **[HW]** Controlled latency comparison | 11 | blocked | P1 | 010, 016 |
| [019](TICKET-019.md) | Reproducible APK build and release packaging | 12 | done | P0 | 002 |
| [020](TICKET-020.md) | Documentation: README, Windows setup, architecture | 12 | in_progress | P0 | 010 (final), 019 |
| [021](TICKET-021.md) | Rumble / force feedback (interrupt OUT) | 11 | backlog | P3 | 010 |
| [022](TICKET-022.md) | Publish source to GitHub | 12 | done | P1 | 019 |
| [023](TICKET-023.md) | BUG: stale selection after adapter mode change | 8 | done | P1 | 014 |
| [024](TICKET-024.md) | **[HW]** Adapter sends no input reports on Shield in XInput mode | 3 | blocked | P0 | 004 |

Statuses in this table are kept in sync with each ticket's header.

## Requirement → ticket traceability

| Requirement (from project brief) | Ticket(s) |
|---|---|
| Inspect workspace and Android build tools | 002 |
| Review cgutman/USBIPServerForAndroid and vadimgrn/usbip-win2; reuse with licensing/attribution; no VirtualHere code | 003 |
| Standard USB/IP with usbip-win2 client and signed release drivers; investigate compatibility | 003, 006, 008, 010 |
| No new Windows kernel driver unless demonstrably needed | 003, 010 |
| Milestone 1: prove unrooted Shield grants access to the 8BitDo's interfaces/endpoints | 004 |
| Explain exact limitation if Android prevents it, before big UI / new architecture | 004 (architecture gate for 009, 014) |
| Prove Windows enumerates forwarded adapter and receives input | 010 |
| Run without root on the Shield's actual Android version | 004, 009, 010 |
| Read USB transfers directly through Android USB APIs | 004, 007, 009 |
| Control and interrupt transfers, descriptors, initialization, other needed transfers (XInput mode) | 005, 007, 008, 010 |
| Forward independently of Android gamepad processing and Moonlight controller forwarding | 004 (kernel driver detach), 009, 011 |
| Transfer cancellation, disconnects, reconnection, re-enumeration | 012, 013, 017 |
| Run in background while Moonlight is active | 009, 011 |
| Simple Android TV UI usable with remote: select, permission, start/stop, status, errors | 014 |
| Avoid unnecessary buffering, polling delays, allocations in input path | 007, 008, 016 |
| Limit sharing to selected device; explicit start/stop | 009, 014, 015 |
| Local network only; never exposed to the internet | 015 |
| Do not weaken Windows security or install unsigned drivers | 010, 020 |
| One controller via 8BitDo adapter; no audio / arbitrary devices / isochronous | 008 (isochronous rejected), 015 (single device) |
| Rumble after reliable input | 021 |
| Validate incrementally steps 1–6 | 004, 024, 010, 011, 017, 018 |
| Distinguish report interval, network RTT, button-to-screen latency | 016, 018 |
| Buildable source, sideloadable APK, license and notices, Windows setup, test results | 019, 020, 003, 022 |
| Meaningful tests: protocol, cancellation, malformed requests, lifecycle failures, reconnection | 006, 008, 012 |
| Free/open source; no subscriptions, license servers, hardware binding | 003, 019 |
| Track discovered bugs as tickets | this README (workflow step 5) |
