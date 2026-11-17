# Tasks: Independent Discord chat controls for `claude` and `agy`

Read `../README.md`, `requirements.md` and `design.md` before starting any task. Every PR stays within 600 changed lines (`Cargo.lock` excluded), introduces zero HTTP/TLS/regex Cargo crates, and passes `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace --all-targets --locked`.

## PR plan

| PR | Tasks | Branch | Needs |
| --- | --- | --- | --- |
| 1 | T1.1 chat config, allowlist & dual-bot router | `feat/m1-chat-control-t1-1-router` | nothing |
| 2 | T1.2 thread-to-worktree ledger & Discord chunker | `feat/m1-chat-control-t1-2-threads` | PR 1 |
| 3 | T1.3 multi-account `~/repos` hub & `launchd` templates | `feat/m1-chat-control-t1-3-repo-hub` | PR 1 |
| 4 | T1.4 zero-dep Node 24 Discord Gateway sidecar renderer | `feat/m1-chat-control-t1-4-sidecar` | PRs 2, 3 |
| 5 | T2.1 `chat setup|install|uninstall|status|restart` CLI | `feat/m1-chat-control-t2-1-cli` | PR 4 |
| 6 | T2.2 `chat dispatch` registry lock, queue tickets & env-stripped runner | `feat/m1-chat-control-t2-2-dispatch` | PR 5 |
| 7 | T3.1 Mission Control `GET /api/chat/threads` & docs | `feat/m1-chat-control-t3-1-hub-docs` | PR 6 |

After PR 7, the owner runs **LIVE** tasks T4.1 and T4.2 against the paired MacBook and private Discord server.

## Phase 1: pure planners (`src/chat.rs`, `src/schedule.rs`)

### T1.1 Chat config, fail-closed allowlist and dual-bot message router

- [ ] Open
- **Covers:** R1.3, R2.2, R2.3, R2.4, R2.5, R2.6, R2.7, R3.1, R4.3
- **Depends on:** nothing
- **Files:** `src/chat.rs` (new), `src/lib.rs`
- **Steps:**
  1. Create `src/chat.rs` with `ChatError`, `ChatBotTarget` (`Claude`, `Agy`), `ChannelRepoRoute`, `ChatConfig`, `InboundChatEvent`, `ChatBuiltInCommand`, and `ChatRouteDecision`.
  2. Implement fail-closed `route_inbound_message` (rejects empty or non-matching `allowed_user_ids`, rejects a message mentioning both bots, drops a `message_id` already in the dedup record, routes DMs through `dm_route`, deduplicates events across the `@Claude` and `@Agy` bot sockets so each message executes at most once, and parses `!status`, `!diff`, `!model`, `!effort`, `!repos`, `!clone`, `!stop`).
  3. Export `pub mod chat;` in `src/lib.rs`.
- **Tests** in `src/chat.rs`:
  - `rejects_unallowlisted_and_empty_allowlist_users_fail_closed`
  - `deduplicates_dual_bot_events_for_claude_agy_and_followup_turns`
  - `rejects_dual_mention_and_redelivered_message_id`
  - `routes_dm_through_dm_route_or_replies_without_creating_worktree`
  - `parses_builtin_bang_commands_without_invoking_llm`
  - `rejects_clone_repo_with_extra_slashes_and_unsafe_owner_or_repo`
- **Acceptance:** `cargo test -p cheap-otto --lib chat::tests` passes.

### T1.2 Thread binding ledger, harness invocation builder and code-fence chunker

- [ ] Open
- **Covers:** R1.4, R3.2, R3.3, R4.1, R4.2, R5.2
- **Depends on:** T1.1
- **Files:** `src/chat.rs`
- **Steps:**
  1. Add `ChatThreadBinding` (`thread_id`, `gh_account`, `repo_name`, `branch_name`, `worktree_dir`, `status: ChatBindingStatus { Pending, Ready }`, `claude_session_id`, `agy_conversation_id`, `last_active_runner`, `latest_turns`) and `ChatThreadRegistry` (with the bounded dedup record and queue sequence counters), plus the pure `allocate_thread_slug` that appends a unique thread-derived suffix and rejects collisions.
  2. Build `HarnessRunConfig` turns (`src/harness.rs`) for `ClaudeCode` (`--session-id` on turn 1, `--resume` thereafter) and `Antigravity` (`--conversation` when known), with explicit list of stripped env var names (`DISCORD_CLAUDE_TOKEN`, `DISCORD_AGY_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`).
  3. Implement `chunk_discord_message(&str, usize)` preserving balanced ` ``` ` code blocks across `<= 1900`-char chunks.
- **Tests** in `src/chat.rs`:
  - `preserves_separate_claude_and_agy_sessions_on_shared_worktree`
  - `chunks_long_markdown_and_reopens_fenced_code_blocks`
  - `allocates_unique_slug_per_thread_and_rejects_collisions`
  - `pending_binding_is_not_runnable_and_rolls_back_cleanly`
  - `child_env_strips_discord_and_github_tokens_and_keeps_unrelated_vars`
- **Acceptance:** `cargo test -p cheap-otto --lib chat::tests` passes.

### T1.3 Multi-account `~/repos/<gh-account>` script & reserved `chat` LaunchAgent

- [ ] Open
- **Covers:** R1.1, R1.2, R1.4, R5.1
- **Depends on:** T1.1
- **Files:** `src/chat.rs`, `src/schedule.rs`
- **Steps:**
  1. Implement `render_repo_hub_setup_script(&[String])`, validating every account with `validate_safe_identifier` before rendering, and generating `~/repos/<acct>/.worktrees`, `~/.local/bin/git-credential-otto-gh` (reading `<accounts.tokens_dir>/<acct>.token`), `~/.local/bin/otto-repo`, and idempotent `~/.gitconfig` `includeIf` blocks.
  2. Add `RESERVED_CHAT_PROJECT_NAME = "chat"` (`com.cheapotto.chat.discord`), guard it in `src/schedule.rs` alongside `dashboards`, and implement `render_chat_plist_template()`.
- **Tests:**
  - `renders_idempotent_include_if_repo_hub_and_rejects_reserved_chat_schedule`
  - `rejects_unsafe_account_names_in_repo_hub_script`
- **Acceptance:** `cargo test -p cheap-otto --lib` passes.

### T1.4 Zero-dependency Node 24 Discord Gateway sidecar & install/status scripts

- [ ] Open
- **Covers:** R2.1, R2.2, R4.2, R5.1
- **Depends on:** T1.2, T1.3
- **Files:** `src/chat.rs`
- **Steps:**
  1. Implement `render_discord_sidecar_js()` using Node 24 built-in `WebSocket` and `fetch` (Gateway v10 `IDENTIFY`, heartbeat jitter, `RESUME`, thread creation, typing indicator, 3 s throttled progress edits, and, after an `allowed_user_ids` prefilter, spawning `cheap-otto chat dispatch --json` per allowlisted message so unauthorized events create no child process).
  2. Implement `render_chat_install_script`, `render_chat_uninstall_script`, and `render_chat_status_script` + `parse_chat_status`.
- **Tests:** `sidecar_and_install_scripts_use_builtin_node_and_loopback_only`
- **Acceptance:** `cargo test -p cheap-otto --lib chat::tests` passes.

## Phase 2: CLI & worktree turn execution (`src/main.rs`)

### T2.1 `cheap-otto chat setup|install|uninstall|status|restart`

- [ ] Open
- **Covers:** R3.1, R5.1
- **Depends on:** T1.4
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:** Wire dry-run-by-default `chat` lifecycle subcommands gated by `--execute` / `--apply`, streaming remote scripts over SSH stdin (`PeerNode::build_ssh_stdin_spec`).
- **Tests:** `chat_cli_lifecycle_prints_plan_without_execute`
- **Acceptance:** `cargo test --test cli_integration chat_cli` passes.

### T2.2 `cheap-otto chat dispatch` with registry lock, queue tickets, slots & secret stripping

- [ ] Open
- **Covers:** R1.4, R2.2, R2.7, R3.2, R3.4, R4.1, R4.3, R5.2
- **Depends on:** T2.1
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:** Implement synchronous `chat dispatch` on the Mac: validate event via `route_inbound_message`, re-validate the author fail-closed, then under the registry lock (design.md section 3) check the dedup record, allocate the unique slug, take a per-worktree queue ticket and persist the binding as `Pending` via atomic temp-file rename; wait for the lowest ticket on that worktree and for a free `max_parallel_chat_turns` slot; provision `otto-repo worktree` on new threads, then mark the binding `Ready` (on failure roll back the pending binding and any partial branch or worktree); strip secret env vars and run `claude` or `agy` under the separate agent user, after each turn, as the operator, verify HEAD is the thread's `feat/<slug>-<unique-suffix>` branch and, only if the tree changed, broker `git add -A` and `git commit` with operator-controlled Git config (empty operator-owned `core.hooksPath`, operator `user.name`/`user.email`, no repo-local config); validating the canonical remote and brokering `git push`/`gh` in the dispatch process with operator-controlled Git config and hooks, with agent-side read-only git run under `GIT_OPTIONAL_LOCKS=0`; record `latest_turns`; and emit chunked JSON reply frames. Timestamps and lock/slot files live here, not in `src/chat.rs`.
- **Tests:**
  - `chat_dispatch_rejects_unallowlisted_user_and_executes_builtin_status`
  - `chat_dispatch_serializes_same_worktree_turns_in_ticket_order`
  - `chat_dispatch_concurrent_threads_get_unique_branches_and_keep_all_bindings`
  - `chat_dispatch_never_exceeds_max_parallel_chat_turns`
  - `chat_dispatch_rolls_back_pending_binding_when_worktree_provisioning_fails` (stub `otto-repo` exiting non-zero, temp state dir)
  - `chat_dispatch_agent_child_env_has_no_tokens` (temp dirs, stub agent binary, no Mac, no network)
  - `chat_dispatch_broker_rejects_noncanonical_remote_before_push` (temp dirs, stub agent binary, no Mac, no network)
  - `chat_dispatch_broker_commits_agent_edits_on_thread_branch_with_operator_hooks` (temp dirs, stub agent binary, a repo-local hook that must not run; assert the commit is on the binding's branch with the operator identity, a clean tree makes no commit, and the gitdir is untouched by the agent)
- **Note:** the separate-user boundary needs a real macOS user and is verified only by LIVE T4.1.
- **Acceptance:** `cargo test --test cli_integration chat_dispatch` passes.

## Phase 3: Mission Control visibility & docs

### T3.1 Mission Control `GET /api/chat/threads` and operator guide

- [ ] Open
- **Covers:** R5.2
- **Depends on:** T2.2
- **Files:** `src/server.rs`, `README.md`, `CLAUDE.md`, `AGENTS.md`
- **Steps:** Expose `GET /api/chat/threads` behind `RequestGuard` in `src/server.rs` and document Discord bot creation + `~/repos/<gh-account>/` workflow in `README.md` / `CLAUDE.md`.
- **Tests:** `api_chat_threads_returns_bound_worktree_sessions` (asserts the `latest_turns` fields)
- **Acceptance:** All three workspace CI gates pass.

## Phase 4: LIVE rollout (owner-approved)

### T4.1 LIVE: Deploy `@Claude` and `@Agy` Discord bots on `otto-mac`

- [ ] Open (**LIVE** — owner approval required)
- **Covers:** R1.2, R2.1, R2.3, R2.4, R3.3, R4.1
- **Acceptance:** From the Discord mobile/desktop app, posting `@Claude` in a mapped repo channel opens a thread + `~/repos/<acct>/.worktrees/<repo>--<slug>` worktree and replies with the turn summary; posting `@Agy` in that same thread continues work on the exact same branch/worktree under `@Agy`'s identity; a second simultaneous thread in another channel runs in parallel with zero Git credential collisions, and an agent turn that tries to read `~/.cheap-otto/tokens/` or `<accounts.tokens_dir>` is denied by the separate agent user, as is an agent turn that tries to write `.git` or hooks.

### T4.2 LIVE: Unauthorized-user rejection & `launchd` crash recovery

- [ ] Open (**LIVE** — owner approval required)
- **Covers:** R2.2, R5.1
- **Acceptance:** Killing the Node sidecar PID restarts `com.cheapotto.chat.discord` within 10 s and resumes the Gateway session; messages from non-allowlisted IDs are ignored.
