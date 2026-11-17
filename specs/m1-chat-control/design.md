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
  │  spawns `cheap-otto chat dispatch` (env stripped of bot/GH tokens)
  ▼
src/chat.rs + ~/.cheap-otto/chat-threads.json (per-worktree FIFO lock)
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
  - `max_parallel_chat_turns: u8` (default `3`)
  - `channel_routes: BTreeMap<String, ChannelRepoRoute>` (`channel_id -> { gh_account, repo_name, base_ref }`)
- **Inbound routing (`route_inbound_message`):** [R2.2, R2.3, R2.4, R2.5, R4.3]
  - `InboundChatEvent { platform_bot: ChatBotTarget, channel_id, thread_id: Option<String>, author_id, content, mentioned_ids: Vec<String> }`
  - Returns `ChatRouteDecision`:
    - `RejectedUnauthorized { author_id }` when `author_id` is not in `allowed_user_ids`
    - `IgnoreSelfOrOtherBot` when neither bot is addressed in a multi-bot channel message meant strictly for the other bot (prevents double-execution when both `@Claude` and `@Agy` sockets see the same channel event!)
    - `BuiltInCommand(ChatBuiltInCommand)` (`Status`, `Diff`, `SetModel`, `SetEffort`, `ListRepos`, `CloneRepo`, `Stop`)
    - `RunAgentTurn { runner: AgentRunner, create_thread_slug: Option<String>, clean_prompt: String }`
- **Double-socket deduplication rule:** [R2.3, R2.4, R2.5]
  - Both `@Claude` and `@Agy` bots sit in the same server so each posts under its own avatar/name. When a message mentions `@Agy`, ONLY the `agy` bot socket handles it; when it mentions `@Claude`, ONLY the `claude` bot socket handles it; when unmentioned inside an active thread, ONLY `last_active_runner` (default `claude`) handles it.
- **Thread binding ledger (`ChatThreadBinding`, `~/.cheap-otto/chat-threads.json`):** [R3.2, R3.3]
  - `thread_id: String`
  - `gh_account: String`, `repo_name: String`, `branch_name: String`, `worktree_dir: String`
  - `claude_session_id: Option<String>` (pre-populated via `generate_session_uuid()`, `claude_turns: u32`)
  - `agy_conversation_id: Option<String>` (captured from `agy` `stream-json` metadata or `--continue` in isolated worktree)
  - `model_override: Option<String>`, `effort_override: Option<ReasoningEffort>`
  - `last_active_runner: AgentRunner`
- **Multi-account repo hub & LaunchAgent renderers:** [R1.1, R1.2, R4.1, R5.1]
  - `render_repo_hub_setup_script(accounts: &[String]) -> Result<String, ChatError>`: idempotently creates `~/repos/<acct>/.worktrees`, `~/.local/bin/git-credential-otto-gh`, `~/.local/bin/otto-repo`, and `~/.gitconfig` `includeIf` blocks.
  - `chunk_discord_message(text: &str, limit: usize) -> Vec<String>`: splits long agent responses at newline / code-fence boundaries (`<= 1900` chars), closing and reopening ` ``` ` fences cleanly across chunks.
  - `render_discord_sidecar_js()` & `render_chat_plist_template()`: renders `~/.cheap-otto/bin/otto-discord-sidecar.mjs` and `~/Library/LaunchAgents/com.cheapotto.chat.discord.plist` (`RESERVED_CHAT_PROJECT_NAME = "chat"`).

## 4. CLI & Mission Control surfaces (`src/main.rs`, `src/server.rs`)

- `cheap-otto chat setup --allow-user <discord_id> [--map <channel_id>=<acct>/<repo>]... [--execute]` — writes `chat-config.json`, provisions `~/repos/`, and installs tokens (`0600`) in `~/.cheap-otto/tokens/{discord-claude,discord-agy}.token`.
- `cheap-otto chat install|uninstall|status|restart [--execute]` — manages `com.cheapotto.chat.discord` on the Mac via SSH stdin scripts (dry-run by default).
- `cheap-otto chat dispatch` — internal synchronous handler invoked on the Mac by the sidecar for each validated message turn (acquires per-worktree lockfile, runs `otto-repo worktree` if needed, invokes `claude` or `agy` with env tokens stripped, streams summary chunks back to stdout).
- `GET /api/chat/threads` in `src/server.rs` — guarded by `RequestGuard`, surfaces active chat worktrees and turns in Mission Control (`:7420`).

## 5. Discord Developer Portal one-time setup (free, ~3 minutes)

1. At `https://discord.com/developers/applications`, create two free apps: **`Claude Code`** and **`Antigravity`**.
2. In each app -> **Bot**: enable **Message Content Intent**, reset token, save into `~/.cheap-otto/tokens/discord-claude.token` and `discord-agy.token` (`chmod 600`), and invite both to your private Discord server with `View Channels`, `Send Messages`, `Send Messages in Threads`, `Create Public Threads`, `Read Message History`.
