## Summary
<!-- Concisely explain what changed and why. Link related issue(s), e.g. `Closes #123`. -->

## Affected Workspace Scope
- [ ] `cheap-otto` root crate (`src/`)
- [ ] `code-reviewer` / Code Tender crate (`crates/code-reviewer/`)
- [ ] Governance / CI / Cubic AI config (`cubic.yaml`, `.github/`, `.ai/`, `.claude/`, docs)

## Safety & Invariant Checklist
- [ ] Within the size budget (`git diff --shortstat origin/main...HEAD -- . ':(exclude)Cargo.lock' ':(exclude)specs/**' ':(exclude)docs/**' ':(exclude)*.md'`): at most 600 changed code lines (docs/specs excluded) and one theme, or a purely mechanical change that says so in the description
- [ ] PR title follows **Conventional Commits** (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`, `ci:`, `perf:`, `build:`, `revert:`)
- [ ] `#![forbid(unsafe_code)]` preserved; no `.unwrap()` / `.expect()` added in non-test production code
- [ ] Subprocesses use shell-free argument arrays (`CommandSpec` / `.args(...)`); untrusted inputs are validated
- [ ] Zero secrets, `.token` files, `config/repos.toml`, or private repository/org identifiers in tracked files
- [ ] Deterministic unit/integration tests added or updated (no live SSH or network required)

## Verification Output
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-targets --locked`
