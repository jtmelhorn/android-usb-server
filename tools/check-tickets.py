#!/usr/bin/env python3
"""Validate the repository-local ticket system (see tickets/README.md)."""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "tickets"
STATUSES = {"backlog", "ready", "in_progress", "blocked", "done"}
REQUIRED_SECTIONS = [
    "## Problem",
    "## Intended outcome",
    "## Scope",
    "## Exclusions",
    "## Acceptance criteria",
    "## Validation plan",
    "## Required evidence",
    "## Implementation notes / affected files",
    "## Remaining uncertainties / blockers",
]
REQUIRED_FIELDS = ["**Status:**", "**Priority:**", "**Dependencies:**"]


def main() -> int:
    errors = []
    tickets = sorted(ROOT.glob("TICKET-*.md"))
    header_status = {}
    for path in tickets:
        text = path.read_text(encoding="utf-8")
        num = path.stem.split("-")[1]
        if not text.startswith(f"# TICKET-{num}:"):
            errors.append(f"{path.name}: title must start with '# TICKET-{num}:'")
        for field in REQUIRED_FIELDS:
            if field not in text:
                errors.append(f"{path.name}: missing field {field}")
        for section in REQUIRED_SECTIONS:
            m = re.search(re.escape(section) + r"[^\n]*\n(.*?)(?=\n## |\Z)", text, re.S)
            if not m:
                errors.append(f"{path.name}: missing section '{section}'")
            elif len(m.group(1).strip()) < 3:
                errors.append(f"{path.name}: empty section '{section}'")
        m = re.search(r"\*\*Status:\*\*\s*(\w+)", text)
        status = m.group(1) if m else None
        if status not in STATUSES:
            errors.append(f"{path.name}: invalid status {status!r}")
        header_status[num] = status
        if "## Acceptance criteria" in text:
            crit = text.split("## Acceptance criteria", 1)[1].split("\n## ", 1)[0]
            if not re.search(r"^\s*(\d+\.|-)\s+\S", crit, re.M):
                errors.append(f"{path.name}: acceptance criteria must be a list")

    index = (ROOT / "README.md").read_text(encoding="utf-8")
    rows = re.findall(r"^\| \[(\d{3})\]\(TICKET-\d{3}\.md\) \|.*?\| (\w+) \| P\d \|", index, re.M)
    indexed = dict(rows)
    for num, status in header_status.items():
        if num not in indexed:
            errors.append(f"index: TICKET-{num} not listed")
        elif indexed[num] != status:
            errors.append(f"index: TICKET-{num} status {indexed[num]} != header {status}")
    for num in indexed:
        if num not in header_status:
            errors.append(f"index: lists missing TICKET-{num}")

    if errors:
        print("\n".join(errors))
        print(f"checked {len(tickets)} tickets: {len(errors)} problem(s)")
        return 1
    print(f"checked {len(tickets)} tickets: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
