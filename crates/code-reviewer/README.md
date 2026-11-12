# `code-reviewer` — Autonomous Multi-Lens Internal CI Appliance (`cheap-otto` Workspace Crate)

Stateful, multi-lens Pull Request code reviewer, compliance re-verifier, and living-documentation engine built for **Claude Code (`claude`)** and **Antigravity (`agy`)** running autonomously on your headless Apple Silicon MacBook (`otto-mac`).

---

## 1. Architecture & Zero-AI Polling Lifecycle

```text
  GitHub Watched Repos (`config/repos.toml`)
     │
     ▼  Pure Rust 1-query GraphQL batch poll every 2m (`launchd`, 0 AI tokens on idle ticks!)
  Event-Driven Async Dispatcher (`src/main.rs` + `src/state.rs`)
     │
     ├─► Per-SHA 2-strike retry circuit breaker + optional `poll_only_mode` token guard
     ├─► Spawns up to `max_parallel_prs` (default 8) INDEPENDENT detached OS sessions (`AbandonProcessGroup`)
     │
     ├─► 50/50 Subscription Load Intercalation (`claude` Sonnet ⇄ `agy` Medium):
     │      • Alternates lead runner across parallel PR sessions AND across the 8 review brains
     │      • Automatic runtime failover if either CLI hits a rate/quota limit
     │
     ├─► Immediate PR Timeline Banner (`⏳ Triggered & Running` -> `✅ Completed`)
     │
     ├─► Mode 1: INITIAL_FULL (fresh non-draft PR)
     │      ├─ Detached read-only git worktree (`<repo>-<pr>-<sha>-<pid>`) pinned to exact `head.sha`
     │      ├─ 7 Specialist Brains (`01..07`) executed simultaneously (`max_parallel_brains_per_pr = 8`)
     │      └─ Path-sharded parallel `08-adversarial-verifier` checks stripped claims (`path:line` + claim only)
     │
     ├─► Mode 2: RE_REVIEW (new push to open PR OR new human reply comment)
     │      ├─ Loads initial review baseline + all open `OBS-001..N` observations
     │      ├─ Evaluates `previous_sha...head_sha` delta diff + developer replies
     │      └─ Transitions observations (`OPEN` -> `FIXED_IN_CODE` | `ACCEPTED_RATIONALE` | `SUPERSEDED`)
     │
     ├─► Mode 3: POST_MERGE_DOCS (newly merged PR)
     │      └─ Updates `living-docs/<owner>__<repo>/00..04` + optional Google Drive Living Context Doc
     │
     ▼
  Outputs Published Atomically (per-PR exclusive spin-locked ledger on `~/.code-reviewer/state.json`)
     ├─ 1. Local Markdown + JSON ledger (`reviews/<owner>__<repo>/pr-<N>/`)
     ├─ 2. Optional Private Google Drive Living PR Doc (`gdrive` + `gdocs`)
     ├─ 3. Atomic GitHub PR Review (`COMMENT` / `APPROVE` / `REQUEST_CHANGES`) + Hunk-Validated Inline Diff Comments
     └─ 4. Live Web Dashboard GUI (`http://127.0.0.1:7430` via `code-reviewer ui`)
```

---

## 2. The 8 Review Brains & Isolated Knowledge Bases

Each brain lives in `brains/<id>/` with its own `PROMPT.md` and isolated knowledge base (`kb/lessons.md`, `kb/false-positives.md`, `kb/exemplars.md`):

| Brain ID | Lens Focus | Runs On |
| :--- | :--- | :--- |
| `01-intent-traceability` | Blind ticket acceptance criteria (`C1..Cn`), forward/reverse traceability, premature `Closes #N` | Every PR |
| `02-correctness-domain` | Edge cases, state transitions, race conditions, integer money invariants, silent failures | Code PRs |
| `03-security-boundaries` | Auth/session cookies, multi-tenant isolation, XSS/CSP/SSRF, supply-chain scripts | Code PRs |
| `04-data-migrations-ops` | Startup config validation, DB/RLS migrations, deterministic CI/build tooling | Code PRs |
| `05-testing-proof` | Tautological mock detection, negative/boundary assertions, guard test preservation | Code PRs |
| `06-ux-a11y-frontend` | UI state completeness, WCAG 2.2 AA semantics, design-system token discipline, i18n | Frontend PRs |
| `07-docs-contracts` | Zero-fictional-claims check in `.md`/ADRs, OpenAPI/schema alignment | Every PR |
| `08-adversarial-verifier` | Path-sharded skeptic verifying stripped `path:line` claims before GitHub posting | All Candidate Claims |

---

## 3. Quickstart

```bash
# 1. Copy the example configuration (gitignored so private org settings stay local)
cp crates/code-reviewer/config/repos.example.toml crates/code-reviewer/config/repos.toml

# 2. Verify environment & CLI auth
cargo run -p code-reviewer -- doctor

# 3. Deploy & provision onto your paired `otto-mac` M1 appliance (`launchd` 2m poller + `:7430` UI)
cargo run -p code-reviewer -- m1 --host otto-mac sync --sync-tokens

# 4. Open the SSH-tunneled Web Dashboard GUI (`http://127.0.0.1:7430`)
cargo run -p code-reviewer -- ui --host otto-mac
```
