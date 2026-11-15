---
paths:
  - ".github/**"
  - "**/Cargo.toml"
  - "Cargo.lock"
---

# CI, Release & GitHub Workflow

## 1. CI (`.github/workflows/ci.yml`)

Runs on pushes to `main` and on every pull request, on `ubuntu-latest` with stable Rust:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
```

- CI covers Linux only. Windows and macOS are covered by developers and by the deploy step on the Mac, so keep the code free of platform assumptions: build paths with `Path::join`, never assert on separators, and gate OS-specific branches with `cfg`.
- Local toolchains differ too (the Windows workstation may be `windows-gnu`, releases build `windows-msvc`). Don't rely on either's quirks.
- If you change a gate command, change it in `CLAUDE.md` and rule 04 as well.

## 2. Release (`.github/workflows/release.yml`)

- **Trigger:** pushing a tag matching `v*`.
- **Builds** `cargo build --release --workspace --locked` for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`.
- **Packages** both binaries plus `README.md` and `LICENSE` as `cheap-otto-<tag>-<target>.tar.gz` (`.zip` on Windows), with `SHA256SUMS.txt`, and creates or updates the GitHub release for that tag.
- `--locked` means a stale `Cargo.lock` fails the release. Commit the lockfile with every manifest change.

### Cutting a release
1. Bump `version` in both `Cargo.toml` and `crates/code-reviewer/Cargo.toml` to the same value, run `cargo check --workspace` to refresh `Cargo.lock`, and merge that through a PR.
2. Tag the merge commit `vX.Y.Z` (matching the manifests) and push the tag.

Tagging and pushing a tag publish binaries to the public. Do it only when the user asks for a release.

## 3. Branches, commits and PRs

- **`main` is production in three ways:** CI gate, the Pages site (`main:/docs` deploys on merge), and the source for release tags. Don't commit to it directly; branch and open a PR.
- **Commit messages:** Conventional Commits with a scope from the area touched: `bridge`, `doctor`, `ask`, `ui`, `brand`, `code-reviewer`, `code-tender`, `release`, `security`. Subject in the imperative, under about 72 characters, and a body that says what changed and why when it isn't obvious.
- **PR description:** what changed, how it was verified (the gate output, the dry-run you read), and anything the reviewer must do by hand afterwards.
- **Repository owner:** `cokitron/cheap-otto`. If `gh` reports a permission error on push or PR creation, check which account is active with `gh auth status` and ask before switching accounts.
- Commit, push, merge and tag only when the user asks. A request to change code is not a request to ship it.

## 4. What never goes in a commit

`.gitignore` already covers these; don't force-add them or paste their contents into tracked files, PR text or logs:

- `.cheap-otto/`, `peer.json`, `state-backup.json` (host names, users, IPs of the operator's machines)
- `crates/code-reviewer/config/repos.toml` (private org, repo and account names)
- `crates/code-reviewer/living-docs/*/` and `reviews/` (content from watched private repos)
- `.env*`, `*.token`, `*.pem`, `*.key`

Examples in docs and tests use placeholders: `192.168.1.42`, `100.101.102.103`, `admin`, `otto-mac`, `acme-org/web-app`.
