---
paths:
  - ".github/**"
  - "**/Cargo.toml"
  - "Cargo.lock"
---

# CI & Release

Branching, commit and PR conventions live in rule 06 and `CONTRIBUTING.md`. This rule covers what CI runs, what a merge needs, and how a release is cut.

## 1. CI (`.github/workflows/ci.yml`)

Runs on pushes to `main` and on every pull request. A newer push to the same PR cancels the older run.

| Job | Runs on | What it checks |
| --- | --- | --- |
| `PR Governance & Secret Guard` | Ubuntu | The PR title is a Conventional Commit; a same-repo branch uses an approved prefix (`feat/`, `fix/`, `chore/`, `docs/`, `refactor/`, `test/`, `ci/`, `perf/`, `otto/`, `dependabot/`, `claude/`); no tracked `.env` or `.env.*` (the template `.env.example` is allowed), `*.token`, `*.pem`, `*.key`, `peer.json`, `state-backup.json` or `crates/code-reviewer/config/repos.toml` |
| `Rust Gate (ubuntu-latest)`, `(macos-latest)`, `(windows-latest)` | Ubuntu, macOS (`macos-15-intel`), Windows (fast-passed on Ubuntu when a PR touches only `specs/`, `docs/`, `README.md`, `CONTRIBUTING.md` or `LICENSE`) | `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo test --workspace --all-targets --locked`; Ubuntu also runs `cargo fmt --all -- --check` |

- For code-changing PRs the gate runs on all three OSes, so a path separator, a line ending or a shell assumption fails the PR. Docs- or spec-only PRs are fast-passed on Ubuntu (see the `Rust Gate` row above) and skip the Rust checks. Build paths with `Path::join`, never assert on separators, and gate OS-specific branches with `cfg`.
- Local toolchains differ too (the Windows workstation may be `windows-gnu`; CI and releases use `windows-msvc`). Don't rely on either's quirks.
- `--locked` means a stale `Cargo.lock` fails the tests. Commit the lockfile with every manifest change.
- `tests/governance_and_cubic.rs` checks `cubic.yaml`: each Cubic custom rule's description plus the files it lists must stay under 10,000 characters. Rules 01, 02, 03, 04 and 06 and the two `.ai/` files are those instruction files, so keep them lean and put detail in the other rule files.
- If you change a gate command, change it in `CLAUDE.md`, rule 04, rule 06 and `CONTRIBUTING.md` as well.

### What a merge into `main` needs

A repository ruleset enforces this; nobody can bypass it.

- Required checks: the four CI jobs above and `cubic · AI code reviewer`.
- The branch is up to date with `main`, and every review thread is resolved.
- Squash merge only, linear history, no force-push or deletion of `main`.
- A PR that never receives the Cubic check cannot merge. If that happens to a Dependabot PR, land the bump through a maintainer-authored PR instead (section 3).

## 2. Release (`.github/workflows/release.yml`)

- **Trigger:** pushing a tag matching `v*`.
- **Builds** `cargo build --release --workspace --locked --target <target>` for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`.
- **Packages** both binaries plus `README.md` and `LICENSE` as `cheap-otto-<tag>-<target>.tar.gz` (`.zip` on Windows), with `SHA256SUMS.txt`, and creates or updates the GitHub release for that tag.
- `--locked` means a stale `Cargo.lock` fails the release. Commit the lockfile with every manifest change.

### Cutting a release
1. Bump `version` in both `Cargo.toml` and `crates/code-reviewer/Cargo.toml` to the same value, run `cargo check --workspace` to refresh `Cargo.lock`, and merge that through a PR.
2. Tag the merge commit `vX.Y.Z` (matching the manifests) and push the tag.

Tagging and pushing a tag publish binaries to the public. Do it only when the user asks for a release.

## 3. Dependency updates

- Dependabot (`.github/dependabot.yml`) opens weekly PRs for Cargo crates (`chore(deps)`) and GitHub Actions (`ci(actions)`).
- For each one: read the upstream release notes for breaking changes and check that every required check reports. If it is blocked (no Cubic check, or it needs code changes), apply the bump on a `chore/` branch, run the gate, and open a maintainer-authored PR that says which Dependabot PRs it supersedes. Close those PRs once it merges.
- A major version bump is a code change, not a formality: adapt call sites in the same PR and say what changed.
- Workflow-only bumps cannot be exercised by PR CI when they touch `release.yml`; check the action's release notes for input and behaviour changes before bumping.

## 4. Working with GitHub

- **Repository owner:** `cokitron/cheap-otto`. If `gh` reports a permission error on push or PR creation, check which account is active with `gh auth status` and ask before switching accounts.
- **PR description:** what changed, how it was verified (the gate output, the dry run you read), and anything the reviewer must do by hand afterwards. `.github/PULL_REQUEST_TEMPLATE.md` has the sections.
- Commit, push, merge and tag only when the user asks. A request to change code is not a request to ship it.

## 5. What never goes in a commit

`.gitignore` already covers these; don't force-add them or paste their contents into tracked files, PR text or logs:

- `.cheap-otto/`, `peer.json`, `state-backup.json` (host names, users, IPs of the operator's machines)
- `crates/code-reviewer/config/repos.toml` (private org, repo and account names)
- `crates/code-reviewer/living-docs/*/` and `crates/code-reviewer/reviews/` (content from watched private repos)
- `.env`, `.env.*` (except `.env.example`), `*.token`, `*.pem`, `*.key`

Examples in docs and tests use placeholders: `192.168.1.42`, `100.101.102.103`, `admin`, `otto-mac`, `acme-org/web-app`.
