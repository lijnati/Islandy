# Security policy

## Supported versions

Only the latest release receives fixes. Islandy updates itself (Settings › About), so staying
current is automatic unless you turned updates off.

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Use GitHub's private reporting
instead: **Security › Report a vulnerability** on this repository. Include:

- the Islandy version (Settings › About) and Windows version;
- what an attacker can do, and the steps to reproduce;
- the diagnostic summary if relevant (Settings › Privacy › Copy diagnostic summary; it contains
  no activity or secrets).

You'll get an acknowledgement as soon as possible, and a fix or mitigation plan once the report
is confirmed. Credit is given in the release notes unless you prefer otherwise.

## Scope highlights

- The local activity receiver (`127.0.0.1`, token-authenticated)
- The `islandy` command-line modes (`hook`, `notify`, `status`)
- Edits to Claude Code's `settings.json`
- The updater (signature verification of GitHub release installers)
- The IPC surface between the WebView and Rust

The design and threat model are in [docs/security.md](docs/security.md).
