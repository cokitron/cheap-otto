# Contributing to `cheap-otto`

Thank you for contributing to **`cheap-otto`**! This repository is an open-source Cargo workspace containing two tightly integrated crates:

1. **`cheap-otto`** (`src/`) — Cross-OS Dev Bridge & Parallel Agent Worktree Farm pairing Windows/Linux workstations with headless Apple Silicon MacBooks over SSH.
2. **`code-reviewer`** (`crates/code-reviewer/`) — Autonomous Multi-Lens Internal CI Reviewer & Code Tender review-response appliance.

To keep the codebase safe, deterministic, and easy to review for both humans and **Cubic AI**, we enforce a strict, transparent Git and engineering workflow.

---

## 1. Git Strategy & Branching Model

- **`main` is strictly Pull-Request-only.** Direct pushes to `main` are never allowed.
- **Branch from `main`:** Always create short-lived, single-purpose branches from the latest `origin/main` using lowercase kebab-case prefixes:
  - `feat/<topic>` — new user-facing feature
  - `fix/<topic>` — bug fix paired with a regression test
  - `chore/<topic>` — maintenance, governance, dependency or config updates
  - `docs/<topic>` — documentation improvements
  - `refactor/<topic>` — behavior-preserving internal cleanup
  - `test/<topic>` — unit or integration test additions
  - `ci/<topic>` — GitHub Actions or CI gate updates
  - `perf/<topic>` — measurable runtime or memory optimization
  - `otto/<task-id>` — automated `cheap-otto` delegated worktree branches
- **Conventional Commits & PR Titles:** Both commit subjects and Pull Request titles **must** follow [Conventional Commits](https://www.conventionalcommits.org/):
  ```text
  feat(bridge): support custom SSH port in tunnel planner
  fix(code-reviewer): filter non-owner PRs in stage-1 tender poll
  chore(governance): configure cubic.yaml and OSS contribution rules
  ```
- **Small PRs:** keep each PR to at most 400 changed lines (tests included, `Cargo.lock` excluded) and one concern, so it can be reviewed in one sitting. Check with `git diff --shortstat origin/main...HEAD -- . ':(exclude)Cargo.lock'`. Split bigger work into PRs that each leave `main` green; [`specs/README.md`](specs/README.md#pr-size-budget) explains how.
- **Squash-Merge Policy:** Pull Requests squash-merge into `main` as a single clean Conventional Commit. Keep PRs focused so reviews stay fast and bisectable.

---

## 2. Local Verification Gate (Required Before Opening a PR)

Every Pull Request is tested across **Linux, macOS, and Windows** in GitHub Actions and must pass all three Cargo gates locally with **zero warnings**:

```bash
# 1. Formatting check
cargo fmt --all -- --check

# 2. Strict Clippy lint check across all workspace crates, targets, and features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Deterministic unit & integration test suite (offline, no SSH/GitHub auth required)
cargo test --workspace --all-targets --locked
```

---

## 3. Non-Negotiable Engineering & Security Standards

1. **Zero `unsafe` (`#![forbid(unsafe_code)]`):** Both crates forbid `unsafe` Rust.
2. **No Panics in Production Paths:** Never add `.unwrap()`, `.expect()`, `panic!()`, or unchecked slice indexing on external/OS inputs in library or CLI runtime code; return structured `Result` errors (`thiserror` / typed error enums).
3. **Shell-Free Subprocess Execution:** Build OS commands with explicit argument arrays (`CommandSpec` / `Command::new(...).args(...)`) rather than interpolated shell strings. Validate identifiers before constructing remote paths or tmux/git names.
4. **Reversible Host Modifications:** Any macOS power/sleep (`pmset`) or `launchd` schedule operation must preserve deterministic backup and `restore` paths.
5. **Deterministic Tests Only:** Unit and integration tests must never make live network calls, require SSH access to an M1 host, or mutate `~/.cheap-otto` / `~/.code-reviewer` in the user's real home directory (use temporary directories).
6. **Public Sanitization:** Never include tokens, API keys, `.env` files, `crates/code-reviewer/config/repos.toml`, real private host credentials, or private organization names in commits, fixtures, comments, or PR descriptions.

---

## 4. Automated Code Review (Cubic AI) & Maintainer Tending

- **Cubic AI (`/cubic.yaml`)** automatically reviews Pull Requests opened against `main` (including external contributor PRs) using repository-specific safety and architecture rules in `.claude/rules/` and `.ai/`.
- When Cubic AI leaves inline review comments on your PR, please address valid findings with a new commit and test, or reply directly on the review thread explaining any intentional trade-off.
- Note for contributors: the maintainer's local **Code Tender** daemon (`crates/code-reviewer`) is configured to act only on the maintainer's own Pull Requests (`tender_allowed_authors = ["cokitron"]`, `skip_fork_prs = true`), so it is not intended to run on or comment on external contributor or fork Pull Requests.

---

## 5. License

By contributing to `cheap-otto`, you agree that your contributions will be licensed under the [MIT License](LICENSE).
