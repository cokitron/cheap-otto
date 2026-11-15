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
  1. force-pushes local `HEAD` to the Mac's repo as `otto-sync` and checks it out as `main`;
  2. runs `cargo test --all-targets` and `cargo clippy --all-targets -- -D warnings` there;
  3. `cargo install`s the binary and runs `scripts/m1-provision.sh`, which rewrites the `launchd` jobs (`com.cheapotto.code-reviewer.pr-poller`, `.ui`, `.code-tender`) and copies this crate's skills into the Mac's global `~/.claude/skills` and `~/.gemini/antigravity/skills`.
  So whatever is checked out locally becomes production. Run it only when asked, from the commit the user wants deployed.
- Prefer `code-reviewer m1 exec|push-file|status` to hand-written `ssh` strings: they go over stdin and survive PowerShell quoting.

## 2. Pipeline

```text
launchd (120s) → poll: one batched GraphQL query per repo, no LLM call when nothing changed
  → state.rs TriggerDecision: initial review | re-review (new push or human reply) | post-merge docs
  → detached per-PR worker process (up to max_parallel_prs)
      → read-only git worktree pinned to head.sha
      → brains 01..07 in parallel, lead runner alternating claude ⇄ agy, failover on quota errors
      → 08-adversarial-verifier, path-sharded, sees only `path:line` + claim
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

- Acts only on non-fork PRs by `allowed_authors` (default: the two configured accounts), on repos with `tender_enabled`.
- Ignores resolved threads, clean pass reviews and anything carrying its own marker.
- `max_autonomous_cycles_per_pr` (clamped to `1..=10`) caps fix loops; `tend-poll` stops dispatching for a PR that reached it. Items it can't fix are answered with the needs-human marker instead of another attempt.
- `TenderDeliveryMode::StagePatchOnly` stages a patch for one-click approval in the dashboard instead of pushing. Never bypass the mode check.
- Before committing it rejects `.env*`, key and submodule changes, runs the repo's `local_check_cmd`, and has the opposite provider verify the diff.
- Agents and check commands run with `GH_TOKEN` and `GITHUB_TOKEN` removed from the environment.

## 4. Prompts and knowledge bases are product, not docs

- `brains/<NN-name>/PROMPT.md` plus `kb/lessons.md`, `kb/false-positives.md` and `kb/exemplars.md`. Each brain reads only its own KB. `tender-agents/` follows the same shape.
- KB entries are appended by `code-reviewer kb add` and from the dashboard; keep the files append-friendly and don't reformat existing entries.
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
