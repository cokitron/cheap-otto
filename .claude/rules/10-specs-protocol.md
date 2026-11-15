---
paths:
  - "specs/**"
---

# Specs Protocol

Planned work lives in `specs/<feature>/` as `requirements.md`, `design.md` and `tasks.md`, written in that order, with a `README.md` hand-off page. `specs/README.md` is the full protocol; read it before you implement or edit a spec. The short version:

## Implementing a task

1. Read `CLAUDE.md`, then the spec's `README.md`, `requirements.md`, `design.md`, then your task in `tasks.md`.
2. Take one task whose dependencies are checked off. Claim it in its GitHub issue and branch as `<feature>/<task-id>-<slug>` from an up-to-date `main`.
3. Touch only the files the task lists, plus spec sync: if the implementation shows the design is wrong, also edit that spec's `design.md`, `requirements.md` and `tasks.md` in the same PR and record why in the issue. Use the test names the task gives.
4. Run the three gate commands and every acceptance check in the task. Compare with the expected results; paste them into the PR.
5. Tick the task in `tasks.md`, add `PR: #NN`, and open the PR with `Implements specs/<feature> T<id>, closes #NN`.
6. Blocked? Stop and write what you tried, what you saw and what decision you need in the issue. Don't guess, and don't weaken a test.

## Limits

- Tasks marked **LIVE** act on the real MacBook or real GitHub accounts. Only the owner runs them. Every other task must be completable with `--dry-run`, unit tests, isolated local tests with a temp `--state-dir` and no live endpoints, and the preview servers in `.claude/launch.json`.
- A task is not a licence to refactor around it. If the design is wrong, change `design.md` in the same PR and say why.

## Editing a spec

- Keep requirement ids (`R3.2`) and task ids (`T2.1`) stable once issues exist; issues and PRs refer to them.
- Requirements are EARS statements (WHEN/IF for conditional behaviour, plain SHALL for unconditional) that a test, command or observation can check. Design statements cite the requirement ids they satisfy and name real files and functions. Tasks cite requirement ids, list files, steps, test names and acceptance commands or observable checks with expected results.
- Describe the code as it is today from the source, not from memory. An open question always carries a default, so no task waits on an answer.
- A new spec gets a row in the table at the end of `specs/README.md`.
