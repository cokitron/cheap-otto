# Active Lessons — `02-correctness-domain`

- **[L-CD-001]** (`*`) Monetary amounts must use integer base units with explicit scale; any floating-point arithmetic on money or rendering an `"unknown"` mutation outcome as `0` is `CRITICAL`.
- **[L-CD-002]** (`*`) Flag `catch` blocks that swallow network/parsing errors and return empty arrays/objects indistinguishable from legitimate empty states.
- **[L-CD-003]** (`*`) Dates in logs or external API outputs may be UTC while a reviewer's local clock differs—compare against UTC before flagging timestamps as in the future.
