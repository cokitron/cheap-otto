# Design: Independent Discord chat controls for `claude` and `agy`

## 1. Why Discord + zero-dependency Node 24 sidecar + Rust core

1. **Free-tier durability:** Discord allows unlimited free bot identities (`@Claude` + `@Agy` in one private server), permanent thread history (unlike Slack Free's 90-day wipe and 10-app cap), and outbound-only WSS Gateway connections (`wss://gateway.discord.gg/?v=10&encoding=json`).
2. **Dependency hygiene (`.claude/rules/05-crate-ecosystem-and-cli-patterns.md`):** `cheap-otto` forbids adding HTTP/TLS/regex crates to `Cargo.toml`. Node 24 (already provisioned on `otto-mac` at `/opt/homebrew/opt/node@24/bin/node`) ships standard-library `WebSocket` and `fetch`. A ~140-line embedded JS sidecar (`render_discord_sidecar_js()`) handles only the Discord WSS heartbeat/reconnect and REST message calls, delegating every inbound event to `cheap-otto chat dispatch --json` (or `127.0.0.1:7420/api/chat/inbound`) so **all security, allowlisting, worktree planning, session state and harness command construction live in pure unit-tested Rust (`src/chat.rs`)**.

## 2. End-to-end topology

```text
Phone / Web / Desktop Discord App
  │  #web-app (mapped to <gh-account>/web-app)
  │   └── Thread "feat-passkeys"
  │         ├── "@Claude build WebAuthn verify + unit tests"
  │         └── "@Agy build passkey UI component & polish states"
  ▼ (Outbound WSS Gateway v10 — 0 open ports on Mac)
com.cheapotto.chat.discord (Node 24 stdlib WebSocket + fetch sidecar)
  │  spawns `cheap-otto chat dispatch` only for allowlisted authors (env stripped of bot/GH tokens)
  ▼
src/chat.rs + ~/.cheap-otto/chat-threads.json (registry lock, per-worktree queue tickets)
  ├── claude -p ... --session-id/--resume <claude_uuid> --output-format stream-json
  └── agy    -p ... --conversation <agy_conv_id>        --output-format stream-json
        │
        ▼ CWD: ~/repos/<gh-account>/.worktrees/web-app--feat-passkeys/
          Git identity & HTTPS auth via includeIf -> git-credential-otto-gh <gh-account>
```

## 3. Pure Rust module (`src/chat.rs`) [R1, R2, R3, R4, R5]

`src/chat.rs` is pure (no I/O, no clock, `#![forbid(unsafe_code)]`):

- **Configuration (`ChatConfig`, `~/.cheap-otto/chat-config.json`):** [R2.2, R3.1]
  - `allowed_user_ids: Vec<String>` (non-empty numeric Discord snowflake IDs; empty list refuses all messages fail-closed)
  - `claude_bot_user_id: Option<String>`, `agy_bot_user_id: Option<String>`
  - `repos_root: String` (default `"~/repos"`)
  - `dm_route: Option<ChannelRepoRoute>` (repo used for DMs; absent means a DM with several or no bindings gets a help reply and creates nothing) [R2.3]
  - `max_parallel_chat_turns: u8` (default `3`)
  - `channel_routes: BTreeMap<String, ChannelRepoRoute>` (`channel_id -> { gh_account, repo_name, default_base_ref }`)
- **Inbound routing (`route_inbound_message`):** [R2.2, R2.3, R2.4, R2.5, R4.3]
  - `InboundChatEvent { platform_bot: ChatBotTarget, message_id, channel_id, thread_id: Option<String>, author_id, content, mentioned_ids: Vec<String> }`
  - Returns `ChatRouteDecision`:
    - `RejectedUnauthorized { author_id }` when `author_id` is not in `allowed_user_ids`
    - `RejectedAmbiguousMention` when both `@Claude` and `@Agy` are mentioned [R2.6]
    - `DuplicateDelivery { message_id }` when `message_id` is already in the dedup record passed in as input [R2.7]
    - `IgnoreSelfOrOtherBot` when neither bot is addressed in a multi-bot channel message meant strictly for the other bot (prevents double-execution when both `@Claude` and `@Agy` sockets see the same channel event!)
    - `BuiltInCommand(ChatBuiltInCommand)` (`Status`, `Diff`, `SetModel`, `SetEffort`, `ListRepos`, `CloneRepo`, `Stop`)
    - `RunAgentTurn { runner: AgentRunner, create_thread_slug: Option<String>, clean_prompt: String }`
- **Double-socket deduplication rule:** [R2.3, R2.4, R2.5]
  - Both `@Claude` and `@Agy` bots sit in the same server so each posts under its own avatar/name. When a message mentions `@Agy`, ONLY the `agy` bot socket handles it; when it mentions `@Claude`, ONLY the `claude` bot socket handles it; when unmentioned inside an active thread, ONLY `last_active_runner` (default `claude`) handles it.
- **Shared persistence & coordination (one mechanism, referenced elsewhere):** [R1.4, R2.7, R3.2, R3.4, R5.2]
  `src/chat.rs` only computes; the `chat dispatch` shell in `src/main.rs` owns lock files, slot files, temp-file rename and timestamps, and passes timestamps and file contents in as parameters.
  - **Registry lock:** every read-modify-write of `chat-threads.json` runs under one registry-wide lock and persists via write-to-temp-file then atomic rename, so no binding is lost or torn.
  - **Slug allocation:** under that lock, a pure `allocate_thread_slug(base_slug, thread_id, &registry)` appends a unique thread-derived suffix and rejects a collision before the binding is created; the binding is first persisted with `status: Pending` and only marked `Ready` once the worktree exists, so a failed provisioning can be rolled back (`src/chat.rs` stays pure and only returns the transitions and the rollback plan; the shell applies them and passes in the timestamp used to reap stale `Pending` bindings) [R3.2]; the branch is `feat/<slug>-<unique-suffix>`. Components pass `validate_safe_identifier` (`src/bridge.rs:159`, alphanumerics, `-`, `_`, `.` only, so slashes are rejected); `/` appears only in the fixed `feat/` prefix.
  - **Dedup record:** a bounded list of recent Discord `message_id`s kept in the registry file and checked under the same lock before dispatch.
  - **Queue tickets:** under the same lock each turn takes the next per-worktree sequence number; a turn runs only when its ticket is the lowest outstanding for that worktree, giving strict arrival order (a bare lockfile would not).
  - **Slots:** before spawning an agent a turn claims one of `max_parallel_chat_turns` slot files; if none is free it waits, keeping its ticket.
  - **Credential isolation:** agent turns run under a separate macOS user that cannot read `~/.cheap-otto/tokens/` or `<accounts.tokens_dir>` and cannot write the worktree's `.git` pointer, gitdir, hooks, or broker state; `chat dispatch` validates the operator-owned canonical remote and runs brokered `git push`/`gh` with operator-controlled Git config and hooks [R4.1]. Open question, default: the agent user gets group-write access to the working tree only, with `.git`/gitdir/hooks/broker state operator-owned and read-only to the agent.
- **Thread binding ledger (`ChatThreadBinding`, `~/.cheap-otto/chat-threads.json`):** [R3.2, R3.3]
  - `thread_id: String`
  - `status: ChatBindingStatus { Pending, Ready }` (turns run only on `Ready`; a failed provisioning removes the `Pending` binding and any partial branch or worktree)
  - `gh_account: String`, `repo_name: String`, `branch_name: String`, `worktree_dir: String`
  - `claude_session_id: Option<String>` (pre-populated via `generate_session_uuid()`, `claude_turns: u32`)
  - `agy_conversation_id: Option<String>` (captured from `agy` `stream-json` metadata or `--continue` in isolated worktree)
  - `model_override: Option<String>`, `effort_override: Option<ReasoningEffort>`
  - `last_active_runner: AgentRunner`
  - `latest_turns: BTreeMap<runner, ChatTurnSummary { turn_seq: u32, status, summary, finished_at }>` (`finished_at` is a string supplied by the shell; this is the source for `GET /api/chat/threads`) [R5.2]
  - `queue_next_seq: u32`, `recent_message_ids` (bounded dedup record, registry-level)
- **Multi-account repo hub & LaunchAgent renderers:** [R1.1, R1.2, R4.1, R5.1]
  - `render_repo_hub_setup_script(accounts: &[String]) -> Result<String, ChatError>`: validates every account with `validate_safe_identifier` before rendering (rejecting metacharacters, `..` and slashes) and idempotently creates `~/repos/<acct>/.worktrees`, `~/.local/bin/git-credential-otto-gh` (reads `<accounts.tokens_dir>/<acct>.token`, the key used at `crates/code-reviewer/src/github.rs:183-188`; `~/.code-reviewer/tokens` is only the default), `~/.local/bin/otto-repo`, and `~/.gitconfig` `includeIf` blocks.
  - `chunk_discord_message(text: &str, limit: usize) -> Vec<String>`: splits long agent responses at newline / code-fence boundaries (`<= 1900` chars), closing and reopening ` ``` ` fences cleanly across chunks.
  - `render_discord_sidecar_js()` & `render_chat_plist_template()`: renders `~/.cheap-otto/bin/otto-discord-sidecar.mjs` and `~/Library/LaunchAgents/com.cheapotto.chat.discord.plist` (`RESERVED_CHAT_PROJECT_NAME = "chat"`).

## 4. CLI & Mission Control surfaces (`src/main.rs`, `src/server.rs`)

- `cheap-otto chat setup --allow-user <discord_id> [--map <channel_id>=<acct>/<repo>]... [--execute]` — writes `chat-config.json`, provisions `~/repos/`, and installs tokens (`0600`) in `~/.cheap-otto/tokens/{discord-claude,discord-agy}.token`.
- `cheap-otto chat install|uninstall|status|restart [--execute]` — manages `com.cheapotto.chat.discord` on the Mac via SSH stdin scripts (dry-run by default).
- `cheap-otto chat dispatch` — internal synchronous handler invoked on the Mac by the sidecar, after the sidecar's own `allowed_user_ids` prefilter, for each message (re-validates fail-closed, takes the registry lock, queue ticket and slot described in section 3, allocates the slug and persists the binding as `Pending`, runs `otto-repo worktree` if needed, then marks the binding `Ready` (on error it rolls back the pending binding and any partial branch or worktree) [R3.2], invokes `claude` or `agy` with env tokens stripped and under the separate agent user, validating the canonical remote and brokering each `git push`/`gh` itself with operator-controlled Git config and hooks [R4.1]; streams summary chunks back to stdout).
- `GET /api/chat/threads` in `src/server.rs` — guarded by `RequestGuard`, surfaces active chat worktrees and turns in Mission Control (`:7420`).

## 5. Discord Developer Portal one-time setup (free, ~3 minutes)

1. At `https://discord.com/developers/applications`, create two free apps: **`Claude Code`** and **`Antigravity`**.
2. In each app -> **Bot**: enable **Message Content Intent**, reset token, save into `~/.cheap-otto/tokens/discord-claude.token` and `discord-agy.token` (`chmod 600`), and invite both to your private Discord server with `View Channels`, `Send Messages`, `Send Messages in Threads`, `Create Public Threads`, `Read Message History`.
