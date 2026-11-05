# Security policy

noti-fi runs as a per-user service (no root) and listens only on a user-private Unix socket and the session D-Bus.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting (Security tab, "Report a vulnerability") rather than a public issue.
Include the version (`notify --version`), what you did, and what you expected. You can expect a first reply within a week.

## Supported versions

Only the latest release receives fixes. `notify upgrade` brings an install up to date.

## Scope notes

- `notify upgrade` downloads only over HTTPS from this project's release path and verifies a sha256 before replacing anything.
- Dependency advisories are checked in CI with `cargo deny` (see `deny.toml` for the few reviewed exceptions).
