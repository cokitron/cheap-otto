# False-Positive Suppression Guardrails — `02-correctness-domain`

- **[FP-CD-001]** (`*`) Respect documented workspace package consumption conventions recorded in the repository's ADRs or `CLAUDE.md`.
- **[FP-CD-002]** (`*`) Do not flag UTC vs local timezone drift when formatters explicitly format in UTC, and do not assume external GitHub branch-protection rulesets are enforced solely from workflow YAML without repository ruleset evidence.
