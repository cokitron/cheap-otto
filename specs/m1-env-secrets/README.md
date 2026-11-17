# m1-env-secrets: Zero-wire `.env` management and agent-isolated secret brokering

**Status:** in progress (`requirements.md`, `design.md`, `tasks.md` and PR 1 `T1.1` ready; Phases 1–4 ship as 7 PRs under the 600-code-line cap, followed by two owner-approved **LIVE** verification tasks T5.1 and T5.2).

## The goal in one paragraph

Keep Windows (`D:\projects\<app>`) and the paired Apple Silicon MacBook (`otto-mac`) aligned on `.env` configuration across `~/otto-workspaces/<project>` and `~/repos/<gh-account>/<project>` while enforcing two hard security boundaries: **no secret value ever crosses the Windows-to-Mac bridge**, and **no coding agent (`claude`, `agy`, `kiro`, Code Tender) ever reads or inherits real `.env` secrets**. Each machine provisions secrets locally (via interactive Mac TTY entry into `0600` storage or macOS Keychain, or via local vault CLI pulls from `op`, `gcloud` or `keychain`), the bridge compares only `.env.example` schemas and ephemeral `HMAC-SHA256` digests, agent worktrees stay free of `.env` files, and only the operator-owned broker (`cheap-otto env run` / `exec --with-env`) injects secrets into runtime workloads while scrubbing secret values from captured output.

## What "done" looks like

- `cheap-otto env schema` validates `.env.example` keys, flags blocked execution-hijack variables (`DYLD_*`, `LD_PRELOAD`, `PATH`, `GIT_SSH_COMMAND`, `NODE_OPTIONS`, `RUSTC_WRAPPER`), and reports undeclared or missing local keys without printing any values.
- `cheap-otto env diff --execute` generates a fresh 256-bit ephemeral nonce, probes `~/.cargo/bin/cheap-otto env mac-digest` on `otto-mac` over SSH `stdin`, compares `HMAC-SHA256(nonce, key || "\0" || value)` per key, reports `Synced`, `ValueDrift`, `MissingOnMac`, `MissingOnWindows`, `MissingBoth` or `UndeclaredExtra`, and caches only redacted key states in `.cheap-otto/env-status.json`.
- `cheap-otto env set <KEY> --remote --execute` opens an interactive SSH TTY straight to the Mac (`stty -echo` on `/dev/tty`, or `security -i` over `stdin` for Keychain) so secret values never pass through Windows memory, `argv`, `ps`, `git` or logs; `cheap-otto env pull --local|--remote --execute` resolves `op://`, `gcloud:` and `keychain:` references locally on each machine.
- Agent worktrees (`delegate`, `ask`, `schedule --agent`, `chat`, Code Tender) contain zero `.env` files (only `.env.example`), cannot read `~/.cheap-otto/env/` (`0700` / `0600` operator-owned), and block commits if a turn creates `.env*`.
- `cheap-otto exec --with-env --cmd "<cmd>" --execute` (and `cheap-otto env run` on the Mac) injects scoped secrets into the child workload under the operator UID and scrubs any secret values (`len >= 6`, longest-first) from logged stdout/stderr.
- Mission Control (`:7420`) exposes cached redacted key counts and drift state at `GET /api/env`.

## Read in this order

1. [`../README.md`](../README.md): the specs protocol and PR size budget (`<= 600` lines).
2. [`requirements.md`](requirements.md): five requirements (`R1`–`R5`) with EARS acceptance criteria.
3. [`design.md`](design.md): architecture, pure `src/env_secrets.rs` interfaces, CLI commands, and key decisions.
4. [`tasks.md`](tasks.md): 9 tasks across Phases 1–5 and a 7-PR rollout table.
