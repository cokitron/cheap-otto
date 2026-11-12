---
name: fix-from-drive
description: Read a PR's Internal CI Review Document from Google Drive (or local M1 mirror), systematically resolve every open observation (OBS-001..N), run repo checks, and push for automatic compliance re-review.
---

# `fix-from-drive` — Tackle Open PR Review Observations from Google Drive

Use this skill from **any** machine (Windows workstation or M1 MacBook) and **any** agent (Claude Code, Antigravity, Codex) to resolve findings produced by `code-reviewer`.

## Steps
1. **Locate the PR's Living Review Doc in Google Drive:**
   - List your configured private Drive folder (`gdrive_folder_id` in `config/repos.toml`):
     ```bash
     gdrive readonly ls <DRIVE_FOLDER_ID>
     gdocs readonly read <DOC_ID> --comments=false
     ```
   - Or on the M1 directly: read `~/otto-workspaces/code-reviewer/reviews/<owner>__<repo>/pr-<N>/latest-review.md`.
2. **Execute Section 1 (`Open Observations — Agent Action Queue`) in severity order (`CRITICAL` -> `HIGH` -> `MEDIUM` -> `LOW`):**
   - Check **Section 2 (`Resolved & Accepted Compliance Ledger`)** first so you never regress previously closed items.
   - Apply each fix at the cited `path:line` or record a concrete, evidence-backed reply on the PR if an item is governed by an intentional ADR/invariant or tracked follow-up issue `#N`.
3. **Run the Repository Gate Before Pushing:**
   - Run the repository's configured `local_check_cmd` (e.g., `npm run check` or `cargo test --all-targets`).
4. **Push Changes:**
   - Pushing to the PR branch automatically triggers `code-reviewer` in **`RE_REVIEW` compliance mode** on the M1, updating both the GitHub PR review and the Google Drive document.
