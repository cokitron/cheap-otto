# Code Tender KB — Local Verification & Check Recipes

> Code Tender runs inside a worktree backed by the local repository checkout (`local_repo_path`) with local dependencies linked. Record repo-specific test, build, typecheck, and lint recipes here so fixes are proven locally—never pushed blindly for CI to test.

- `[*]` Run the project's typechecker, linter, and relevant unit/integration tests locally inside the worktree before finalizing edits; treat failing local checks as a hard blocker to `auto_push`.
- `[*]` Inspect `package.json`, `Cargo.toml`, `pyproject.toml`, `CLAUDE.md`, and `AGENTS.md` in the local app first to understand workspace conventions and exact verification scripts.
