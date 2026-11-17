# Requirements: Independent Discord chat controls for `claude` and `agy` across multi-account repos

## Introduction

`cheap-otto` already models stateful multi-turn coding sessions (`src/agent_session.rs`) and CLI harnesses for both Claude Code (`claude --resume <uuid>`) and Antigravity (`agy --conversation <id>`) in `src/harness.rs`, but driving them requires an SSH terminal or workstation tunnel. The operator wants to talk independently to `claude` and `agy` from anywhere (phone, web or desktop) for free, while running multiple complex tasks in parallel across repositories belonging to different GitHub accounts on the Mac (`~/repos/<gh-account>/`).

This spec introduces a zero-new-Cargo-dependency Discord chat control plane (`src/chat.rs` + a stdlib-only Node 24 Gateway sidecar under `launchd`) with two independent Discord bot identities (`@Claude` and `@Agy`), mapping Discord channels to `~/repos/<gh-account>/<repo>` checkouts and Discord threads to isolated parallel Git worktrees.

## Glossary

| Term | Meaning |
| --- | --- |
| Mac | The paired headless Apple Silicon MacBook (`otto-mac`) |
| Multi-account repo hub | `~/repos/<gh-account>/<repo>` base clones + `~/repos/<gh-account>/.worktrees/<repo>--<slug>` worktrees |
| Credential helper | `~/.local/bin/git-credential-otto-gh <gh-account>` wired via per-account `.gitconfig` files and `~/.gitconfig` `includeIf` rules; it reads `<accounts.tokens_dir>/<gh-account>.token` (default `~/.code-reviewer/tokens`), and only the `chat dispatch` broker invokes it (R4.1) |
| Unique suffix | Short suffix derived from the Discord thread ID and appended to the prompt-derived slug, so each thread gets a distinct branch and worktree; `<repo>--<slug>` elsewhere in this spec means the base form before the suffix |
| Registry lock | The single registry-wide lock held by `chat dispatch` around every read-modify-write of `~/.cheap-otto/chat-threads.json` (design.md section 3) |
| Bot pair | Two separate free Discord bot applications (`claude_bot` and `agy_bot`) in the operator's private Discord server |
| Gateway sidecar | A zero-npm-dependency Node 24 script (`WebSocket` + `fetch`) running under `launchd` (`com.cheapotto.chat.discord`) |
| Thread binding | Entry in `~/.cheap-otto/chat-threads.json` linking a Discord thread ID to a worktree, `claude_session_id` and `agy_conversation_id` |
| Execute gate | `--execute` (alias `--apply`); without it, mutating CLI commands only print their deterministic plan |

## Requirements

### R1. Multi-account repository hub & parallel worktree isolation (`~/repos/<gh-account>/`)

**User story:** As an operator with multiple GitHub accounts on the Mac, I want repos cloned under per-account folders with isolated Git credentials and per-task worktrees, so parallel chat tasks never collide on branches or GitHub tokens.

1. The system SHALL organize chat-managed repositories under `~/repos/<gh-account>/<repo>` and task worktrees under `~/repos/<gh-account>/.worktrees/<repo>--<slug>` (with the unique suffix appended for thread worktrees, see R1.4).
2. WHEN provisioning the repo hub THEN the system SHALL configure `[includeIf "gitdir:~/repos/<gh-account>/"]` rules pointing to per-account `.gitconfig` files that bind `user.name`, `user.email` (`<gh-account>@users.noreply.github.com`) and `credential.https://github.com.helper` to `!~/.local/bin/git-credential-otto-gh <gh-account>`, with that helper reading `<accounts.tokens_dir>/<gh-account>.token` (default `~/.code-reviewer/tokens`), without mutating global `gh auth` state.
3. The system SHALL reject `<gh-account>`, `<repo>` and `<slug>` values containing path traversal (`..`), slashes or shell metacharacters via `validate_safe_identifier` (`src/bridge.rs`).
4. WHEN two chat threads target the same `<gh-account>/<repo>` concurrently THEN each thread SHALL receive a unique branch (`feat/<slug>-<unique-suffix>`) and worktree directory, with branch/worktree allocation atomic, so file edits never overlap.

### R2. Independent `@Claude` and `@Agy` Discord bots (outbound-only, free tier)

**User story:** As the operator away from my desk, I want to DM or `@mention` `@Claude` and `@Agy` independently on Discord from any device without opening inbound firewall ports or paying for tunnel/SaaS relays.

1. The Gateway sidecar SHALL maintain outbound-only Discord Gateway v10 WebSocket connections (`wss://gateway.discord.gg/?v=10&encoding=json`) for configured bot tokens (`claude` and/or `agy`) using Node 24's built-in `WebSocket` and `fetch` APIs with zero third-party npm packages and zero new Rust HTTP/TLS crates.
2. WHEN a message arrives from a Discord user ID NOT in `allowed_user_ids` THEN the router SHALL drop the event immediately before creating any worktree or spawning any subprocess, and log the rejected user ID. The sidecar SHALL apply the same `allowed_user_ids` prefilter before spawning `cheap-otto chat dispatch`, so an unauthorized event creates no child process, and `chat dispatch` SHALL re-validate the author fail-closed in Rust.
3. WHEN an allowlisted user DMs `@Claude` or mentions `@Claude` in a channel/thread THEN the router SHALL dispatch the turn to `AgentRunner::ClaudeCode` (`claude -p --output-format stream-json --session-id/--resume <uuid>`). WHEN the message is a DM THEN the router SHALL select the repository and worktree from `dm_route` in `chat-config.json`, or from the single existing binding when the operator has exactly one, and IF neither resolves THEN it SHALL reply asking the operator to post in a mapped channel and SHALL NOT create a worktree.
4. WHEN an allowlisted user DMs `@Agy` or mentions `@Agy` in a channel/thread THEN the router SHALL dispatch the turn to `AgentRunner::Antigravity` (`agy -p --output-format stream-json --conversation <id>`).
5. WHEN an allowlisted user posts in an existing bound thread without an explicit `@mention` THEN the router SHALL dispatch to whichever agent (`claude` or `agy`) answered most recently in that thread (defaulting to `claude` on a new thread).
6. WHEN one message mentions both `@Claude` and `@Agy` THEN the router SHALL reject it with a short reply asking for a single target and SHALL NOT run either agent.
7. WHEN the same Discord message ID is delivered more than once (Gateway redelivery or resume) THEN the system SHALL handle it at most once, using a bounded deduplication record checked before dispatch.

### R3. Channel-to-repo mapping & shared-worktree handoffs

**User story:** As the operator, I want each Discord channel mapped to a specific repo and each Discord thread mapped to one shared worktree where `@Claude` and `@Agy` can both work sequentially on the same feature.

1. `~/.cheap-otto/chat-config.json` SHALL map Discord channel IDs (or `<category>/<channel-name>`) to `{ gh_account, repo_name, default_base_ref }`.
2. WHEN an allowlisted user posts a top-level prompt in a mapped `#<repo>` channel THEN the system SHALL create a Discord thread derived from the prompt slug, persist a `ChatThreadBinding` as `Pending` in `~/.cheap-otto/chat-threads.json` under a registry-wide lock using an atomic temporary-file replacement, provision the worktree `~/repos/<gh-account>/.worktrees/<repo>--<slug>`, then mark the binding `Ready`. IF provisioning fails THEN the system SHALL remove the pending binding and any partial branch or worktree. Stale `Pending` bindings SHALL be reaped using a timestamp passed in by the shell, and turns SHALL run only on `Ready` bindings.
3. Each `ChatThreadBinding` SHALL store independent `claude_session_id: Option<String>` and `agy_conversation_id: Option<String>` fields alongside the shared `worktree_dir`, so `@Claude` and `@Agy` each keep multi-turn memory while collaborating on the same checked-out branch.
4. IF a second prompt arrives for a `worktree_dir` that already has an in-flight turn (`claude` or `agy`) THEN the router SHALL queue the turn in strict arrival order using per-worktree queue ticket sequence numbers (and post a brief queued reaction/status) so two agents never write to the same worktree simultaneously, while turns in *different* threads/worktrees run in parallel, with at most `max_parallel_chat_turns` (default `3`) agent turns running across all `chat dispatch` processes, enforced by a cross-process slot limit before any agent is spawned.

### R4. Secret isolation, streaming status & chat slash commands

**User story:** As the operator, I want live progress updates, diff/PR shortcuts in chat, and strict token isolation so prompt-injected repo content cannot read bot or GitHub tokens.

1. Before spawning `claude` or `agy`, the runner SHALL strip `DISCORD_CLAUDE_TOKEN`, `DISCORD_AGY_TOKEN`, `GH_TOKEN` and `GITHUB_TOKEN` from the child process environment; bot tokens SHALL live in `0600` files under `~/.cheap-otto/tokens/`. Because environment stripping alone does not stop a same-user agent from reading token files or the Git credential store, agent turns SHALL run under a separate macOS user identity that cannot read `~/.cheap-otto/tokens/` or `<accounts.tokens_dir>` and cannot write the worktree's `.git` pointer, gitdir, hooks, or broker state, and staging and committing the agent's edits (`git add -A`, `git commit`) and GitHub-authenticated operations (`git push`, `gh`) SHALL be brokered by the `chat dispatch` process under the operator identity, never executed by the agent. After each agent turn, `chat dispatch` SHALL commit only when HEAD is the thread's `feat/<slug>-<unique-suffix>` branch and the tree changed, using the same operator-controlled Git config and hooks as push (an operator-owned empty `core.hooksPath`, operator `user.name` and `user.email`, no repo-local config honored). Before each brokered operation `chat dispatch` SHALL validate the operator-owned canonical remote and run it with operator-controlled Git config and hooks.
2. While a turn executes, the Gateway sidecar SHALL send typing indicators and throttled progress edits (`<= 1` edit per `3` s, chunking final replies cleanly at `<= 1900` chars per Discord message) parsed via `parse_stream_json_line` (`src/harness.rs`), and forward tool events to Pixel Office (`127.0.0.1:7421`) / Mission Control (`127.0.0.1:7420`).
3. Built-in text/prefix commands (`!status`, `!diff`, `!model <id>`, `!effort <level>`, `!repos`, `!clone <acct> <owner/repo>`, `!stop`) SHALL execute deterministically without invoking an LLM. `!clone` SHALL split its repository argument at the single `/` and validate `<acct>`, `<owner>` and `<repo>` separately with `validate_safe_identifier` (which rejects slashes), rejecting any other shape, and SHALL pass the validated values as discrete arguments or `posix_single_quote`d remote text.

### R5. `launchd` resilience & Mission Control integration

1. `cheap-otto chat install --execute` SHALL install and bootstrap `com.cheapotto.chat.discord` (`RunAtLoad=true`, `KeepAlive=true`) logging to `~/.cheap-otto/logs/chat/discord.log`, using the reserved namespace `chat` (`RESERVED_CHAT_PROJECT_NAME`).
2. Mission Control (`src/server.rs`) SHALL expose `GET /api/chat/threads` behind `RequestGuard` so active Discord worktree threads and their latest `claude`/`agy` turns appear in the web control plane. Each `ChatThreadBinding` SHALL persist per-runner latest-turn metadata (`status`, `summary`, `finished_at`, `turn_seq`) written by `chat dispatch`, and the endpoint SHALL map that metadata one-to-one into its JSON.
