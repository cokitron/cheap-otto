# m1-dashboards: every dashboard served from the Mac, one tunnel

**Status:** planned. No implementation task has started except the prerequisite (T0.1).

**Tracking issue:** #6 (tasks are #7 to #30, label `m1-dashboards`, milestone "M1 dashboards").

## The goal in one paragraph

cheap-otto has three dashboards: Mission Control (port 7420), the Pixel Office (7421) and the Reviewer with Code Tender (7430). They should all run as `launchd` services on the MacBook and reach the Windows workstation through one SSH tunnel, started by one command. Today they are started three different ways and their tunnels fight over ports, so opening one can break another, and Mission Control does not survive a reboot.

## What "done" looks like

- With a logged-in GUI session on the Mac and `pixel-agents` installed, `code-reviewer m1 sync` leaves both binaries and all three dashboard services installed on the Mac. Without `pixel-agents`, the Pixel Office is skipped and reported.
- `cheap-otto ui --execute` on the workstation makes every installed dashboard reachable through a single `ssh` process and opens Mission Control. It stops with an error if an installed service fails to listen.
- `cheap-otto dashboards status --execute` says which dashboards are up and why one is not. A skipped or failed dashboard stays unavailable and is reported there.
- Mission Control and the Reviewer link to each other. The Pixel Office page is third-party and unmodified, so it has no links.
- A dashboard that crashes comes back by itself.
- `cheap-otto dashboards uninstall --execute` removes what this added.

## Read in this order

1. [`../README.md`](../README.md): the protocol, and the ten steps for carrying out a task.
2. [`requirements.md`](requirements.md): nine requirements with testable criteria, and what is out of scope.
3. [`design.md`](design.md): the current state with evidence, the target, every interface, the decisions and the open questions.
4. [`tasks.md`](tasks.md): 24 tasks across Phases 0–6 (six implementation phases plus the Phase 0 prerequisite), each with files, steps, test names and acceptance checks.

## Where to start

| If you are | Start with |
| --- | --- |
| An agent picking up work | Pick the first unchecked, unclaimed task in `tasks.md` whose dependencies are all checked; skip tasks already claimed in their issue. T0.1 is in progress, so T1.1, T1.8 and T4.1 are currently available. |
| Reviewing the plan | `design.md` sections 1 (problems), 8 (decisions) and 9 (open questions) |
| The owner | Answer the four open questions in `design.md` section 9 if you disagree with the defaults, and run the two **LIVE** tasks at the end |

## Order of work

```text
Phase 0  T0.1 origin guard ─────────────────────────────┐ (blocks only tasks that edit the two server.rs files)
Phase 1  T1.1 registry ─┬─ T1.2 reserved name            │
                        ├─ T1.3 plists ── T1.4 install ── T1.5 uninstall/restart
                        ├─ T1.6 status                   │
                        └─ T1.7 tunnel       T1.8 ssh config (independent)
Phase 2  T2.1 subcommands (needs T1.4, T1.5, T1.6)       │
         T2.2 tunnel management (needs T1.7)             │
         T2.3 rewrite ui (needs T1.8, T2.1, T2.2)        │
Phase 3  T3.1 hub API (needs T0.1, T1.1) ── T3.2 links ◀─┘
Phase 4  T4.1 both binaries (independent) ── T4.2 provisioning (needs T2.1)
         T4.3 reviewer ui (needs T0.1, T2.3) ── T4.5 open-ui.ps1
         T4.4 pixel button (needs T0.1, T1.1)    T4.6 ports + link (needs T0.1, T1.1)
Phase 5  T5.1 user docs (needs T2.3, T4.3), T5.2 agent docs (needs T2.3, T4.2)
Phase 6  T6.1 LIVE deploy and verify ── T6.2 LIVE reboot check        (owner only)
```

Tasks on different branches of this tree can be done in parallel by different agents. Tasks that touch the same file (most of Phase 1 edits `src/dashboards.rs`; Phase 2 edits `src/main.rs`) are easier done one after another.

## Safety for this spec

- Nothing before Phase 6 needs the real Mac. If a task seems to need it, the task is wrong or you are off the path: stop and say so in the issue.
- The install script kills whatever listens on a dashboard's port on the Mac before starting the service. That only ever runs on the Mac, behind `--execute` or during a deploy.
- The reviewer's own service (`com.cheapotto.code-reviewer.ui`) keeps being installed by `m1-provision.sh`. This spec reads its state and tunnels its port; it does not manage it.
- A deploy (`code-reviewer m1 sync`) ships whatever is checked out. Only the owner runs it, from `main`.

## Coordination with other open work

- **PR #2 (governance)** edits `crates/code-reviewer/src/m1.rs`, `crates/code-reviewer/scripts/m1-provision.sh`, `crates/code-reviewer/src/server.rs`, `src/main.rs`, `CLAUDE.md` and `AGENTS.md`, and moves the Mac checkout default to `~/otto-workspaces/cheap-otto`. Phase 4 tasks touch the same files. Before starting T4.1 to T4.6, pull `main` and re-read those files; if PR #2 has merged, line references in `design.md` section 1 may have moved, and the function names are what to search for.
- **The origin guard (T0.1)** edits both `server.rs` files. T3.1, T4.3, T4.4 and T4.6 start after it merges.

## Decisions waiting on the owner

All four have defaults, so no task is blocked. See `design.md` section 9.

1. Automatic login on the Mac after a reboot (default: report only).
2. Starting the tunnel at workstation login (default: no).
3. `ssh otto-mac` no longer forwarding dev ports 3000, 5173, 8080 (default: accepted; `cheap-otto pair --tunnel` does it).
4. Moving the reviewer's service under the registry later (default: not in this spec).

## Progress log

Add one line per merged PR, newest first.

- _nothing merged yet_
