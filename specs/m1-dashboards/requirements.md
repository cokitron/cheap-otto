# Requirements: dashboards served from the M1

## Introduction

cheap-otto has three web dashboards. All of them belong on the MacBook (`otto-mac`), reached from the Windows workstation through SSH. Today each one is started and tunnelled by a different piece of code, and those pieces collide: two of them forward the same port, the shared SSH config forwards ports on every connection, and one dashboard is not a service at all. The result is dashboards that sometimes fail to open, die on reboot, and get restarted under each other.

This spec makes the three dashboards one managed set: each runs as a `launchd` service on the Mac, one command on the workstation opens one tunnel for all of them, and one command reports their state. The evidence for every problem named here is in `design.md`, section 1.

## Glossary

| Term | Meaning |
| --- | --- |
| Mac | The headless MacBook, SSH alias `otto-mac` |
| Workstation | The Windows PC the operator works on |
| Dashboard | A long-running HTTP server on the Mac, bound to `127.0.0.1` |
| Dashboard set | Mission Control (port 7420), Pixel Office (7421), Reviewer (7430) |
| Managed service | A dashboard whose LaunchAgent is installed by `cheap-otto` (Mission Control, Pixel Office) |
| Registered service | A dashboard that `cheap-otto` knows about but another tool installs (Reviewer, installed by `m1-provision.sh`) |
| Tunnel | One `ssh -N` process on the workstation that forwards every dashboard port |
| Registry | The single definition of dashboard ids, ports and labels in `src/dashboards.rs` |
| Execute gate | `--execute` (alias `--apply`); without it a command only prints its plan |

## Requirements

### R1. Dashboards run as services on the Mac

**User story:** As the operator, I want every dashboard to start by itself and come back after a crash, so that I never start one by hand.

1. WHEN the operator runs the dashboard install command with the execute gate THEN the system SHALL install one LaunchAgent for each managed service, with `RunAtLoad` and `KeepAlive` set to true.
2. WHEN a managed dashboard process exits for any reason THEN `launchd` SHALL start it again without operator action.
3. WHEN the Mac user's GUI session starts THEN every installed dashboard service SHALL start without operator action.
4. The system SHALL bind every dashboard to `127.0.0.1` and SHALL NOT generate any configuration that binds a dashboard to another address.
5. WHEN the install command runs and a service's LaunchAgent file is unchanged and the service is loaded THEN the system SHALL leave that service running and SHALL NOT restart it.
6. IF `pixel-agents` is not installed on the Mac THEN the install command SHALL skip the Pixel Office service, report that it was skipped and why, and still install the other services.
7. IF the Mac has no GUI session for the user THEN the install command SHALL fail with a message that names the cause and the fix, and SHALL NOT report success.
8. The system SHALL write each managed service's output to `~/.cheap-otto/logs/dashboards/<id>.log` on the Mac.
9. WHEN the install command (re)installs a service THEN it SHALL first stop the legacy tmux sessions `cheapotto-serve` and `cheapotto-pixel` and any stray process holding that service's port, so the service can bind. The install command SHALL then wait until the port is free (up to 5 s, then force-kill, then wait 2 s) before bootstrapping, and IF the port is still held THEN it SHALL report it, skip that service and fail.
10. IF a managed service is not listening on its port within 10 s after bootstrap, or its port cannot be freed, or its LaunchAgent cannot be written or bootstrapped, THEN the install command SHALL report the service, the cause, the service's log file and, when `launchd` has one, its last exit status, continue with the remaining services, and exit non-zero (exit code 4). Skipped services (R1.6) are not failures. A service whose port could not be freed SHALL NOT be reported as listening, even though another process answers on that port.

### R2. One registry

**User story:** As a maintainer, I want dashboard ids, ports and labels defined once, so that install, status, tunnel and the pages can never disagree.

1. The system SHALL define the dashboard set (id, display name, port, `launchd` label, who installs it) in one module of the root crate, and install, uninstall, status, tunnel, `ui` and the hub API SHALL all read it from there.
2. The system SHALL reject a dashboard set that contains port 0 or the same port twice.
3. The system SHALL name managed LaunchAgents `com.cheapotto.dashboards.<id>`.
4. WHEN a schedule is added for a project named `dashboards` THEN the system SHALL reject it with a typed error, so a schedule label can never collide with a dashboard label.
5. The `code-reviewer` crate SHALL use the same port numbers as the registry, and a test SHALL fail if they differ.

### R3. One tunnel

**User story:** As the operator, I want one command to make every dashboard reachable on the workstation, so that opening one dashboard never breaks another.

1. WHEN the operator runs `cheap-otto ui` with the execute gate THEN the system SHALL make every installed dashboard reachable on the workstation at `http://127.0.0.1:<port>` through exactly one SSH tunnel process.
2. WHEN every dashboard port already answers on the workstation AND the tunnel recorded in `tunnel.json` is a live ssh process whose recorded destination and SSH port equal the current peer's and whose recorded ports cover every dashboard port THEN the system SHALL NOT start another tunnel; IF every port answers but no such recorded tunnel exists THEN the system SHALL start nothing, fail, name the ports, and print the command that finds the holder.
3. The tunnel SHALL be spawned with discrete arguments, `-N`, `ExitOnForwardFailure=yes`, `ServerAliveInterval=30`, `ServerAliveCountMax=3`, `ConnectTimeout=10`, and a loopback bind on both ends of every forward.
4. WHEN the system starts a tunnel THEN it SHALL record the process id, ports, destination, SSH port and start time in `tunnel.json` in the state directory.
5. IF some dashboard ports answer on the workstation and others do not THEN the system SHALL stop the tunnel it recorded (only if that process is alive and its command line is the recorded tunnel's) and start a new one; IF the ports are held by a process it did not record THEN it SHALL fail, name the ports, and print the command that finds the holder. IF the recorded tunnel is alive but was opened to a different destination or SSH port than the current peer THEN the system SHALL stop it and start a new one, even when every port answers. The system SHALL NOT signal a recorded process id whose command line it cannot verify as the recorded tunnel; such a record SHALL be removed as stale.
6. IF the tunnel process exits, or has not opened every forward within 5 s, THEN the system SHALL terminate the process if still running, remove `tunnel.json`, report failure naming the ports that did not answer, and SHALL NOT print that the dashboards are ready or open a browser.
7. The operator SHALL be able to see the tunnel's state and stop it with a command.
8. The managed `Host` block that `cheap-otto` writes into `~/.ssh/config` SHALL NOT contain `LocalForward` lines, and rewriting the block SHALL remove any that an older version wrote.
9. `code-reviewer ui` and `open-ui.ps1` (a wrapper over it) SHALL NOT forward the Pixel Office port or start a second tunnel, and WHEN the Reviewer port already answers on the workstation they SHALL reuse the existing tunnel. WHEN it does not answer, they SHALL delegate to `cheap-otto ui`, which owns the single shared tunnel under R3.1 to R3.5, only if the requested host is the paired peer (or none was given) and the port is the default Reviewer port; otherwise they SHALL start a tunnel that forwards only the requested Reviewer port. IF that tunnel does not open the Reviewer port within 5 s, or its process exits, THEN they SHALL terminate and reap the process before reporting failure.
10. WHEN `cheap-otto ui` runs with the execute gate THEN it SHALL stop before starting the tunnel or opening a browser if the install step failed, and after the tunnel is up it SHALL probe each dashboard's state on the Mac and warn for any dashboard that is not listening.

### R4. One deploy

**User story:** As the operator, I want the deploy I already run to leave every dashboard installed and current, so that there is no second procedure to remember.

1. WHEN the operator runs `code-reviewer m1 sync` THEN the Mac SHALL have both `cheap-otto` and `code-reviewer` installed in `~/.cargo/bin`, built from the synced commit.
2. WHEN provisioning runs on the Mac and the installed `cheap-otto` supports it THEN provisioning SHALL install or refresh the managed dashboard services using the install script that `cheap-otto` generates.
3. IF dashboard installation fails during provisioning THEN provisioning SHALL print a warning and continue, so a dashboard problem never blocks a reviewer deploy.
4. The install script SHALL be generated by one function, and the workstation path (over SSH) and the Mac path (provisioning) SHALL both run that same script.

### R5. One status

**User story:** As the operator, I want one command that tells me which dashboards are up and why one is not, so that I do not SSH in to find out.

1. WHEN the operator runs the dashboard status command with the execute gate THEN the system SHALL report, for each dashboard: whether its LaunchAgent is loaded, its process id if running, whether its port is listening on the Mac, and whether its port answers on the workstation. For every dashboard that is not listening on the Mac, the system SHALL also report why (binary not installed, no GUI session, LaunchAgent not loaded, or loaded but not listening, with `launchd`'s last exit status when there is one) and where its log is, so a service that failed or was skipped at install time is never shown as a bare "not loaded".
2. The status command SHALL report whether `pixel-agents` and the `cheap-otto` binary are installed on the Mac, and whether automatic login is configured.
3. IF automatic login is not configured THEN the status command SHALL warn that dashboards start only after the user logs in following a reboot.
4. The status command SHALL offer `--json` output that contains the same facts and nothing else on stdout.
5. The status command SHALL only read; it SHALL NOT start, stop or change anything.

### R6. Dashboards link to each other

**User story:** As the operator, I want to move between dashboards from any of them, so that I do not keep a list of ports.

1. Mission Control SHALL serve `GET /api/dashboards`, returning each dashboard's id, display name, port and whether its port is listening on the machine running the server.
2. WHEN a sibling dashboard is listening THEN Mission Control's page SHALL show a link to it, built from the page's own hostname and the sibling's port.
3. The Reviewer dashboard's page SHALL show a link to Mission Control, built the same way.
4. Links between dashboards SHALL be plain navigation and SHALL NOT require any dashboard to accept cross-origin API requests.

### R7. Dashboards stay closed to hostile pages

**User story:** As the operator, I want a web page in my browser to be unable to drive my dashboards, so that browsing is not a way into the Mac.

1. Every dashboard server this project builds SHALL reject any request whose authority is not a loopback name, and SHALL reject a state-changing request that carries an `Origin` not on that server's allowlist. A request with no `Origin` header is not subject to the origin check. The authority is the `Host` header; Mission Control also accepts the HTTP/2 or absolute-form request authority when `Host` is absent, and the Reviewer reads only the `Host` header. (Delivered by the origin guard, T0.1. No task in this spec may change either server's contract.)
2. No task in this spec SHALL add a CORS allow header or widen a bind address.

### R8. Existing commands keep working, and everything can be undone

**User story:** As the operator, I want my habits and my exit to survive, so that this change costs me nothing.

1. `cheap-otto ui --execute` SHALL remain the one command that leaves Mission Control open in the browser, and SHALL keep accepting `--port`, `--pixel-port`, `--state-dir` and `--no-open`.
2. WHEN the operator runs the dashboard uninstall command with the execute gate THEN the system SHALL unload and delete only the managed dashboard LaunchAgents.
3. WHEN the operator runs `cheap-otto restore --unload-schedules --apply` (adding `--remote` to act on the paired MacBook over SSH) THEN the system SHALL continue to remove every `com.cheapotto.*` LaunchAgent, including the dashboard services; WITHOUT `--apply` it SHALL only print the plan.
4. Every new side-effecting command SHALL be a dry run without the execute gate and SHALL print the full plan first, except that with `--json` stdout SHALL contain only the JSON document. A dry run SHALL NOT open any socket, local or remote.

### R9. The change is documented where people and agents look

1. The README, the Pages site quickstart, `crates/code-reviewer/README.md`, both copies of the `update-m1` skill, `CLAUDE.md` and rules 07 and 08 SHALL describe the new commands and SHALL NOT describe the old tmux startup.

## Out of scope

- Restyling Mission Control to the brand. It keeps its current theme; that is a separate spec.
- Authentication, multiple users, or reaching dashboards from anywhere but loopback through SSH.
- Starting the tunnel automatically when the workstation logs in.
- Changing the Mac's login settings. The spec only reports whether automatic login is on.
- Replacing `pixel-agents` or changing how it draws.
- Moving the Reviewer's LaunchAgent out of `m1-provision.sh`, or renaming its label.
- Adding a ninth check to `cheap-otto doctor`.
- Forwarding the dev ports (3000, 5173, 8080) through the dashboard tunnel. They keep their own tunnel (`cheap-otto pair --tunnel`).
