# Security Policy

## Reporting a vulnerability

Mutter runs with real macOS-sensitive access: microphone capture,
Accessibility (for text injection), and Screen Recording (for
system-audio capture). If you find a way to:

- exfiltrate dictated audio or transcripts off the device (this project's
  entire premise is zero network calls after the model download — a leak
  here is a critical bug, not a feature gap),
- escalate or retain a macOS permission beyond what the user granted,
- inject text into an application the user didn't intend,
- read or tamper with another user's local history database or settings,

please **do not open a public GitHub issue**. Instead, report it privately
via **GitHub's [private vulnerability reporting](https://github.com/brownfrosamurai/mutter/security/advisories/new)**
(Security tab → "Report a vulnerability") or by emailing
femimeduna@gmail.com.

Include:

- macOS version and Mutter version/commit
- Steps to reproduce
- What you observed vs. what should have happened

## What's out of scope

- Findings that require physical/root access to an already-compromised
  machine (Mutter's local SQLite history and settings are not encrypted at
  rest — this is a known, accepted tradeoff for a local-first single-user
  desktop app, not something we're tracking reports on).
- The lack of Apple notarization on unsigned/ad-hoc builds — this is a
  documented, intentional v1 scope decision (see `CLAUDE.md`), not a
  vulnerability.

## Response

This is currently a solo-maintained project. Expect an initial response
within a few days, not guaranteed SLAs. Fixes for confirmed vulnerabilities
will be prioritized ahead of feature work.
