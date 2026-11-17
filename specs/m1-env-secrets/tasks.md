# Tasks: Zero-wire `.env` management and agent-isolated secret brokering

Read `../README.md`, `requirements.md` and `design.md` before starting any task. Every PR stays within 600 changed code lines (`Cargo.lock`, `specs/`, `docs/` and `*.md` excluded) and passes the three workspace gates (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --locked`). Specs ship in PR 1 alongside the first slice of Rust code.

## PR plan

| PR | Tasks | Branch | Needs |
| --- | --- | --- | --- |
| 1 | Spec + T1.1 scope layout, dotenv parser & blocked keys | `feat/m1-env-secrets-t1-1-parser-and-scope` | nothing |
| 2 | T1.2 pure-Rust HMAC-SHA256 & drift classifier | `feat/m1-env-secrets-t1-2-hmac-and-diff` | PR 1 |
| 3 | T1.3 vault refs, SSH/Keychain planners, output redactor & worktree guard | `feat/m1-env-secrets-t1-3-vault-and-plans` | PRs 1, 2 |
| 4 | T2.1 local store, `mac-digest`, `set-local`, `pull --local` & `env run` | `feat/m1-env-secrets-t2-1-local-store-and-run` | PR 3 |
| 5 | T2.2 workstation bridge CLI (`schema`, `diff`, `status`, `set --remote`, `exec --with-env`) | `feat/m1-env-secrets-t2-2-bridge-cli` | PR 4 |
| 6 | T3.1 Mission Control `GET /api/env` & UI readiness card | `feat/m1-env-secrets-t3-1-hub` | PR 3 |
| 7 | T4.1 user, security & agent documentation | `docs/m1-env-secrets-t4-1-docs` | PRs 5, 6 |

After PR 7, the owner runs **LIVE** tasks T5.1 and T5.2 against the paired MacBook and workstation.

## Phase 1: pure planners & verifiers (`src/env_secrets.rs`)

### T1.1 Scope layout, `.env` / `.env.example` parser & blocked keys

- [x] Done
- **PR:** #74
- **Covers:** R1.1, R1.2, R1.3, R1.4, R4.2
- **Depends on:** nothing
- **Files:** `src/env_secrets.rs` (new), `src/lib.rs`
- **Steps:**
  1. Create `src/env_secrets.rs` (`#![forbid(unsafe_code)]`, zero I/O) with `EnvError`, `EnvScope`, `BLOCKED_ENV_KEYS`, `validate_env_key`, `parse_dotenv`, `render_dotenv`, and `parse_env_example`.
  2. Enforce `EnvScope` bijective store paths (`env/<project>/<profile>/tree/.env` and `env/<project>/<profile>/tree/<subpath>/.env`), keep its fields private behind `project()`, `profile()` and `subpath()` accessors with `EnvScope::new` as the only constructor, ASCII-lowercase every identifier in `EnvScope::new`, and reject unsafe `project`, `profile` or `subpath` segments.
  3. Export `pub mod env_secrets;` in `src/lib.rs` in alphabetical order.
- **Tests** in `src/env_secrets.rs`:
  - `scope_paths_are_bijective_across_root_and_nested_monorepo_subpaths`
  - `scope_identifiers_are_case_canonical_so_store_paths_cannot_collide_on_case_insensitive_filesystems`
  - `rejects_blocked_execution_hijack_and_malformed_env_keys`
  - `parses_quoted_escaped_and_commented_dotenv_and_rejects_nul_or_unclosed_quotes`
  - `parses_env_example_and_rejects_credential_bearing_agent_dummies`
- **Acceptance:** `cargo test -p cheap-otto --lib env_secrets::tests` passes.

### T1.2 Pure-Rust SHA-256, HMAC-SHA256 & zero-secret drift classification

- [ ] Open
- **Covers:** R2.1, R2.2, R2.3, R2.4
- **Depends on:** T1.1
- **Files:** `src/env_secrets.rs`
- **Steps:**
  1. Implement pure safe Rust `sha256` and `hmac_sha256` plus `compute_entry_hmac(nonce, key, value)` over `key || 0x00 || value`.
  2. Implement `format_mac_digest_lines` and `parse_mac_digest_output` (`KEY=<name> DIGEST=<64-hex> SOURCE=file|keychain`).
  3. Implement `diff_env_scope` and `EnvStatusReport` (`Synced`, `ValueDrift`, `MissingOnMac`, `MissingOnWindows`, `MissingBoth`, `UndeclaredExtra`), verifying serialized JSON contains zero values and zero digests.
- **Tests** in `src/env_secrets.rs`:
  - `sha256_and_hmac_sha256_match_nist_and_rfc_4231_vectors`
  - `entry_hmac_separates_key_and_value_boundaries_and_changes_with_nonce`
  - `classifies_synced_drifted_missing_and_undeclared_keys_without_serializing_digests`
- **Acceptance:** `cargo test -p cheap-otto --lib env_secrets::tests` passes.

### T1.3 Vault reference parser, SSH/Keychain command specs, output redactor & worktree guard

- [ ] Open
- **Covers:** R2.2, R3.1, R3.3, R3.4, R4.4, R5.2
- **Depends on:** T1.1, T1.2
- **Files:** `src/env_secrets.rs`
- **Steps:**
  1. Implement `SecretRef` (`Op`, `Gcloud`, `Keychain`) parser and `build_vault_pull_spec(&SecretRef)` using discrete argument arrays and rejecting option-like (`-`) or metacharacter segments. The keychain grammar is `keychain:<account>@<service>`, split at the first `@`, with `account` a safe identifier and `service` `/`-separated segments of `[A-Za-z0-9._-]` without a leading `-`; the canonical service form everywhere is `com.cheapotto.env/<project>/<profile>[/<subpath>]`, and for `env set --keychain` the account is the variable name.
  2. Implement `build_mac_digest_probe_spec` (streaming `NONCE=<hex>` via `CommandSpec::with_stdin` over `ssh -o ClearAllForwardings=yes -o BatchMode=yes -o ConnectTimeout=10 -p <port> -- <user>@<host> ~/.cargo/bin/cheap-otto env mac-digest --project <p> --profile <prof> [--subpath <s>]` with validated identifiers and no `sh -s`), `build_remote_set_tty_spec` (`ssh -t`), `build_remote_pull_spec`, and `render_security_stdin_upsert` (`/usr/bin/security -i` stdin command with zero `-w` argv) which fails closed with an `EnvError` variant on a secret containing NUL, CR, LF, a double quote or a backslash (those values use the `0600` file store) and on `service` or `account` outside the safe charset.
  3. Implement single-pass longest-first `redact_secret_values` (covering raw, URL-encoded and JSON-escaped variants of every non-empty loaded value, with no length, boolean or integer exemption) and `find_forbidden_worktree_env_paths` flagging `.env` and `.env.*` at any depth except `.env.example`.
- **Tests** in `src/env_secrets.rs`:
  - `parses_vault_references_and_rejects_flag_or_shell_injection`
  - `keychain_refs_round_trip_account_at_service_and_reject_missing_at_second_at_and_leading_dash`
  - `digest_probe_streams_nonce_over_stdin_and_keeps_argv_secret_free` (asserts the remote command is `env mac-digest` and the argv contains no `sh -s`)
  - `keychain_upsert_uses_security_stdin_and_never_passes_secret_in_argv`
  - `keychain_upsert_rejects_nul_cr_lf_double_quote_and_backslash_secrets_and_unsafe_service_or_account` (one case per rejected character)
  - `redacts_every_nonempty_value_longest_first_including_short_and_encoded_forms`
  - `worktree_hygiene_blocks_dotenv_variants_including_ignored_entries_while_allowing_dotenv_example` (covers a `!! .env` ignored entry)
- **Acceptance:** `cargo test -p cheap-otto --lib env_secrets::tests` passes.

## Phase 2: store, broker & bridge CLI (`src/main.rs`, `src/bridge.rs`)

### T2.1 Atomic `0600` store, `mac-digest`, `set-local`, `pull --local` & `env run` broker

- [ ] Open
- **Covers:** R3.2, R3.3, R3.4, R4.1, R5.1, R5.2
- **Depends on:** T1.3
- **Files:** `src/main.rs`
- **Steps:**
  1. Implement the stores in one step. Workstation status path: resolve the state dir via `--state-dir`, `./.cheap-otto`, repo-root `.cheap-otto`, then `~/.cheap-otto`, take `env.lock` in the resolved dir, and write `env-status.json` through a PID-named temp with `sync_all()`, atomic rename and cleanup on failure; Windows semantics are separate (no Unix mode, inherited per-user ACL, `std::fs::rename` replaces the target). Mac secret store: create `0700` directories, use `~/.cheap-otto/env/env.lock`, and write through a PID-named temp opened with `OpenOptionsExt::mode(0o600)` under `cfg(unix)`, then `sync_all()`, atomic rename and cleanup on failure.
  2. Wire `cheap-otto env mac-digest` (reads `NONCE=<hex>` from `stdin`, prints `KEY=... DIGEST=... SOURCE=...`), `env set-local` (hidden `/dev/tty` read with echo-restore guard, or `security -i` + `keychain.json` on macOS), `env pull --local [--execute]` / `env pull-local`, and operator-only `env run --project <p> [--profile <prof>] [--subpath <s>] -- <program> [args...]` (rejects non-operator invocations before resolving secrets by comparing the effective UID from `/usr/bin/id -u` with discrete args to the `MetadataExt::uid` of `~/.cheap-otto/env`, via a pure decision function taking both UIDs as inputs because `forbid(unsafe_code)` rules out `libc::getuid`; then injects validated env via `Command::envs` and streams stdout/stderr through `redact_secret_values`).
- **Tests** in `tests/cli_integration.rs` and `src/main.rs`:
  - `env_store_writes_mode_0600_atomically_and_mac_digest_emits_no_values`
  - `env_run_injects_scoped_vars_without_argv_leak_and_redacts_output`
  - `env_run_rejects_non_operator_uid_before_reading_the_store`
- **Acceptance:** `cargo test -p cheap-otto --test cli_integration env_` passes.

### T2.2 Workstation bridge CLI (`schema`, `diff`, `status`, `set --remote`, `pull --remote`, `exec --with-env`)

- [ ] Open
- **Covers:** R2.2, R2.4, R2.5, R3.1, R3.4, R4.1, R4.4, R5.1
- **Depends on:** T2.1
- **Files:** `src/main.rs`, `src/bridge.rs`, `src/worktree.rs`, `src/agent_session.rs`, `src/schedule.rs`, `src/server.rs`
- **Steps:**
  1. Wire `cheap-otto env schema`, `env diff`, `env status`, `env set --remote`, and `env pull --remote`, ensuring dry runs (no `--execute`) perform zero network, subprocess or state-file writes and print zero secret values.
  2. Add optional `--with-env [=<profile>]` and `--subpath <s>` to `cheap-otto exec` (wrapping the remote command in `~/.cargo/bin/cheap-otto env run`), keeping `delegate` and `ask` strictly free of `--with-env`.
  3. Add the pre-commit worktree `.env*` check (`git status --porcelain -uall --ignored=matching`, or a complete filesystem scan, inspecting untracked, modified and ignored paths) using `find_forbidden_worktree_env_paths` rules at every commit gate: the delegated commit snippet in `src/worktree.rs`, the follow-up commit snippet in `src/agent_session.rs`, the scheduled agent commit path in `src/schedule.rs`, and the server-side delegate/follow-up handlers in `src/server.rs` (wired through `src/main.rs` and `src/bridge.rs`). If T2.2 exceeds the 600-code-line budget it MUST be split per `specs/README.md`, updating the PR plan table, README counts and status line, and each `Depends on:` in the same commit.
- **Tests** in `tests/cli_integration.rs`:
  - `env_dry_runs_open_no_socket_write_no_files_and_print_no_secret_values`
  - `exec_with_env_wraps_remote_broker_while_delegate_rejects_with_env`
  - `delegated_commit_gate_blocks_dotenv_including_ignored_entries`
  - `scheduled_commit_gate_blocks_dotenv_including_ignored_entries`
  - `follow_up_commit_gate_blocks_dotenv_including_ignored_entries`
- **Acceptance:** `cargo test -p cheap-otto --test cli_integration` passes.

## Phase 3: Mission Control visibility (`src/server.rs`, `src/ui_assets.rs`)

### T3.1 `GET /api/env` endpoint & Mission Control env readiness card

- [ ] Open
- **Covers:** R5.3
- **Depends on:** T1.3
- **Files:** `src/server.rs`, `src/ui_assets.rs`
- **Steps:**
  1. Add `GET /api/env` behind `RequestGuard` reading `.cheap-otto/env-status.json` under `ServeConfig.state_dir` (`null` when missing or corrupt), returning only project, profile, subpath, timestamp and per-key `EnvKeySyncState` entries (never reading `~/.cheap-otto/env/` or probing SSH).
  2. Render a compact Env Sync card in Mission Control (`src/ui_assets.rs`) showing synced, drifted and missing key counts.
- **Tests** in `src/server.rs`:
  - `api_env_returns_cached_redacted_status_and_never_exposes_values_or_digests`
  - `api_env_rejects_rebound_host_via_request_guard`
- **Acceptance:** `cargo test -p cheap-otto --lib server::tests` passes.

## Phase 4: documentation (`README.md`, `CLAUDE.md`, `.claude/rules/`, `docs/`)

### T4.1 User, security & agent documentation

- [ ] Open
- **Covers:** R2.1, R2.5, R4.1, R4.3
- **Depends on:** T2.2, T3.1
- **Files:** `README.md`, `CLAUDE.md`, `.claude/rules/02-security-and-appliance-safety.md`, `.claude/rules/05-crate-ecosystem-and-cli-patterns.md`, `docs/index.html`
- **Steps:**
  1. Document `cheap-otto env schema|diff|status|set|pull|run` and `exec --with-env` in `README.md` and `docs/index.html` (Quickstart), verifying every command runs as written against `--help`.
  2. Add `~/.cheap-otto/env/` (`0700` / `0600`), `.cheap-otto/env-status.json`, and the zero-wire / zero-agent secret invariants to `CLAUDE.md` and rules `02` and `05`.
- **Acceptance:** `cargo test --workspace --all-targets --locked` passes (`governance_and_cubic` budgets remain under cap).

## Phase 5: LIVE verification (owner only)

### T5.1 Interactive TTY provisioning, zero-wire diff & scrubbed `exec --with-env`

- [ ] Open
- **Covers:** R2.1, R2.2, R3.1, R3.2, R3.3, R5.1, R5.2, R5.3
- **Depends on:** PRs 1–7 and `m1 sync`
- **LIVE**
- **Steps and expected results:**
  1. Deploy with `cargo run -p code-reviewer -- m1 sync`.
  2. Run `cheap-otto env set DEMO_SECRET --remote --execute`, enter a 16-char test token at the hidden Mac TTY prompt, and verify `stat -f '%Lp' ~/.cheap-otto/env/<project>/dev/tree/.env` on the Mac prints `600`.
  2a. Run `cheap-otto env set DEMO_KC --remote --keychain --execute` with a token and confirm the `security -i` stdin grammar on the Mac: the item is stored under service `com.cheapotto.env/<project>/dev` with account `DEMO_KC`, and a secret containing a double quote or backslash is rejected by `render_security_stdin_upsert` before any command is emitted (use the `0600` file store for it instead).
  3. Run `cheap-otto env diff --execute` from Windows; verify `DEMO_SECRET` reports `MissingOnWindows` (or `Synced` once set locally) and `.cheap-otto/env-status.json` contains neither the test token nor any hex digest.
  4. Run `cheap-otto exec --with-env --cmd 'printf "%s\n" "$DEMO_SECRET"' --execute` and verify the printed line is `[REDACTED:DEMO_SECRET]`.

### T5.2 Agent worktree isolation & commit hygiene on the real Mac

- [ ] Open
- **Covers:** R4.1, R4.2, R4.3, R4.4
- **Depends on:** T5.1
- **LIVE**
- **Steps and expected results:**
  1. Delegate a read-only inspection task (`cheap-otto delegate ... --execute`) and verify its worktree contains `.env.example` and zero `.env` / `.env.*` files, and its child `env` omits `DEMO_SECRET`.
  2. Verify `~/.cheap-otto/env` has mode `700` owned by the operator UID so another local user gets `Permission denied`.
