# TICKET-001: Ticket system and initial backlog

- **Status:** done
- **Milestone:** 1 — Workspace, toolchain, licensing, existing-code assessment
- **Priority:** P0
- **Dependencies:** none

## Problem
The project has many hardware-dependent unknowns. Work needs a traceable, service-free record of
what is required, what was validated, and what remains unverified.

## Intended outcome
`tickets/README.md` (workflow, statuses, ordered index, traceability) and one Markdown file per work
item, covering every requirement in the project brief.

## Scope
- Ticket template fields: title, status, problem/outcome, scope/exclusions, dependencies, priority,
  acceptance criteria, validation plan, required evidence, implementation notes/affected files,
  uncertainties/blockers.
- Backlog organised around the 12 milestones in the brief.
- Requirement → ticket traceability table.

## Exclusions
- External issue trackers, GitHub Issues sync.

## Acceptance criteria
1. `tickets/README.md` defines workflow, the five statuses, Definition of Done, and an ordered index.
2. Each milestone 1–12 has at least one ticket.
3. Every requirement bullet in the brief appears in the traceability table with ≥1 ticket.
4. TICKET-004 (USB access feasibility) is a dependency of the forwarding architecture (TICKET-009).
5. TICKET-010 (working Windows controller input) is a dependency of UI polish (TICKET-014).
6. Every ticket file contains all template sections with non-placeholder content.

## Validation plan
- `tools/check-tickets.py` checks every `TICKET-*.md` for required section headings and a valid status,
  that the index lists every ticket file, and that index statuses match ticket headers.
- Manual review of the traceability table against the brief.

## Required evidence
- Output of `python tools/check-tickets.py` (recorded below).

## Implementation notes / affected files
- `tickets/README.md`, `tickets/TICKET-001.md` … `TICKET-022.md`, `tools/check-tickets.py`.
- Hardware tickets are tagged **[HW]** so simulated tests cannot close them.

## Evidence
```
$ python tools/check-tickets.py
checked 22 tickets: OK
```
(Re-run after every status change; last run recorded in the final handoff.)

## Remaining uncertainties / blockers
- None for this ticket. New tickets will be added as investigation exposes work.
