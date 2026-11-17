# Design: Zero-wire `.env` management and agent-isolated secret brokering

## 1. Current state and motivation

- **Source sync intentionally excludes `.env` (`src/bridge.rs`, `.github/workflows/ci.yml`):** `PeerNode::build_sync_plan_for_branch` pushes Git commits over SSH, while `.gitignore` and CI's `PR Governance & Secret Guard` reject `.env` and `.env.*` (allowing `.env.example`). As a result, synced checkouts on `otto-mac` have `.env.example` and no runtime `.env`.
- **Two security traps to avoid:**
  1. **Wire & log exposure:** Shipping raw `.env` files across `cheap-otto` commands, passing `-w <secret>` or `KEY=VALUE` on `argv` (`ps` visible), or storing unsalted `SHA-256(value)` hashes on disk exposes secrets (or offline-guessable digests of short/low-entropy values) across logs, state files, and Mission Control APIs.
  2. **Agent exfiltration:** `cheap-otto delegate` (`src/worktree.rs`), `ask` (`src/agent_session.rs`), `m1-chat-control` (`specs/m1-chat-control`), and Code Tender run autonomous LLM CLIs (`claude`, `agy`, `kiro`) inside Git worktrees. Any `.env` file placed in a worktree or inherited in child env vars can be read by prompt-injected repository content and sent to external LLM APIs or written into `~/.cheap-otto/events/*.ndjson`.

## 2. End-to-end architecture

```text
Workstation (Windows / Linux / macOS)                Paired MacBook (otto-mac)
┌──────────────────────────────────────────┐         ┌─────────────────────────────────────────────────────┐
│ Repo: .env.example (+ local .env)        │         │ Operator UID (~/.cheap-otto/env/, mode 0700)        │
│   ├─► env schema (offline key check)     │         │   └── <project>/<profile>/tree/[<subpath>/].env     │
│   └─► env diff --execute                 │         │       (0600) + macOS Login Keychain (security -i)   │
│        1. 256-bit ephemeral nonce N      ├── SSH ──┼─► cheap-otto env mac-digest --project P --profile F │
│        2. compare HMAC-SHA256(N, K\0V)   │  stdin  │◄─ KEY=<K> DIGEST=<hex> SOURCE=file|keychain         │
│        3. save redacted env-status.json  │         │   (no automated path carries secret values)         │
│                                          │         │                                                     │
│   ├─► env set <K> --remote --execute ────┼─ ssh -t ┼─► cheap-otto env set-local (/dev/tty, echo off)     │
│   │   (operator-typed TTY entry: the sole exception, encrypted SSH channel only, never read/stored/logged)   │
│   ├─► env pull --local / --remote ───────┼─────────┼─► each machine runs local op / gcloud / security    │
│   └─► exec --with-env --execute ─────────┼─────────┼─► cheap-otto env run (operator UID only)            │
│                                          │         │     ├── Command::envs -> workload (no argv secrets) │
│                                          │         │     └── redact_secret_values -> logs / stdout       │
│                                          │         │                                                     │
│                                          │         │ Agent worktrees (delegate / ask / chat / Tender)    │
│                                          │         │   └── .env.example only; 0 .env files; EACCES store │
└──────────────────────────────────────────┘         └─────────────────────────────────────────────────────┘
```

## 3. Pure core module (`src/env_secrets.rs`) [R1, R2, R3, R4, R5]

`src/env_secrets.rs` is 100% pure (no filesystem, network, subprocess, clock or entropy I/O; `#![forbid(unsafe_code)]`; zero new Cargo dependencies in `Cargo.toml`):

- **Scope & bijective path layout (`EnvScope`):** [R1.1, R1.2]
  - `EnvScope` holds private fields (`project`, `profile`, `subpath: Option<String>`) exposed through read-only accessors `project()`, `profile()` and `subpath()`; `EnvScope::new(project, profile, subpath) -> Result<Self, EnvError>` is the only constructor, so every scope reaches the path and Keychain helpers through validation.
  - `EnvScope::new` ASCII-lowercases `project`, `profile` and every `subpath` segment before validating, and the command layer lowercases `--project`, `--profile` and `--subpath`, so scopes differing only by case map to one canonical store path on case-insensitive filesystems.
  - `project` and `profile` (default `"dev"`) must pass `validate_safe_identifier` (`src/bridge.rs:159`).
  - `subpath` splits on `/`: `1..=8` non-empty segments, `<= 128` total bytes, each starting with ASCII `[A-Za-z0-9]` (forbidding leading `.` and `-`, so a segment can never be `.`, `..` or `.env`) and containing only `[A-Za-z0-9._-]`.
  - `EnvScope::relative_store_path(&self) -> String` returns `env/<project>/<profile>/tree/.env` (`None`) or `env/<project>/<profile>/tree/<subpath>/.env` (`Some`), making root and nested monorepo scopes (`apps` and `apps/web`) collision-free, while Keychain-backed key names live outside `tree/` in `env/<project>/<profile>/keychain.json` (`BTreeMap<String, Vec<String>>` keyed by `subpath` or `""`).
  - `EnvScope::keychain_service(&self) -> String` returns `com.cheapotto.env/<project>/<profile>` (`None`) or `com.cheapotto.env/<project>/<profile>/<subpath>` (`Some`), which is bijective because `project` and `profile` forbid `/`.
- **Dotenv parser, blocked keys & dummy annotations:** [R1.3, R1.4, R4.2]
  - `BLOCKED_ENV_KEYS: &[&str]` rejects loader, shell-startup, path and toolchain-hijack keys (`PATH`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES`, `DYLD_LIBRARY_PATH`, `DYLD_FRAMEWORK_PATH`, `DYLD_FALLBACK_LIBRARY_PATH`, `BASH_ENV`, `ENV`, `ZDOTDIR`, `IFS`, `CDPATH`, `SSH_AUTH_SOCK`, `HOME`, `USER`, `LOGNAME`, `SHELL`, `GIT_SSH_COMMAND`, `GIT_EXEC_PATH`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_COUNT`, `GIT_CONFIG_PARAMETERS`, `NODE_OPTIONS`, `PYTHONSTARTUP`, `PYTHONPATH`, `RUBYOPT`, `PERL5OPT`, `RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC_WRAPPER`) case-insensitively via `validate_env_key`.
  - `parse_dotenv(text: &str) -> Result<ParsedDotenv, EnvError>` parses `KEY=VALUE` (with optional `export `), single/double/unquoted values, rejects interior `\0` bytes and unclosed quotes, preserves last-wins map order (`BTreeMap<String, String>`), and records duplicate keys as non-secret warnings.
  - `parse_env_example(text: &str) -> Result<EnvSchema, EnvError>` extracts expected key names plus optional `# @otto:agent-dummy=<literal>` comments above a key, rejecting dummy literals that contain `://` userinfo (`@` before path) or secret prefixes (`sk-`, `ghp_`, `gho_`, `xox`, `eyJ`).
- **Pure-Rust SHA-256, HMAC-SHA256 & drift classifier:** [R2.2, R2.3, R2.4]
  - `sha256(data: &[u8]) -> [u8; 32]` and `hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32]` implemented in ~95 lines of safe Rust (`u32::wrapping_add`, `rotate_right`), verified against NIST FIPS 180-4 and RFC 4231 test vectors.
  - `compute_entry_hmac(nonce: &[u8; 32], key: &str, value: &str) -> String` hashes `key.as_bytes() || &[0u8] || value.as_bytes()` (injective because `validate_env_key` forbids `\0`) and formats 64 lowercase hex chars.
  - `diff_env_scope(schema: Option<&EnvSchema>, local_digests: &BTreeMap<String, String>, mac_entries: &BTreeMap<String, MacDigestEntry>) -> Vec<EnvKeyDiffRow>` classifies every key into `EnvKeySyncState::{Synced, ValueDrift, MissingOnMac, MissingOnWindows, MissingBoth, UndeclaredExtra}` and builds `EnvStatusReport` containing **zero values and zero digests**.
- **Vault references, `CommandSpec` planners & Keychain stdin:** [R2.2, R3.1, R3.3, R3.4]
  - `SecretRef` parses `op://<vault>/<item>/<field>`, `gcloud:<project>/<secret>[#<version>]`, and `keychain:<account>@<service>` (split at the first `@`; `account` a safe identifier, `service` `/`-separated segments of `[A-Za-z0-9._-]` without a leading `-`, matching `EnvScope::keychain_service` output `com.cheapotto.env/<project>/<profile>[/<subpath>]`; for `env set --keychain` the account is the variable name), validating every segment against safe ASCII characters with no leading `-` so references can never inject CLI flags into `op`, `gcloud` or `security`.
  - `build_mac_digest_probe_spec(peer: &PeerNode, scope: &EnvScope, nonce_hex: &str) -> Result<CommandSpec, EnvError>` builds `ssh -o ClearAllForwardings=yes -o BatchMode=yes -o ConnectTimeout=10 -p <port> -- <user>@<host> ~/.cargo/bin/cheap-otto env mac-digest --project <p> --profile <prof> [--subpath <s>]` (validated identifiers, no `sh -s`) using `CommandSpec::with_stdin` (`src/command.rs:58`) so `NONCE=<hex>` travels over `stdin`, never `argv`.
  - `build_remote_set_tty_spec(peer: &PeerNode, scope: &EnvScope, key: &str, keychain: bool) -> Result<CommandSpec, EnvError>` builds `ssh -t -o ClearAllForwardings=yes -o BatchMode=yes -o ConnectTimeout=10 -p <port> -- <user>@<host> ...` invoking `~/.cargo/bin/cheap-otto env set-local`.
  - `build_remote_pull_spec(peer: &PeerNode, scope: &EnvScope, manifest_json: &str) -> Result<CommandSpec, EnvError>` streams only the validated non-secret `SecretRef` JSON over SSH `stdin` to `~/.cargo/bin/cheap-otto env pull-local`.
  - `render_security_stdin_upsert(service: &str, account: &str, secret: &str) -> Result<String, EnvError>` formats the quoted `add-generic-password -U -a ... -s ... -w ...` line piped to `/usr/bin/security -i` on `stdin` so `-w <secret>` never appears in `ps` [R3.3]. It fails closed with a dedicated `EnvError::UnsupportedKeychainValue` when the secret contains NUL, CR, LF, a double quote or a backslash (those values use the `0600` file store) and with `EnvError::InvalidKeychainField` when `service` or `account` falls outside the safe charset; the exact `security -i` grammar is confirmed LIVE in T5.1.
- **Output scrubber & worktree `.env` guard:** [R4.4, R5.2]
  - `redact_secret_values(input: &str, secrets: &BTreeMap<String, String>) -> String` collects every non-empty secret value (no length, boolean or integer exemption), adds URL-encoded and JSON-escaped variants, sorts longest-candidate-first so inner substrings never split outer secrets, and replaces matches with `[REDACTED:<KEY>]`.
  - `find_forbidden_worktree_env_paths(paths: &[String]) -> Vec<String>` flags `.env` or `.env.*` at any directory depth except `.env.example`; the shell feeds it paths from `git status --porcelain -uall --ignored=matching` (or a complete filesystem scan) so untracked, modified and ignored (`!! .env`) entries are all inspected [R4.4].

## 4. Imperative shells (`src/main.rs`, `src/server.rs`) [R2, R3, R4, R5]

- **State & store locking:**
  - Workstation state files (`env-status.json`, `env-refs.json`, `env.lock`) resolve via `resolve_active_state_dir` (`--state-dir`, `./.cheap-otto`, repo-root `.cheap-otto`, `~/.cheap-otto`), with `env.lock` taken in the resolved directory. `env-status.json` stores only project, profile, subpath, key names, classification states, sources and probe timestamp (never values or digests), so `GET /api/env` returns scope without inferring it from `state_dir`; it is written through a PID-named temp file, `sync_all()`, atomic rename and cleanup on failure; it is `0600` on Unix and uses the inherited per-user ACL on Windows (`std::fs::rename` replaces the target) [R2.4].
  - Mac secret store files always live under `~/.cheap-otto/env/` (`0700` dirs), guarded by `~/.cheap-otto/env/env.lock`, written to a PID-named sibling temp file opened with `OpenOptionsExt::mode(0o600)` under `cfg(unix)` before writing bytes, `sync_all()`, and atomically renamed into place [R3.2].
- **CLI surface (`cheap-otto env <subcommand>`):**
  - `cheap-otto env schema [--project <p>] [--subpath <s>] [--json]` — offline check of `.env.example` vs local `.env` key names and `BLOCKED_ENV_KEYS` (prints key names only, never values).
  - `cheap-otto env diff` / `env status [--project <p>] [--profile <prof>] [--subpath <s>] [--execute] [--json]` — dry-run probe plan without `--execute`; with `--execute`, runs the ephemeral-nonce `HMAC-SHA256` probe and updates `.cheap-otto/env-status.json`.
  - `cheap-otto env set <KEY> [--profile <prof>] [--subpath <s>] [--keychain] [--remote|--local] [--execute]` — interactive hidden-TTY prompt (`stty -echo` on `/dev/tty` with a drop guard restoring echo on Unix; Console mode guard or hidden stdin prompt) directly on the target machine.
  - `cheap-otto env pull [--profile <prof>] [--subpath <s>] [--remote|--local] [--execute]` — resolves `SecretRef` entries locally on the chosen machine via discrete `Command::args` (`GH_TOKEN`/`GITHUB_TOKEN` stripped where unused).
  - `cheap-otto env run --project <p> [--profile <prof>] [--subpath <s>] -- <program> [args...]` — Mac-side operator-only broker used by `cheap-otto exec --with-env` and `schedule --ci --with-env`; before resolving any secret it calls a pure decision function `env_run_authorized(effective_uid, store_owner_uid)`, where the effective UID comes from `/usr/bin/id -u` (discrete args; `forbid(unsafe_code)` rules out `libc::getuid`) and the owner UID from `MetadataExt::uid` of `~/.cheap-otto/env`, and rejects non-operator invocations; then injects validated env map via `Command::envs` and streams output through `redact_secret_values`.
  - `cheap-otto env mac-digest` / `env set-local` — internal Mac-side subcommands invoked over SSH by `env diff` and `env set --remote`.
- **Mission Control API (`GET /api/env` in `src/server.rs`):** [R5.3]
  - Guarded by `RequestGuard`; reads `env-status.json` from `ServeConfig.state_dir`, never reads `~/.cheap-otto/env/` and never spawns SSH.

## 5. Key decisions & open questions (with defaults)

1. **Keychain vs `0600` file default on the Mac:** *Default: `0600` file under `~/.cheap-otto/env/`, with `--keychain` opt-in per key.* Headless SSH sessions without an unlocked GUI Keychain cannot read Login Keychain items without `security unlock-keychain`, whereas `0600` files owned by the operator UID work in unattended `launchd` CI jobs while still returning `EACCES` to `#71`'s separate agent user.
2. **Scope of `redact_secret_values`:** *Default: redact every non-empty loaded value* (plus URL-encoded and JSON-escaped forms, longest first) with no length, boolean or integer exemption, because the store accepts short values as secrets and printing one would leak it. Values that are not secrets belong in `.env.example` defaults, not the secret store.
3. **Reference manifest location:** *Default: repo-tracked `env.otto.toml` (safe because it holds only vault URIs, rejected if any raw literal fails `SecretRef` validation), with `.cheap-otto/env-refs.json` as a private local override.*
