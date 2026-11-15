# m1-dashboards: every dashboard served from the Mac, one tunnel

**Status:** planned. No implementation task has started except the prerequisite (T0.1).

## The goal in one paragraph

cheap-otto has three dashboards: Mission Control (port 7420), the Pixel Office (7421) and the Reviewer with Code Tender (7430). They should all run as `launchd` services on the MacBook and reach the Windows workstation through one SSH tunnel, started by one command. Today they are started three different ways and their tunnels fight over ports, so opening one can break another, and Mission Control does not survive a reboot.

## What "done" looks like

- `code-reviewer m1 sync` leaves both binaries and all three dashboard services installed on the Mac.
- `cheap-otto ui --execute` on the workstation makes all three reachable through a single `ssh` process and opens Mission Control.
- `cheap-otto dashboards status --execute` says which dashboards are up and why one is not.
- Each dashboard links to the others.
- A dashboard that crashes comes back by itself.
- `cheap-otto dashboards uninstall --execute` removes what this added.

## Read in this order

1. [`../README.md`](../README.md): the protocol, and the ten steps for carrying out a task.
2. [`requirements.md`](requirements.md): nine requirements with testable criteria, and what is out of scope.
3. [`design.md`](design.md): the current state with evidence, the target, every interface, the decisions and the open questions.
4. [`tasks.md`](tasks.md): 24 tasks in six phases, each with files, steps, test names and acceptance checks.

## Where to start

| If you are | Start with |
| --- | --- |
| An agent picking up work | The first unchecked task in `tasks.md` whose dependencies are all checked. T1.1, T1.8 and T4.1 depend on nothing. |
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
Phase 5  T5.1 user docs, T5.2 agent docs (need T2.3 and Phase 4)
Phase 6  T6.1 LIVE deploy and verify ── T6.2 LIVE reboot check        (owner only)
```

Tasks on different branches of this tree can be done in parallel by different agents. Tasks that touch the same file (most of Phase 1 edits `src/dashboards.rs`; Phase 2 edits `src/main.rs`) are easier done one after another.

## Safety for this spec

- Nothing before Phase 6 needs the real Mac. If a task seems to need it, the task is wrong or you are off the path: stop and say so in the issue.
- The install script kills whatever listens on a dashboard's port on the Mac before starting the service. That only ever runs on the Mac, behind `--execute` or during a deploy.
- The reviewer's own service (`com.cheapotto.code-reviewer.ui`) keeps being installed by `m1-provision.sh`. This spec reads its state and tunnels its port; it does not manage it.
- A deploy (`code-reviewer m1 sync`) ships whatever is checked out. Only the owner runs it, from `main`.

## Decisions waiting on the owner

All four have defaults, so no task is blocked. See `design.md` section 9.

1. Automatic login on the Mac after a reboot (default: report only).
2. Starting the tunnel at workstation login (default: no).
3. `ssh otto-mac` no longer forwarding dev ports 3000, 5173, 8080 (default: accepted; `cheap-otto pair --tunnel` does it).
4. Moving the reviewer's service under the registry later (default: not in this spec).

## Progress log

Add one line per merged PR, newest first.

- _nothing merged yet_
