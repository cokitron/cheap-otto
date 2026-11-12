# Code Tender KB — Verified Fix Patterns

> Format each rule with `[owner/repo]` or `[*]` so agents across PRs apply proven remediation patterns instead of reinventing or pushing half-fixes.

- `[*]` Always pair behavior fixes with a targeted regression test covering the failure mode requested by the reviewer.
- `[*]` Before editing a module, inspect its sibling callers, shared helpers, and types in the local app checkout so signatures and error paths stay consistent across the entire app.
