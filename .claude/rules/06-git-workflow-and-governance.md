# Git Workflow, Open-Source Governance & Cubic AI Review Rules

## 1. Branching & Pull-Request-Only `main`
- **`main` is strictly PR-only.** Never commit or force-push directly to `main`. Every change—whether by the maintainer (`@cokitron`), an external contributor, or an AI agent—lands through a Pull Request targeting `main`.
- **Short-lived, single-purpose branches** branched from `origin/main` using lowercase kebab-case prefixes:
  - `feat/<scope-or-topic>` — new user-facing capability
  - `fix/<scope-or-topic>` — bug fix with regression test
  - `chore/<scope-or-topic>` — tooling, config, governance, dependency maintenance
  - `docs/<scope-or-topic>` — documentation or architecture guides
  - `refactor/<scope-or-topic>` — behavior-preserving structural cleanup
  - `test/<scope-or-topic>` — test coverage additions
  - `ci/<scope-or-topic>` — GitHub Actions / CI pipeline updates
  - `perf/<scope-or-topic>` — measurable performance/memory improvements
  - `otto/<task-id>` — automated `cheap-otto` delegated worktree branches
- **PR size:** a PR changes at most 600 lines (tests included, `Cargo.lock` excluded) and covers one theme. Small related changes share a PR; work that will not fit is split into parts that each leave `main` green; `specs/README.md` (PR size budget) says how. Purely mechanical changes (lockfile, generated assets, renames, dependency bumps) may exceed this when the PR says so and contains nothing else.
- **Squash-merge to `main`:** Pull Requests squash-merge into a single atomic Conventional Commit on `main` so history remains linear, bisectable, and changelog-ready.

## 2. Conventional Commits & PR Titles
- Commit subjects and Pull Request titles MUST follow Conventional Commits:
  `<type>(<optional-scope>): <imperative summary>` (72 characters max recommended for commit subjects; no trailing period).
- Allowed `<type>` values: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`, `perf`, `build`, `revert`.

## 3. Pre-PR Verification Gate (Mandatory)
Every PR must pass all three workspace gates across Linux, macOS, and Windows with zero warnings:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --locked
```

## 4. Cubic AI Reviews & Code Tender Coexistence
- **Cubic AI (`/cubic.yaml`)** is the automated code reviewer for `cokitron/cheap-otto`. Keep `/cubic.yaml` valid against `https://cubic.dev/schema/cubic-repository-config.schema.json`, keep `issues.auto_fix: "off"` so Cubic never pushes competing commits, and keep each custom rule's `description + file_paths` text well below 10,000 characters.
- **Public Sanitization Rule:** Never commit real IP addresses (use RFC 5737 documentation examples like `192.0.2.x` or `198.51.100.x`, or placeholder LAN examples), private organization/repository names, tokens, `.env` files, or `crates/code-reviewer/config/repos.toml` into git-tracked files.
