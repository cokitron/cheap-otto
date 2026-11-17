# m1-chat-control: Talk to `@Claude` and `@Agy` from Discord across parallel multi-account repos

**Status:** planned (`requirements.md`, `design.md` and `tasks.md` ready; ships as 7 PRs under the 600-line cap with zero new Cargo crates, followed by two owner-approved **LIVE** rollout tasks T4.1 and T4.2).

## The goal in one paragraph

Let the operator message **Claude Code (`@Claude`)** and **Antigravity (`@Agy`)** independently—or sequentially in the same feature thread—from anywhere (phone, web, desktop) for \$0 using a private Discord server and outbound-only WebSockets on the Mac (`com.cheapotto.chat.discord`). Each Discord channel maps to a repository under `~/repos/<gh-account>/<repo>` (isolated per GitHub account via `~/.gitconfig` `includeIf` credential helpers), and each Discord thread maps to an isolated Git worktree (`~/repos/<gh-account>/.worktrees/<repo>--<slug>`) holding independent `claude` (`--resume`) and `agy` (`--conversation`) session IDs so complex tasks run in parallel across repositories and accounts without branch or token collisions.

## What "done" looks like

- `~/repos/<gh-account>/` on the Mac isolates `user.name`, `user.email` and HTTPS tokens per GitHub account automatically via `includeIf`, with `otto-repo` managing base clones and `.worktrees/`.
- `@Claude` and `@Agy` appear online as two separate bots in the operator's private Discord server via one `launchd` service (`com.cheapotto.chat.discord`) using Node 24's built-in `WebSocket`/`fetch` and `cheap-otto chat dispatch` (0 inbound ports, 0 third-party npm packages, 0 new Rust HTTP crates).
- Posting in `#<repo>` creates a Discord thread and matching Git worktree; `@Claude` and `@Agy` can be addressed independently via DM or `@mention` and hand off work on the same worktree branch safely via a per-worktree FIFO lock.
- Non-allowlisted Discord user IDs are dropped fail-closed, and bot/GitHub tokens are stripped from spawned agent subprocesses.
- Mission Control (`:7420`) surfaces active Discord thread worktrees via `GET /api/chat/threads`.

## Read in this order

1. [`../README.md`](../README.md): the specs protocol and 600-line PR size budget.
2. [`requirements.md`](requirements.md): five requirements (`R1`–`R5`) with EARS acceptance criteria.
3. [`design.md`](design.md): dual-bot topology, pure `src/chat.rs` router/ledger, sidecar architecture, and one-time Discord app setup.
4. [`tasks.md`](tasks.md): 9 tasks across Phases 1–4 and a 7-PR rollout plan.
