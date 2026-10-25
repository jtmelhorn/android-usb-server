# TICKET-022: Publish source to GitHub

- **Status:** done
- **Milestone:** 12
- **Priority:** P1
- **Dependencies:** 019

## Problem
The project must be open source at https://github.com/jtmelhorn/android-usb-server (empty repo as of
2026-10-24).

## Intended outcome
Source, tickets, docs, license and notices pushed to `main`; APK attached to a GitHub release once a
build exists (binary not committed to git).

## Scope
- `git init`, `.gitignore` (build outputs, local.properties, keystores), initial commit, push.
- GitHub release with APK + SHA256SUMS (requires `gh` or the web UI — `gh` is not installed; release
  creation is left to the user or done via web UI on request).

## Exclusions
- CI workflows.

## Acceptance criteria
1. `git ls-remote` shows `main` at the pushed commit.
2. No secrets or build outputs in the repository (`git ls-files` review).

## Validation plan
- `git push`; `git ls-remote origin`.

## Required evidence
- Push output / commit hash.

## Implementation notes / affected files
- `.gitignore`.

## Remaining uncertainties / blockers
- Push depends on local git credentials for GitHub being available.

## Evidence (2026-10-25)
- `git push -u origin main` → `* [new branch] main -> main`; `git ls-remote origin main` →
  `1d54f6afa3b9175e9432ce047dff8cd6a7f4a94d refs/heads/main`.
- `git ls-files` (83 files) contains no keystores, `local.properties`, `.so`, `.apk` or `target/`.
- Limitation: no GitHub Release created (`gh` CLI not installed). The APK is built locally to
  `dist/` with `SHA256SUMS`; attaching it to a release is a follow-up for the user or a later session.

