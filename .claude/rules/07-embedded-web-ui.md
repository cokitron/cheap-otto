---
paths:
  - "src/ui_assets.rs"
  - "src/server.rs"
  - "crates/code-reviewer/src/ui_html.rs"
  - "crates/code-reviewer/src/server.rs"
  - ".claude/launch.json"
---

# Embedded Web UIs

Each binary ships one web page compiled into the executable. There is no bundler, no `node_modules` and no asset directory.

| Page | Constant | Served by | Default port | Look |
| --- | --- | --- | --- | --- |
| Mission Control | `MISSION_CONTROL_HTML` in `src/ui_assets.rs` | axum, `src/server.rs` | 7420 (pixel-agents office on 7421) | Older dark theme (blue accent, rounded cards) |
| Reviewer + Code Tender dashboard | `DASHBOARD_HTML` in `crates/code-reviewer/src/ui_html.rs` | hand-rolled `std::net` server | 7430 | Neo-brutalist: the brand's source of truth |

## 1. The page is one raw string

- Mission Control is `r####"…"####`; the dashboard is `r##"…"##`. The text `"####` (or `"##`) must never appear inside, or the literal ends early. If new content needs that sequence, add a `#` to both delimiters.
- HTML, CSS and JS are inline in that one string. Keep CSS in the `<style>` block and JS in the single `<script>` at the end.
- **Fully self-contained.** No CDN scripts, web fonts, remote images or analytics. The page is opened through an SSH tunnel on machines that may be offline, so every byte comes from the binary. Use the system font stacks already declared.
- Vanilla JS only: `fetch`, template strings, `onclick` handlers. No framework, no build step.
- **Escape everything you interpolate.** Agent names, task text, branch names, PR titles and log lines are attacker-influenced. Use the page's helper for text (`escapeHtml` in Mission Control, `esc` in the dashboard) and never build an `onclick="…"` attribute from unescaped data.

## 2. The API is the product

- The page talks only to its own server with relative `fetch('/api/…')` calls. Anything the page can do, an external agent can do with the same request, so design routes for both: JSON in, JSON out, stable field names.
- New Mission Control behaviour is a route in `build_router` plus a handler that calls a core planner (rule 03), then UI. Cover the route in the `#[tokio::test]` at the bottom of `src/server.rs` with `oneshot`.
- The dashboard's routes are a `match (method, path)` in `crates/code-reviewer/src/server.rs`. `POST /api/actions/*`, `/api/tender/*`, `/api/settings/*` and `/api/repos/*` act for real: they start reviews, push fixes and rewrite `config/repos.toml`.
- Validate path and body identifiers in the handler (`validate_safe_identifier`, `validate_repo_slug`) before using them in a command or a path.
- **Every request passes the request guard before routing** (rule 02, section 6). A `Host` that isn't the server gets `403`; so does a `POST`, `PUT`, `PATCH` or `DELETE` whose `Origin` isn't the server's own. Neither server sends CORS headers. Same-origin `fetch` and clients that send no `Origin` (CLI, `curl`, agents) are unaffected, so:
  - keep `fetch` paths relative, and never point a page at another port's API without growing that server's allowlist first;
  - send `Content-Type: application/json` with every body. Both pages' `api()` helpers do; the dashboard answers `415` otherwise;
  - a new route is guarded automatically. In `oneshot` tests send a loopback `Host` header (see `guarded_request` in the `src/server.rs` tests), or the answer is `403`.

## 3. Style

- When you restyle or extend either page, use the brand tokens from rule 08: black ground, white ink, Signal Yellow `#F7D842`, square corners, hard offset shadows, UPPERCASE mono labels. `ui_html.rs` already defines them as CSS variables at the top of its `<style>` block; copy from there.
- Mission Control still uses the older blue theme. Don't mix the two looks inside one page: either leave a page's theme alone or convert the whole page.
- Status is always a word plus a colour (`■ RUNNING`), never colour alone.
- The pixel-agents office is a separate project shown in an iframe. It is the visual layer only: nothing in the control plane may depend on it being up.

## 4. Previewing safely

`.claude/launch.json` defines two preview servers on spare ports, so they never collide with a live tunnel on 7420 or 7430:

| Name | Command | What you get |
| --- | --- | --- |
| `mission-control` | `cheap-otto serve --port 7440 --state-dir target/preview-state --workspaces-root target/preview-workspaces` | The real page against an empty throwaway state dir. No SSH calls. |
| `reviewer-dashboard` | `code-reviewer --workspace crates/code-reviewer serve --bind 127.0.0.1:7441` | The real page reading the local ledger in `~/.code-reviewer`. |

- Rendering and `GET` requests are safe on both.
- On the reviewer dashboard, **don't click action buttons or send `POST` requests** while previewing: they run against the real config and accounts.
- After a UI change, rebuild (the HTML is compiled in), reload, and check the browser console for errors. Check a narrow viewport too; operators open these on laptops beside a terminal.
