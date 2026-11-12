---
name: review-pr
description: Run or inspect a multi-lens Internal CI code review or stateful compliance re-review on a watched GitHub PR, posting inline comments and syncing the Google Drive handoff doc.
---

# `review-pr` — Multi-Lens Internal CI Review Skill

## Workflow
1. Verify appliance readiness if needed:
   ```bash
   code-reviewer --workspace ~/otto-workspaces/code-reviewer doctor
   ```
2. Preview pending reviews across watched repos:
   ```bash
   code-reviewer --workspace ~/otto-workspaces/code-reviewer poll --dry-run
   ```
3. Run a targeted review or compliance re-review on a specific PR:
   ```bash
   code-reviewer --workspace ~/otto-workspaces/code-reviewer review --repo <OWNER/REPO> --pr <PR_NUMBER>
   ```
4. Inspect tracked PR status, open observations, and Google Drive Doc URL:
   ```bash
   code-reviewer --workspace ~/otto-workspaces/code-reviewer status
   ```
