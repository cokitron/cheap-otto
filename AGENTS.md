# AGENTS.md — `cheap-otto` Engineering & Rust Guidelines

This file defines the workspace rules for AI coding agents (Antigravity, Claude Code, Codex, Cursor) working in `cheap-otto`.

## Always-On Core Rules

Antigravity caps each workspace rule file at **24,000 bytes** after expanding `@[...](...)` includes. Only `CLAUDE.md` and rule `06` are inlined below so nothing is truncated; **read the matching path-scoped rules on demand** before editing files in their scope.

@[CLAUDE.md](CLAUDE.md)
@[Git Workflow & OSS Governance](.claude/rules/06-git-workflow-and-governance.md)

## Path-Scoped Rules & Specs (Read On Demand Before Editing)

Before modifying files in any of the scopes below, read the linked rule file in full:

- **Rust sources & manifests (`**/*.rs`, `**/Cargo.toml`):** [01-rust-idioms-and-edition-2024.md](.claude/rules/01-rust-idioms-and-edition-2024.md) and [04-clippy-formatting-and-docs.md](.claude/rules/04-clippy-formatting-and-docs.md)
- **Security, shell/PowerShell scripts & appliance guardrails (`**/*.rs`, `**/*.sh`, `**/*.ps1`, `**/Cargo.toml`):** [02-security-and-appliance-safety.md](.claude/rules/02-security-and-appliance-safety.md) and [.ai/security-and-tender-boundaries.md](.ai/security-and-tender-boundaries.md)
- **Functional core, shells & tests (`src/`, `tests/`, `crates/`):** [03-architecture-and-testing.md](.claude/rules/03-architecture-and-testing.md) and [.ai/architecture-and-invariants.md](.ai/architecture-and-invariants.md)
- **Dependencies, `clap` flags & JSON state evolution (`Cargo.toml`, `Cargo.lock`, `main.rs`):** [05-crate-ecosystem-and-cli-patterns.md](.claude/rules/05-crate-ecosystem-and-cli-patterns.md)
- **Autonomous PR reviewer & Code Tender (`crates/code-reviewer/`):** [07-code-reviewer.md](.claude/rules/07-code-reviewer.md)
- **Embedded dashboards, `server.rs` & `request_guard.rs`:** [08-embedded-web-ui.md](.claude/rules/08-embedded-web-ui.md)
- **Brand assets, `README.md`, `docs/` & `docs/blog/`:** [09-brand-and-pages-site.md](.claude/rules/09-brand-and-pages-site.md)
- **GitHub Actions CI, Rulesets, Dependabot & Releases (`.github/`, `Cargo.toml`, `Cargo.lock`):** [10-ci-and-release.md](.claude/rules/10-ci-and-release.md)
- **Specs & multi-PR task execution (`specs/`):** [11-specs-protocol.md](.claude/rules/11-specs-protocol.md) and [specs/README.md](specs/README.md)

## Workspace Skills

`.agents/skills.json` registers `crates/code-reviewer/.agents/skills/` (`update-m1`, `review-pr`, `fix-from-drive`) at the repository root. Keep `crates/code-reviewer/.agents/skills/` and `crates/code-reviewer/.claude/skills/` byte-identical when editing any skill.
