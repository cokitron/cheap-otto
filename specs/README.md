# Specs: requirements → design → tasks

Work that is too large for one sitting is planned here before it is built. A spec is a folder with three documents written in order, plus a short hand-off page. Any agent (Claude Code, Antigravity, Kiro, Codex) or person can pick up a task from a spec and finish it without the conversation that produced it.

```text
specs/<feature>/
├── README.md         Status, what is done, what is next, who must approve what
├── requirements.md   What must be true when we are finished, as testable statements
├── design.md         How we will make it true: components, interfaces, decisions
└── tasks.md          Ordered, small, verifiable steps that implement the design
```

## The protocol

1. **Requirements first.** Each requirement has an id (`R3`), a user story, and numbered acceptance criteria in EARS form (`R3.2`): *WHEN <trigger> THEN the system SHALL <response>*, or *IF <condition> THEN the system SHALL <response>*. A criterion must be checkable by a test, a command or an observation. Requirements also list what is out of scope.
2. **Design second.** The design starts from the code as it is today (with file and function names), then describes the target. Every design element names the requirements it satisfies. Decisions are recorded with the alternatives that were rejected and why. Unknowns are listed as open questions, each with a default so work is never blocked on an answer.
3. **Tasks last.** Each task is one reviewable change: its goal, the requirement ids it covers, the files it touches, the steps, and acceptance checks that are commands with expected results. A task lists the tasks it depends on. A task that needs the real MacBook or real GitHub accounts is marked **LIVE** and belongs to the owner.
4. **Change the documents in order.** If implementation shows the design is wrong, fix `design.md` (and `requirements.md` if the goal changed) in the same PR as the code, then adjust `tasks.md`. Never let the code drift from the spec silently.

## How to carry out a task

Follow these steps exactly. They are written so that an agent with no other context gets the same result as an expert.

1. **Read first:** `CLAUDE.md` (safety and commands), then the spec's `README.md`, `requirements.md` and `design.md`, then your task in `tasks.md`. The path-scoped rules in `.claude/rules/` apply to the files you touch.
2. **Pick one task** whose dependencies are all checked off. Do not start a task that someone else has claimed in its GitHub issue.
3. **Claim it:** comment on the task's GitHub issue that you are starting, and create a branch named `<feature>/<task-id>-<short-slug>` (for example `m1-dashboards/t1-3-plist-template`) from an up-to-date `main`.
4. **Do only that task.** Touch the files it lists. If you find you must change something else, stop and write down why in the issue; do not widen the change on your own.
5. **Write the tests the task names**, with the exact test function names given. A bug fix or a new behaviour without a test is not done.
6. **Run the gate** and read the output:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-targets
   ```
   Then run every acceptance check in the task and compare with the expected result.
7. **Stay off the live appliance.** Never pass `--execute` or `--apply`, and never run `code-reviewer m1 …`, `poll`, `tend-poll`, `review`, `tend` or `ui`, unless the task is marked **LIVE** and the owner has told you to run it. Exercise the CLI as a dry run with a temp `--state-dir`. Preview pages with the servers in `.claude/launch.json`. The same exemption applies as in the definition of done: issue comments, branch pushes and PRs are normal coordination, not live operations.
8. **Update the spec:** tick the task's checkbox in `tasks.md` and add one line under it with the PR number. If you changed the design, update `design.md`.
9. **Open a PR** against `main` with a Conventional Commit title. The description says which task it implements (`Implements specs/<feature> T1.3, closes #NN`), what changed, and pastes the gate result and the acceptance check output. Do not merge it yourself unless the owner asked.
10. **If you are blocked** (a check fails for a reason outside the task, the design contradicts the code, a dependency is missing): stop, leave the branch as it is, and write in the issue what you tried, what you saw (exact output) and what decision you need. Do not guess and do not weaken a test to get past it.

## Definition of done for a task

- Every acceptance check in the task passes, and the three gate commands pass.
- No new `unsafe`, `.unwrap()`, `.expect()` or `panic!` outside tests; no new dependency unless the task says so.
- Nothing was executed on or deployed to the real MacBook, and no `code-reviewer` command (`m1 …`, `poll`, `tend-poll`, `review`, `tend`, `ui`) acted on real GitHub repos, unless the task is **LIVE** and the owner approved it. Commenting on the task's issue and opening the PR are expected and are not covered by this rule.
- Docs that describe the changed behaviour are updated in the same PR when the task lists them.
- `tasks.md` shows the task as done with its PR number, and the issue is closed by the PR.

## Writing a new spec

Copy the structure of an existing spec folder. Keep requirement and task ids stable once issues exist, because issues and PRs refer to them. Add the spec to the table below.

| Spec | Goal | Status |
| --- | --- | --- |
| [`m1-dashboards`](m1-dashboards/README.md) | Every dashboard runs as a service on the MacBook and reaches the workstation through one SSH tunnel | Planned |
