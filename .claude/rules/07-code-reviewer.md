---
paths:
  - "crates/code-reviewer/**"
---

# `code-reviewer` Crate

An autonomous PR reviewer and review-feedback fixer that runs on the Mac under `launchd`. It posts to real GitHub PRs with real accounts and spends real LLM quota, so its safety rules matter more than its style rules.

## 1. Running it safely

- **It is live by default.** `poll`, `tend-poll`, `review` and `tend` call LLMs and write to GitHub unless you pass `--dry-run`. `sync-docs` writes to Google Drive. `watch add|remove` and `kb add` edit files in the workspace.
- **Read-only commands:** `doctor` (checks auth for `gh`, `claude`, `agy`, Drive), `status`, `watch list`.
- **Workspace resolution:** `--workspace` (default `.`) may point at the repo root or at `crates/code-reviewer`; the binary finds `brains/` either way and reads `config/repos.toml` beside it. That file is gitignored and holds real org and account names: never print it into a commit, PR or log.
- **Everything under `m1` touches the Mac.** In particular `m1 sync`:
  1. force-pushes local `HEAD` to the Mac's repo (`~/otto-workspaces/cheap-otto` by default) as `otto-sync`, then checks it out as `main` with a hard reset;
  2. runs `cargo test --workspace --all-targets --locked` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` there;
  3. `cargo install`s both binaries (`cheap-otto` first, then `code-reviewer`; `render_remote_build_script` in `m1.rs`) and runs `scripts/m1-provision.sh`, which rewrites the reviewer's `launchd` jobs (`com.cheapotto.code-reviewer.pr-poller`, `.ui`, `.code-tender`), copies this crate's skills into the Mac's global `~/.claude/skills` and `~/.gemini/antigravity/skills`, and in its section 8 pipes `cheap-otto dashboards install-script` to `/bin/sh`. That installs the dashboard services `com.cheapotto.dashboards.mission-control` and `com.cheapotto.dashboards.pixel-office` (skipped without `pixel-agents`). A failed dashboard install prints a warning and does not fail the deploy; skipped optional services are reported as skipped and do not fail the deploy.
  So whatever is checked out locally becomes production. Run it only when asked, from the commit the user wants deployed.
- **`ui` shares the dashboard tunnel.** When nothing answers on the local port, `open_tunneled_ui` (`server.rs`) kickstarts the Reviewer service on the Mac, then, when `.cheap-otto/peer.json` records a peer, `--host` is omitted or names that peer, and `--port` is 7430, runs `cheap-otto ui --execute --no-open` so the one tunnel that `cheap-otto` records in `tunnel.json` carries the Reviewer too. Any other host or port gets a Reviewer-only fallback tunnel (one `-L`). Both commands go through `strip_github_credentials`. `scripts/open-ui.ps1` only wraps `code-reviewer ui`; never open a tunnel by hand next to it.
- Prefer `code-reviewer m1 exec|push-file|status` to hand-written `ssh` strings: they go over stdin and survive PowerShell quoting.

## 2. Pipeline

```text
launchd (120s) → poll: one batched GraphQL query per repo, no LLM call when nothing changed
  → state.rs TriggerDecision: initial review | re-review (new push or human reply) | post-merge docs
  → detached per-PR worker process (up to max_parallel_prs)
      → read-only git worktree pinned to head.sha
      → applicable brains 01..07 in waves capped by `max_parallel_brains_per_pr`, lead runner alternating claude ⇄ agy, failover on quota errors
      → 08-adversarial-verifier, path-sharded, receives stripped candidate metadata and reads the referenced files/direct callers
      → diff.rs: inline comment if the line is inside a diff hunk, else summary observation
      → github.rs: one atomic review (COMMENT | APPROVE | REQUEST_CHANGES)
  → ledger ~/.code-reviewer/state.json + reviews/<owner>__<repo>/pr-<N>/ (+ optional Drive doc)
```

- **Ledger writes go through `with_exclusive_lock`.** Parallel workers share `state.json`; a write outside the lock loses another PR's update. There is a test for this (`exclusive_ledger_lock_preserves_concurrent_async_pr_updates`).
- **Markers prevent loops.** Bot output carries `BOT_COMMENT_MARKER`, `STATUS_BANNER_MARKER`, `BOT_TENDER_MARKER` or `BOT_TENDER_NEEDS_HUMAN_MARKER`, and the trigger logic ignores comments that carry them. Changing a marker string orphans every existing bot comment, so version it (`:v1` → `:v2`) and keep recognising the old one.
- **Hunk validation is not optional.** GitHub rejects the whole review with HTTP 422 if one inline comment points outside the diff. Every finding goes through `diff.rs` classification before posting.
- **A per-SHA retry breaker** stops a failing PR from burning quota on every tick, and `poll_only_mode` turns LLM work off entirely. Don't add a code path that reviews without consulting them.
- **Observations are stateful.** Re-reviews move `OBS-NNN` items between `ObservationStatus` values instead of re-raising them. Keep ids stable across runs.

## 3. Code Tender (`tender.rs`)

The only component that writes to other people's branches. Its limits are load-bearing:

- **Who it may tend** (`is_tender_author_allowed` in `config.rs`, applied by `run_tender_poll` in `main.rs`):
  1. If the repo sets `tender_allowed_authors`, only those authors are tended, watched PR or not.
  2. With no per-repo list, a PR the operator explicitly watched (ledger entry with `watched`) is tended whoever wrote it. Watching a PR is an explicit override.
  3. With no per-repo list and no watch, the author must be in the global `tender.allowed_authors`; when that is empty too, the author must be `accounts.primary`, `accounts.secondary` or the repo's `forced_account`.
  4. `tender_enabled = false` skips a repo's PRs **unless the PR is explicitly watched**, so the flag is not an unconditional off switch.
  5. In the author checks an empty or `unknown` author fails closed, and `*` in a list allows everyone.
- Fork PRs (`isCrossRepository`) are skipped at poll time, before any review thread is fetched, any worktree is touched or any LLM runs. `.ai/security-and-tender-boundaries.md` is the authoritative statement of these boundaries; keep it and the code in step.
- The reviewer has its own per-repo filters: `review_allowed_authors` and `skip_fork_prs`.
- Ignores resolved threads, clean pass reviews and anything carrying its own marker.
- `max_autonomous_cycles_per_pr` (clamped to `1..=10`) caps fix loops; `tend-poll` stops dispatching for a PR that reached it. Items it can't fix are answered with the needs-human marker instead of another attempt.
- `TenderDeliveryMode::StagePatchOnly` stages a patch for one-click approval in the dashboard instead of pushing. Never bypass the mode check.
- Before committing it runs the name-based secret guard (section 6), runs the repo's `local_check_cmd`, and has the opposite provider verify the diff.
- Its agents and check commands run with the GitHub tokens removed from their environment (section 6).

## 4. Prompts and knowledge bases are product, not docs

- `brains/<NN-name>/PROMPT.md` plus `kb/lessons.md`, `kb/false-positives.md` and `kb/exemplars.md`. Each brain reads only its own KB. Tender agents use `PROMPT.md` plus optional `notes.md`, per-agent `kb/lessons.md`, and shared files under `tender-agents/kb/` (`check-recipes.md`, `do-not-repeat.md`, `patterns-fixed.md`); they have no `false-positives.md` or `exemplars.md`.
- The tracked `kb/` files are public defaults. Entries learned at runtime (`code-reviewer kb add`, the dashboard) go to a private overlay under `<state_dir>/kb` (`~/.code-reviewer/kb` by default, see `runtime_kb_root`), so they survive a deploy and never put private repo names in git. Don't write runtime lessons into the tracked files.
- A prompt change alters what gets posted on PRs. Treat it like a code change: say what behaviour changes and why, and keep the output contract the parser expects (ids, `path:line`, severity and confidence tags).
- Brain ids are referenced by number and name in code, config, the dashboard and the README table. Renaming one means changing all four.
- `living-docs/*/` and `reviews/` are runtime output and gitignored.

## 5. Crate conventions

- **Binaries are resolved, not assumed.** `gh_binary()`, `git_binary()`, `claude_binary()` and `agy_binary()` prefer the Homebrew or `~/.local/bin` path because `launchd` runs with a minimal `PATH`. A new external tool gets the same treatment.
- **Cross-platform still applies.** The workstation half (`m1 …`, `ui`) runs on Windows; the appliance half runs on macOS. Branch on the target OS where a shell, opener or process check differs, as `server.rs`, `state.rs` and `tender.rs` do.
- **Std-only and synchronous.** See rule 05. Parallelism is OS processes and threads, not an async runtime.
- **Config is additive.** New keys get `#[serde(default)]`, a line in `config/repos.example.toml`, and a control in the dashboard settings if operators need to change it.
- **Skills are mirrored.** `.claude/skills/` and `.agents/skills/` in this crate must stay byte-identical; `m1 sync` installs both sets globally on the Mac. Edit one, copy to the other, and check with `diff -r`.
- **Scripts:** `scripts/*.sh` are POSIX `sh` for the Mac, `scripts/*.ps1` are Windows wrappers. See rule 02.

## 6. Tokens and subprocess environment

- GitHub access is `gh` with a per-account `GH_TOKEN` read from `<accounts.tokens_dir>/<user>.token` (`github.rs`). `accounts.tokens_dir` is required in `config/repos.toml`: there is no serde default, so omitting it fails config loading. The example config sets `~/.code-reviewer/tokens`, and `m1 sync --sync-tokens` writes tokens to that fixed path on the Mac whatever the config says, so a different `tokens_dir` receives no synced tokens. Keep the key at that value until the sync path reads it. It never mutates the global `gh` login. Token files are operator-provisioned: `code-reviewer m1 …` writes them on the Mac with mode 600 (`m1.rs`). Nothing validates file or directory permissions when a token is read, so a looser mode is not detected.
- **One helper strips tokens:** `strip_github_credentials` (`brains.rs`) removes `GH_TOKEN` and `GITHUB_TOKEN` from a `Command`. It is called for every LLM agent spawn (`invoke_agent_cli`, `invoke_tender_cli`), the runner auth probes (`is_runner_authenticated`), and Code Tender's dependency install and check commands (`install_worktree_js_deps_if_lockfile_diverged`, `run_local_check_gate`).
- **Not stripped today:** the formatters run by `auto_format_modified_files` (`cargo fmt`, `prettier`) inherit the ambient environment. A new spawn site for an agent, a repo command or a formatter must call the helper.
- **Pre-commit guard:** `find_forbidden_modified_files` (`tender.rs`) reads `git status --porcelain` and rejects paths by name only: basename `.env`, `.env.*` (except `.env.example`), paths ending `.token`, `.pem` or `.key`, paths containing `.git/`, and `peer.json`. It does **not** detect submodule or gitlink changes, so a changed gitlink can pass and be staged. Extend the guard (for example by checking gitlink mode `160000` entries) rather than working around it.
