---
paths:
  - "src/ui_assets.rs"
  - "src/server.rs"
  - "src/request_guard.rs"
  - "crates/code-reviewer/src/ui_html.rs"
  - "crates/code-reviewer/src/server.rs"
  - "crates/code-reviewer/src/request_guard.rs"
  - ".claude/launch.json"
---

# Embedded Web UIs

Each binary ships one web page compiled into the executable. There is no bundler, no `node_modules` and no asset directory.

| Page | Constant | Served by | Default port | Look |
| --- | --- | --- | --- | --- |
| Mission Control | `MISSION_CONTROL_HTML` in `src/ui_assets.rs` | axum, `src/server.rs` | 7420 (pixel-agents office on 7421) | Older dark theme (blue accent, rounded cards) |
| Reviewer + Code Tender dashboard | `DASHBOARD_HTML` in `crates/code-reviewer/src/ui_html.rs` | hand-rolled `std::net` server | 7430 | Neo-brutalist: the brand's source of truth |

## How they run

On the appliance every dashboard is a `launchd` LaunchAgent on the Mac, bound to `127.0.0.1`, and the workstation reaches all of them through one SSH tunnel.

- **The registry is `src/dashboards.rs`.** `DashboardSet::standard()` lists Mission Control (7420, `com.cheapotto.dashboards.mission-control`), the pixel-agents office (7421, `com.cheapotto.dashboards.pixel-office`) and the Reviewer (7430, `com.cheapotto.code-reviewer.ui`). Port constants in `crates/code-reviewer/src/server.rs` mirror it, checked by `tests/dashboard_ports.rs` and `tests/dashboard_labels.rs`. Change a port or label there first.
- **Services.** `cheap-otto dashboards install --execute` (and `m1 sync`, through `m1-provision.sh`) writes the two `com.cheapotto.dashboards.*` plists with `KeepAlive`, logging to `~/.cheap-otto/logs/dashboards/<id>.log`. The Reviewer's own plist stays with `m1-provision.sh`. `cheap-otto ui` no longer starts `serve` or `pixel-agents` in tmux. On macOS the reviewer page's Pixel Office button kickstarts the LaunchAgent instead of spawning a process (`pixel_start_action`); the one remaining direct start is `code-reviewer ui`'s `nohup code-reviewer serve` when the Reviewer's LaunchAgent cannot be kickstarted.
- **One tunnel.** `cheap-otto ui --execute` installs the services, opens one `ssh -N` with a `-L 127.0.0.1:<port>:127.0.0.1:<port>` per dashboard, records it in `tunnel.json`, and opens the page `--open` names. `code-reviewer ui` delegates to it only for the recorded peer on port 7430 and otherwise starts a Reviewer-only tunnel. `cheap-otto dashboards status --execute` says which dashboard is loaded, listening and tunnelled, and why not.
- **Links between pages.** Mission Control polls `GET /api/dashboards` and shows a plain link to each other dashboard that is listening (`loadSiblingDashboards`); the reviewer's top bar links to Mission Control on 7420. They are navigation only: no page calls another dashboard's API. The reviewer page's Pixel Office check is the one exception: a `no-cors` `fetch` to the pixel root that only tests liveness.

## 1. The page is one raw string

- Mission Control is `r####"…"####`; the dashboard is `r##"…"##`. The text `"####` (or `"##`) must never appear inside, or the literal ends early. If new content needs that sequence, add a `#` to both delimiters.
- HTML, CSS and JS are inline in that one string. Keep CSS in the `<style>` block and JS in the single `<script>` at the end.
- **Fully self-contained.** No CDN scripts, web fonts, remote images or analytics. The page is opened through an SSH tunnel on machines that may be offline, so every byte comes from the binary. Use the system font stacks already declared.
- Vanilla JS only: `fetch`, template strings, `onclick` handlers. No framework, no build step.
- **Escape everything you interpolate.** Agent names, task text, branch names, PR titles and log lines are attacker-influenced. Use the page's helper for text (`escapeHtml` in Mission Control, `esc` in the dashboard) and never build an `onclick="…"` attribute from unescaped data.

## 2. The API is the product

- Control-plane calls use relative `fetch('/api/…')` requests. Mission Control also embeds the separately served pixel-agents office: it asks `/api/pixel-office` for the port and token, then points the iframe at `http://<page host>:<pixel_port>/?token=<encoded token>` (`loadPixelOffice` in `src/ui_assets.rs`; the `?token=` suffix is URL-encoded with `encodeURIComponent` and only added when a token is returned). That iframe and the plain links between dashboards ("How they run") are the only non-relative URLs; keep them optional, built from `location.hostname` and a registry port, and don't add others. Anything the page can do, an external agent can do with the same request, so design routes for both: JSON in, JSON out, stable field names.
- New Mission Control behaviour is a route in `build_router` plus a handler that calls a core planner (rule 03), then UI. Cover the route in the `#[tokio::test]` at the bottom of `src/server.rs` with `oneshot`.
- The dashboard's routes are a `match (method, path)` in `crates/code-reviewer/src/server.rs`. `POST /api/actions/*`, `/api/tender/*`, `/api/settings/*` and `/api/repos/*` act for real: they start reviews, push fixes and rewrite `config/repos.toml`.
- Validate path and body identifiers in the handler (`validate_safe_identifier`, `validate_repo_slug`) before using them in a command or a path.
- Keep both servers bound to loopback and expose them remotely only through SSH tunnels. Every request passes the request guard before routing (section 3).

## 3. The request guard

Neither server authenticates requests. The loopback bind keeps other machines out; the guard keeps out web pages in the operator's browser, which can reach loopback. Both servers run it on every request before any route: `src/request_guard.rs` behind the `guard_request` middleware in `src/server.rs`, and `crates/code-reviewer/src/request_guard.rs` called from `handle_http_connection`. The two files mirror each other; change them together.

1. **Bind.** Both servers refuse to start unless the bind host is `127.0.0.1`, `localhost` or `::1` (`require_loopback_bind`), so a wildcard or LAN `--host` / `--bind` is an error.
2. **Host.** `Host`, or for Mission Control the HTTP/2 or absolute-form request authority when `Host` is absent, must be `127.0.0.1`, `localhost` or `[::1]`. Anything else, or no usable authority, is `403` (DNS-rebinding defence). The dashboard reads only the `Host` header.
3. **Origin.** A request that is not `GET`, `HEAD` or `OPTIONS` and carries an `Origin` must carry an allowlisted one, or it is `403`. The allowlist is the server's own origins (`http://127.0.0.1:<port>`, `http://localhost:<port>`, `http://[::1]:<port>`), held in the server state (`ControlPlaneState::request_guard`, `ServerContext::guard`).
4. **No `Origin` passes rule 3.** That is the CLI, `curl` and external agents.
5. **Dashboard only:** a `POST` that carries a body must be `Content-Type: application/json`, or it is `415`. This is the guard's policy, enforced before routing, whether or not the handler reads the body. The reason: a browser sends `text/plain`, `application/x-www-form-urlencoded` and `multipart/form-data` bodies cross-origin without a preflight, and a JSON content type is what forces one. Body-less `POST`s are covered by the Origin rule. Mission Control gets the JSON requirement from axum's `Json` extractor on the routes that take a body.

What follows for page and API work:

- Keep `fetch` paths relative. Neither server sends CORS headers today, so a foreign page can't read a response or pass a preflight. Never add a general or wildcard CORS layer.
- The one exception: to let one local dashboard call another's API, grow that server's allowlist with `RequestGuard::trust_loopback_port` (or `trust_origin` in the root crate) and answer CORS for exactly those allowlisted origins, nothing wider.
- Send `Content-Type: application/json` with every body. Both pages' `api()` helpers do.
- The allowlist is keyed on the listening port, so a tunnel must keep the port (`127.0.0.1:<port>:127.0.0.1:<port>`). Through a remapped port the page loads but its `POST`s get `403`.
- A new route is guarded automatically. In `oneshot` tests send a loopback `Host` header (see `guarded_request` in the `src/server.rs` tests), or the answer is `403`.

## 4. Style

- When you restyle or extend either page, use the brand tokens from rule 09: black ground, white ink, Signal Yellow `#F7D842`, square corners, hard offset shadows, UPPERCASE mono labels. `ui_html.rs` already defines them as CSS variables at the top of its `<style>` block; copy from there.
- Mission Control still uses the older blue theme. Don't mix the two looks inside one page: either leave a page's theme alone or convert the whole page.
- Status is always a word plus a colour (`■ RUNNING`), never colour alone.
- The pixel-agents office is a separate project shown in an iframe. It is the visual layer only: nothing in the control plane may depend on it being up.

## 5. Previewing safely

`.claude/launch.json` defines two preview servers on spare ports, so they never collide with a live tunnel on 7420 or 7430:

| Name | Command | What you get |
| --- | --- | --- |
| `mission-control` | `cheap-otto serve --port 7440 --state-dir target/preview-state --workspaces-root target/preview-workspaces` | The real page against an empty throwaway state dir. No SSH calls. |
| `reviewer-dashboard` | `code-reviewer --workspace crates/code-reviewer serve --bind 127.0.0.1:7441` | The real page reading the local ledger in `~/.code-reviewer`. |

- Rendering is safe on both, but not every `GET`: the reviewer dashboard's `GET /api/tender/open-prs` polls GitHub and syncs the ledger (`crates/code-reviewer/src/server.rs`).
- On the reviewer dashboard, **don't click action buttons, open the open-PR picker, or send `POST` requests** while previewing: they run against the real config and accounts. To exercise them, point the preview at a disposable config and state dir instead of `~/.code-reviewer`.
- After a UI change, rebuild (the HTML is compiled in), reload, and check the browser console for errors. Check a narrow viewport too; operators open these on laptops beside a terminal.
