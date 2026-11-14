# Security Policy

## Supported Versions

Security fixes are applied to the latest state of the `main` branch and included in subsequent tagged releases.

| Version / Branch | Supported |
| :--- | :--- |
| `main` (latest) | ✅ Yes |
| Older commits / unmaintained forks | ❌ No |

## Security Model Overview

`cheap-otto` and `crates/code-reviewer` execute developer workflows across local workstations, SSH-paired macOS hosts, and GitHub Pull Requests. We treat the following classes of bugs as security vulnerabilities:

1. **Command or SSH Injection:** Unescaped or shell-interpolated branch names, task IDs, repository slugs, paths, or PR metadata passed to local subprocesses or remote SSH sessions.
2. **Untrusted GitHub Input Handling:** Path traversal, git ref injection, or credential exposure triggered by untrusted Pull Request titles, descriptions, diffs, review comments, or cross-repository forks.
3. **Code Tender Author / Fork Bypass:** Any flaw allowing `Code Tender` to execute AI runners, check out untrusted fork code for automated fixing, or push changes on a Pull Request authored outside the configured allowlist (`tender_allowed_authors`).
4. **Credential Leakage:** Exposure of `GH_TOKEN` / `GITHUB_TOKEN`, SSH keys, `.token` files, or private configuration in logs, child LLM process environments, or git-tracked files.
5. **Host Safety Guardrail Bypass:** Corrupting or bypassing `MacOsStateBackup` restoration or battery charge ceiling validation (`50..=100%`).

## Reporting a Vulnerability

**Please do NOT open a public GitHub issue for suspected security vulnerabilities.**

Instead, report vulnerabilities privately through **[GitHub Private Vulnerability Reporting](https://github.com/cokitron/cheap-otto/security/advisories/new)** on this repository.

Please include:
- Affected crate and file/function (`cheap-otto` or `crates/code-reviewer`)
- Minimal, sanitized reproduction steps or proof-of-concept input
- Potential impact and preconditions (e.g., untrusted PR metadata vs. local CLI invocation)

We aim to acknowledge reports within **72 hours** and coordinate a fix + regression test before public disclosure.
