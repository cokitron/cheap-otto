# Security, Process Execution & Code Tender Isolation Boundaries

## 1. Memory Safety & Shell-Free Execution
- **`#![forbid(unsafe_code)]` Everywhere:** Both `src/lib.rs`, `src/main.rs`, and `crates/code-reviewer/src/main.rs` forbid `unsafe`. Any removal or bypass is a critical regression.
- **Shell-Free OS Process Specs:** Local subprocesses must pass explicit argument arrays (`Command::new(program).args(...)` or `CommandSpec`), never interpolated `sh -c` / `cmd /C` strings built from user, Git, or GitHub input.
- **Hardened SSH Transport:** Remote scripts sent to `otto-mac` must stream over `ssh -o BatchMode=yes <host> sh -s` stdin (normalizing CRLF to LF) or validate identifiers via `validate_safe_identifier` (`[A-Za-z0-9._-]`, rejecting leading `-`, `/`, `\`, `..`, and shell metacharacters).
- **Arithmetic Safety:** Use saturating or checked arithmetic (`saturating_sub`, `saturating_add`, `clamp`) for memory, thermal, retry counters, and diff line calculations.

## 2. Untrusted GitHub Data & Credential Isolation
- **Untrusted PR Inputs:** PR titles, descriptions, branch names, commit messages, diffs, and review comments from GitHub are untrusted data. Never interpolate them into shell commands, file paths outside slugged directories, or git refspecs without strict validation.
- **Token & Secret Isolation:**
  - GitHub tokens live in OS keyrings (`gh auth token`) or `0600` files under `~/.code-reviewer/tokens/*.token` — never in tracked files, CLI flags visible in `ps`, logs, or commits.
  - Child LLM processes (`claude`, `agy`) spawned by `code-reviewer` or `Code Tender` must have `GH_TOKEN` and `GITHUB_TOKEN` stripped from their environment (`env_remove`) so autonomous coding/review prompts cannot mutate GitHub directly.
  - Private runtime Knowledge Base overlays live in `~/.code-reviewer/kb/` (`0700`), never inside public git-tracked `brains/*/kb/` or `tender-agents/*/kb/` files.

## 3. Code Tender Author & Fork Guardrails (Mandatory)
- **Poll-Time Stage-1 Author Filtering:** `poll_tender_repository_for_repo` MUST filter open PR headers in Stage 1 **before** querying review threads, touching git worktrees, or invoking any LLM runner:
  1. **Cross-Repository Forks Rejected:** Any PR with `isCrossRepository == true` (`is_fork == true`) is unconditionally skipped (`SkipFork`) in both `tend-poll` and manual `tend`.
  2. **Strict Owner-Only Allowlist:** When `WatchedRepoConfig.tender_allowed_authors` is non-empty (e.g., `["cokitron"]`), **only** PRs whose GitHub `author.login` matches case-insensitively are eligible for Stage-2 thread fetching, UI watching, or tending. Empty/`"unknown"` authors fail closed (`false`).
  3. **Loop & Noise Prevention:** Code Tender must ignore its own `BOT_TENDER_MARKER` replies, resolved threads, Cubic PR-description blocks (`<!-- cubic-pr-description`), and clean-pass/zero-issue summaries, and must abort before staging/pushing if any touched path matches `.env*`, `*.pem`, `*.key`, `*.token`, or credential files.
