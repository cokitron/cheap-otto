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

1. **Requirements first.** Each requirement has an id (`R3`), a user story, and numbered acceptance criteria in EARS form (`R3.2`): use *WHEN <trigger> THEN the system SHALL <response>* or *IF <condition> THEN the system SHALL <response>* for conditional behaviour; unconditional criteria may use *The system SHALL <response>*. A criterion must be checkable by a test, a command or an observation. Requirements also list what is out of scope.
2. **Design second.** The design starts from the code as it is today (with file and function names), then describes the target. Every design element names the requirements it satisfies. Decisions are recorded with the alternatives that were rejected and why. Unknowns are listed as open questions, each with a default so work is never blocked on an answer.
3. **Tasks last.** Each task is one reviewable change that fits the [PR size budget](#pr-size-budget): its goal, the requirement ids it covers, the files it touches, the steps, and acceptance checks that are commands or observable checks with expected results. A task lists the tasks it depends on. A task that needs the real MacBook or real GitHub accounts is marked **LIVE** and belongs to the owner. A **PR plan** at the top of `tasks.md` groups the tasks into PRs: small related tasks share one PR, and a task too big for one PR is written as parts (a, b), each with its own checkbox, steps, tests and branch, ordered so that every part leaves `main` green.
4. **Change the documents in order.** If implementation shows the design is wrong, fix `design.md` (and `requirements.md` if the goal changed) in the same PR as the code, then adjust `tasks.md`. Never let the code drift from the spec silently.

## PR size budget

A PR should be big enough to be worth a review and small enough to read in one sitting. Every PR, from a person or an agent, stays inside this budget:

- **At most 600 changed lines** (insertions plus deletions, tests included, `Cargo.lock` excluded); aim for about 400.
- **One theme.** Several small tasks that belong together (the same files or the same feature) may share a PR; that is what the PR plan is for. No drive-by refactors, renames, or reformatting of code the tasks do not touch. A behaviour change and a refactor are two PRs.

Check it before you open the PR:

```bash
git diff --shortstat origin/main...HEAD -- . ':(exclude)Cargo.lock'
```

**Bundling.** Fewer, coherent PRs beat many tiny ones. When a spec is written, group its tasks into PRs in the PR plan: tasks that touch the same files or finish one feature go together while they fit the budget. Do not open a PR for a task the plan bundles with others.

**Splitting.** Split a task, or a row of the PR plan, only when that unit would not fit the budget; a row that runs over is split into separate rows. Plan the split before you write code, not after. Cut at a seam that leaves `main` green: types and pure functions with their tests first, wiring and CLI after. In a binary crate (`src/main.rs`, `crates/code-reviewer`) a part must add code together with its caller, because clippy's `dead_code` lint fails the gate on an unused private item; public items in the root library (`src/lib.rs` and its modules) may land before their callers.

**Running over.** If a task grows past the budget while you work, stop at the nearest green seam and open that as part a. In the same PR, rewrite the task in `tasks.md` as parts (spec sync, step 4) and say so in the issue. Do not push through.

**Exceptions.** A purely mechanical change (lockfile, generated assets, a rename with no behaviour change, a dependency bump) may exceed the budget when the PR says so and contains nothing else.

## How to carry out a task

Follow these steps exactly. They are written so that an agent with no other context gets the same result as an expert.

1. **Read first:** `CLAUDE.md` (safety and commands), then the spec's `README.md`, `requirements.md` and `design.md`, then your task in `tasks.md`. The path-scoped rules in `.claude/rules/` apply to the files you touch.
2. **Pick the next row of the spec's PR plan** (one task, a bundle of tasks, or one part of a split task) whose tasks are still unchecked in `tasks.md`, whose issues are open and unclaimed (nobody has commented that they are starting, and no open PR references them), and whose dependencies are all checked off. Skip anything that is done or claimed.
3. **Claim it:** comment on each of its tasks' GitHub issues that you are starting, and create a branch from an up-to-date `main` with the name the PR plan gives, or `<type>/<feature>-<task-id>-<short-slug>`, for example `feat/m1-dashboards-t1-3-plist-templates`. `<type>` must be one of the prefixes CI accepts (rule 06): `feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci` or `perf`. A branch named `<feature>/…` fails the `PR Governance & Secret Guard` check.
4. **Do only that row of the plan.** Touch the files it lists, and keep the diff inside the [PR size budget](#pr-size-budget). The one exception is spec sync: if the implementation shows the design is wrong, also edit that spec's `design.md`, `requirements.md` and `tasks.md` in the same PR (protocol rule 4) and record why in the issue. If you find you must change anything else, stop and write down why in the issue; do not widen the change on your own.
5. **Write the tests the task names**, with the exact test function names given. A bug fix or a new behaviour without a test is not done.
6. **Run the gate** and read the output:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-targets --locked
   ```
   Then run every acceptance check in the task and compare with the expected result, and check the size:
   ```bash
   git diff --shortstat origin/main...HEAD -- . ':(exclude)Cargo.lock'
   ```
   Over 600 lines: split (see [PR size budget](#pr-size-budget)) before opening the PR.
7. **Stay off the live appliance.** Outside a **LIVE** task with owner approval, never pass `--execute` or `--apply` against the real Mac or real GitHub repos, and never run live `code-reviewer m1 …`, `poll`, `tend-poll`, `review`, `tend`, `sync-docs` or `ui`. `sync-docs` writes to Google Drive and has no dry run, so it is never part of a non-LIVE task. Use `--dry-run` for `poll`, `tend-poll`, `review` and `tend`. Local tests may run commands (including with `--execute`) only with an isolated temp `--state-dir` and no live endpoints, for example `tunnel --stop --execute` with no recorded pid. Exercise the CLI as a dry run with a temp `--state-dir`. Preview pages with the servers in `.claude/launch.json`. The same exemption applies as in the definition of done: issue comments, branch pushes and PRs are normal coordination, not live operations.
8. **Update the spec:** tick each task's (or part's) checkbox in `tasks.md` and add one line under it with the PR number. If you changed the design, update `design.md`.
9. **Open a PR** against `main` with a Conventional Commit title. The description says which tasks it implements and closes each of their issues (`Implements specs/<feature> T1.2 and T1.3, closes #NN, closes #MM`; a part that is not the last says `part of #NN` instead), what changed, the size line from step 6, and pastes the gate result and the acceptance check output. Do not merge it yourself unless the owner asked.
10. **If you are blocked** (a check fails for a reason outside the task, the design contradicts the code, a dependency is missing): stop, leave the branch as it is, and write in the issue what you tried, what you saw (exact output) and what decision you need. Do not guess and do not weaken a test to get past it.

## Definition of done for a task

- Every acceptance check in the task passes, and the three gate commands pass.
- The diff is inside the [PR size budget](#pr-size-budget).
- No new `unsafe`, `.unwrap()`, `.expect()` or `panic!` outside tests; no new dependency unless the task says so.
- Nothing was executed on or deployed to the real MacBook, and no `code-reviewer` command (`m1 …`, `poll`, `tend-poll`, `review`, `tend`, `ui`) acted on real GitHub repos, unless the task is **LIVE** and the owner approved it. Commenting on the task's issue and opening the PR are expected and are not covered by this rule.
- Docs that describe the changed behaviour are updated in the same PR when the task lists them.
- `tasks.md` shows the task as done with its PR number, and the issue is closed by the PR.

## Writing a new spec

Copy the structure of an existing spec folder. Keep requirement and task ids stable once issues exist, because issues and PRs refer to them. Add the spec to the table below.

| Spec | Goal | Status |
| --- | --- | --- |
| [`m1-dashboards`](m1-dashboards/README.md) | Every dashboard runs as a service on the MacBook and reaches the workstation through one SSH tunnel | Implemented; the owner's two LIVE tasks (T6.1, T6.2) remain |
