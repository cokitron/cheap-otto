---
paths:
  - "specs/**"
---

# Specs Protocol

Planned work lives in `specs/<feature>/` as `requirements.md`, `design.md` and `tasks.md`, written in that order, with a `README.md` hand-off page. `specs/README.md` is the full protocol; read it before you implement or edit a spec. The short version:

## Implementing a task

1. Read `CLAUDE.md`, then the spec's `README.md`, `requirements.md`, `design.md`, then your task in `tasks.md`.
2. Take the next row of the spec's PR plan (one task, a bundle of tasks, or one part of a split task) whose tasks are unchecked in `tasks.md`, whose issues are open and unclaimed, and whose dependencies are checked off. Claim it in its GitHub issue and branch from an up-to-date `main` as `<type>/<feature>-<task-id>-<slug>` (for example `feat/m1-dashboards-t1-3-plist-templates`). `<type>` is a prefix CI accepts, listed in rule 06; a `<feature>/…` branch fails the PR governance check.
3. Touch only the files the task lists, plus spec sync: if the implementation shows the design is wrong, also edit that spec's `design.md`, `requirements.md` and `tasks.md` in the same PR and record why in the issue. Use the test names the task gives.
4. Run the three gate commands and every acceptance check in the task. Compare with the expected results; paste them into the PR.
5. Tick each task in `tasks.md`, add `PR: #NN`, and open the PR with `Implements specs/<feature> T<id>, closes #NN` for every task in it (an earlier part says `part of #NN`; the last part closes the issue).
6. Blocked? Stop and write what you tried, what you saw and what decision you need in the issue. Don't guess, and don't weaken a test.

## Limits

- PR size: at most 600 changed lines (tests included, `Cargo.lock` excluded) and one theme, checked with `git diff --shortstat origin/main...HEAD -- . ':(exclude)Cargo.lock'`. Small related tasks share a PR as the spec's PR plan says; a task that will not fit alone is split into parts before coding, each leaving `main` green. Purely mechanical changes (lockfile, generated assets, renames, dependency bumps) may exceed it when the PR says so and contains nothing else. `specs/README.md` has the details.
- Tasks marked **LIVE** act on the real MacBook or real GitHub accounts. They need the owner's approval: the owner runs them, or tells an agent to run a specific step.
- Outside a LIVE task, never pass `--execute` or `--apply` against the real Mac or real GitHub repos, and never run live `code-reviewer m1 …`, `poll`, `tend-poll`, `review`, `tend`, `sync-docs` or `ui`. `poll`, `tend-poll`, `review` and `tend` may run only with `--dry-run`. `sync-docs` writes to Google Drive and has no dry run, so it is never part of a non-LIVE task.
- Every non-LIVE task must be completable with unit tests, dry runs, the preview servers in `.claude/launch.json` and isolated local tests. A test may pass `--execute` only with a temp `--state-dir` and no live endpoint. Issue comments, branch pushes and PRs are normal coordination, not live operations.
- A task is not a licence to refactor around it. If the design is wrong, change `design.md` in the same PR and say why.

## Editing a spec

- Keep requirement ids (`R3.2`) and task ids (`T2.1`) stable once issues exist; issues and PRs refer to them.
- Requirements are EARS statements (WHEN/IF for conditional behaviour, plain SHALL for unconditional) that a test, command or observation can check. Design statements cite the requirement ids they satisfy and name real files and functions. Tasks cite requirement ids, list files, steps, test names and acceptance commands or observable checks with expected results.
- Describe the code as it is today from the source, not from memory. An open question always carries a default, so no task waits on an answer.
- A new spec gets a row in the table at the end of `specs/README.md`.
