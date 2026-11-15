# Design: dashboards served from the M1

Read `requirements.md` first. Requirement ids in brackets, such as [R3.5], say which criterion a design element satisfies.

## 1. Where we are today

Every statement in this section was read from the code on `main`.

| Dashboard | Server | Port | How it runs on the Mac | Started by | Tunnel opened by |
| --- | --- | --- | --- | --- | --- |
| Mission Control | `cheap-otto serve` (`src/server.rs`) | 7420 | tmux session `cheapotto-serve` | `cheap-otto ui --execute` (`run_ui` in `src/main.rs`) | `run_ui`: detached `ssh -N -L 7420 -L 7421 user@host` |
| Pixel Office | third-party `pixel-agents` | 7421 | tmux session `cheapotto-pixel`, or an orphan child of the reviewer server | `run_ui`, or `POST /api/pixel-office/start` (`start_pixel_agents_server` in `crates/code-reviewer/src/server.rs`) | both tunnels forward it |
| Reviewer + Code Tender | `code-reviewer serve` (`crates/code-reviewer/src/server.rs`) | 7430 | LaunchAgent `com.cheapotto.code-reviewer.ui` (`RunAtLoad`, `KeepAlive`) | `crates/code-reviewer/scripts/m1-provision.sh`, section 6 | `open_tunneled_ui`: detached `ssh -N -L 7430 -L 7421 otto-mac`; or `scripts/open-ui.ps1` (7430 only) |

### Problems

- **P1. Two tunnels claim port 7421.** `run_ui` and `open_tunneled_ui` both forward the Pixel Office port, and both pass `ExitOnForwardFailure=yes`. Whichever runs second exits at once. `open_tunneled_ui` ignores that: it prints "Dashboard ready" and opens a browser on a dead port.
- **P2. The SSH config forwards ports on every connection.** `PeerNode::build_port_tunnel` (`src/bridge.rs`) writes `LocalForward` lines into the managed `Host otto-mac` block, and `run_ui` rewrites that block to add 7420 and 7421. Every `ssh otto-mac` then tries to bind those ports: `m1 exec`, the `git push` in `m1 sync`, `cheap-otto shell`. Tunnel commands that use the alias (`open_tunneled_ui`, `open-ui.ps1`) fail whenever any of those ports is already bound.
- **P3. Mission Control is not a service.** It lives in a tmux session that `run_ui` kills and recreates on every call (dropping in-flight requests) and that does not survive a reboot.
- **P4. Nothing installs `cheap-otto` on the Mac.** `sync_everything_to_m1` (`crates/code-reviewer/src/m1.rs`) runs `cargo install --path crates/code-reviewer` only. The tmux command `exec cheap-otto serve` works only if someone installed the binary by hand.
- **P5. Pixel Office has no owner.** Three code paths can start it, none restarts it, and none logs it.
- **P6. Tunnels are untracked.** Each is a detached process with no record. There is no way to see or stop it, and stale ones hold ports.
- **P7. There is no status command.** Finding out why a dashboard is down means SSHing in.
- **P8. Dashboards don't know about each other.** There is no shared list of ports and no links between pages.
- **P9. A reboot needs a login.** LaunchAgents in the `gui/<uid>` domain start only when the user's GUI session exists. After a power cut the Mac restarts (`autorestart 1`) and waits at the login window unless automatic login is on. Nothing reports this.

## 2. Target

```text
 WORKSTATION (Windows)                                    MAC (otto-mac)
 ┌────────────────────────────────┐                      ┌──────────────────────────────────────────────┐
 │ cheap-otto ui --execute        │   ssh: run install   │ launchd  gui/<uid>                           │
 │  1. ensure services ───────────┼──────────────────────▶  com.cheapotto.dashboards.mission-control    │
 │  2. ensure ONE tunnel          │                      │     cheap-otto serve      127.0.0.1:7420     │
 │  3. open browser               │   ssh -N  (one pid)  │  com.cheapotto.dashboards.pixel-office       │
 │                                │   -L 7420 ───────────▶     pixel-agents          127.0.0.1:7421     │
 │ 127.0.0.1:7420  Mission Control│   -L 7421 ───────────▶  com.cheapotto.code-reviewer.ui (existing)   │
 │ 127.0.0.1:7421  Pixel Office   │   -L 7430 ───────────▶     code-reviewer serve   127.0.0.1:7430     │
 │ 127.0.0.1:7430  Reviewer       │                      │                                              │
 │ .cheap-otto/tunnel.json (pid)  │                      │ ~/.cheap-otto/logs/dashboards/<id>.log       │
 └────────────────────────────────┘                      └──────────────────────────────────────────────┘
        code-reviewer ui  ──▶ delegates to `cheap-otto ui`; never opens its own tunnel when one exists
        code-reviewer m1 sync ──▶ installs both binaries, then provisioning pipes the install script to sh
```

Three rules hold the design together:

1. **One registry.** `src/dashboards.rs` defines the set. Everything else reads it. [R2.1]
2. **One generator, two transports.** One function renders the install script. The workstation sends it over SSH; provisioning on the Mac pipes it to `sh`. [R4.4]
3. **One opener of tunnels.** Only `cheap-otto` starts a dashboard tunnel. `code-reviewer ui` delegates to it. [R3.1, R3.9]

## 3. Components

### 3.1 Registry: `src/dashboards.rs` (new, functional core)

Pure module: no I/O, no clock. Register it in `src/lib.rs` as `pub mod dashboards;`.

```rust
pub const MISSION_CONTROL_PORT: u16 = 7420;
pub const PIXEL_OFFICE_PORT: u16 = 7421;
pub const REVIEWER_PORT: u16 = 7430;
/// Pseudo-project that owns dashboard LaunchAgent labels. Schedules may not use it.
pub const RESERVED_PROJECT_NAME: &str = "dashboards";
pub const REVIEWER_LAUNCHD_LABEL: &str = "com.cheapotto.code-reviewer.ui";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DashboardError {
    #[error("invalid dashboard port {0}")]
    InvalidPort(u16),
    #[error("dashboard port {0} is used twice")]
    DuplicatePort(u16),
    #[error("unknown dashboard '{0}' (expected mission-control, pixel-office or reviewer)")]
    UnknownDashboard(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DashboardId { MissionControl, PixelOffice, Reviewer }
// DashboardId::all() -> &'static [DashboardId]   (in the order above)
// DashboardId::slug(self) -> &'static str          "mission-control" | "pixel-office" | "reviewer"
// DashboardId::display_name(self) -> &'static str  "Mission Control" | "Pixel Office" | "Reviewer"
// DashboardId::parse(raw: &str) -> Result<Self, DashboardError>

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceManager {
    /// LaunchAgent is rendered and installed by cheap-otto.
    CheapOtto,
    /// LaunchAgent is installed by crates/code-reviewer/scripts/m1-provision.sh.
    CodeReviewerProvisioner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardService {
    pub id: DashboardId,
    pub display_name: String,
    pub port: u16,
    pub launchd_label: String,
    pub manager: ServiceManager,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardSet { pub services: Vec<DashboardService> }
// DashboardSet::standard() -> Self
// DashboardSet::with_ports(mission_control: u16, pixel_office: u16, reviewer: u16) -> Result<Self, DashboardError>
// DashboardSet::get(&self, id: DashboardId) -> Option<&DashboardService>
// DashboardSet::ports(&self) -> Vec<u16>
// DashboardSet::managed(&self) -> impl Iterator<Item = &DashboardService>   (manager == CheapOtto)
```

- Labels: `com.cheapotto.dashboards.mission-control`, `com.cheapotto.dashboards.pixel-office`, and `REVIEWER_LAUNCHD_LABEL` for the reviewer. [R2.3]
- `with_ports` rejects 0 and duplicates. [R2.2]
- Every public function that returns a value carries `#[must_use = "…"]`, as the rest of the core does.
- `build_schedule_install_plan` (`src/schedule.rs`) gains a check: a project equal to `RESERVED_PROJECT_NAME` returns the new variant `ScheduleError::ReservedProjectName(String)`. [R2.4]

### 3.2 LaunchAgent templates

`render_service_plist_template(service: &DashboardService, set: &DashboardSet) -> Option<String>` returns `None` for a service that cheap-otto does not manage. The text contains the literal `$HOME` (and `$PIXEL_BIN` for Pixel Office). It is a template: the install script writes it through an unquoted heredoc, so the Mac's shell fills those in. `launchd` itself does not expand variables, which is why the expansion happens at install time.

Mission Control template (Pixel Office differs only in label, log name and `ProgramArguments`):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.cheapotto.dashboards.mission-control</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>ProgramArguments</key>
  <array>
    <string>$HOME/.cargo/bin/cheap-otto</string>
    <string>serve</string>
    <string>--host</string>
    <string>127.0.0.1</string>
    <string>--port</string>
    <string>7420</string>
    <string>--pixel-port</string>
    <string>7421</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$HOME</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$HOME/.cheap-otto/logs/dashboards/mission-control.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME/.cheap-otto/logs/dashboards/mission-control.log</string>
</dict>
</plist>
```

Pixel Office `ProgramArguments`: `$PIXEL_BIN`, `--port`, `7421`, `--host`, `127.0.0.1`, `--no-terminal` (the flags `run_ui` passes today).

- Ports come from the `DashboardSet`, never from literals in the template code.
- `KeepAlive` and `RunAtLoad` satisfy [R1.1, R1.2, R1.3]; the `127.0.0.1` arguments satisfy [R1.4]; the log paths satisfy [R1.8].
- `ThrottleInterval` 10 keeps a crash loop from spinning.
- The only interpolated values are ports (`u16`) and constant labels, so no escaping is needed. If a string ever becomes operator-supplied, pass it through `xml_escape` (`src/schedule.rs`; make it `pub(crate)`).

### 3.3 Install script

`render_install_script(set: &DashboardSet) -> String` returns one POSIX `sh` script. It must run under `/bin/sh` on macOS with `set -eu` and contain no bashisms. Structure, in order:

```sh
set -eu
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
UID_NUM=$(id -u)
AGENTS_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/.cheap-otto/logs/dashboards"
mkdir -p "$AGENTS_DIR" "$LOG_DIR"

# [R1.7] LaunchAgents need the user's GUI session.
if ! launchctl print "gui/$UID_NUM" >/dev/null 2>&1; then
  echo "error=no-gui-session"
  echo "hint=Log in to the Mac desktop once, or enable automatic login, then run this again."
  exit 3
fi

# [R1.9] Sessions started by older versions of `cheap-otto ui`.
if command -v tmux >/dev/null 2>&1; then
  tmux kill-session -t cheapotto-serve 2>/dev/null || true
  tmux kill-session -t cheapotto-pixel 2>/dev/null || true
fi

# install_service <id> <label> <port>   (plist text on stdin)
install_service() {
  id="$1"; label="$2"; port="$3"
  plist="$AGENTS_DIR/$label.plist"
  tmp="$plist.tmp.$$"
  cat > "$tmp"
  # [R1.5] Unchanged and loaded: leave it alone.
  if [ -f "$plist" ] && cmp -s "$tmp" "$plist" && launchctl print "gui/$UID_NUM/$label" >/dev/null 2>&1; then
    rm -f "$tmp"
    echo "dashboard=$id action=unchanged"
    return 0
  fi
  launchctl bootout "gui/$UID_NUM/$label" 2>/dev/null || true
  # bootout is asynchronous; bootstrapping too early fails with "Input/output error".
  n=0
  while launchctl print "gui/$UID_NUM/$label" >/dev/null 2>&1 && [ "$n" -lt 10 ]; do
    n=$((n + 1)); sleep 0.5
  done
  # [R1.9] Whatever still listens on the port is a stray.
  for pid in $(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true); do
    kill "$pid" 2>/dev/null || true
    echo "dashboard=$id stray_killed=$pid"
  done
  mv "$tmp" "$plist"
  launchctl bootstrap "gui/$UID_NUM" "$plist"
  echo "dashboard=$id action=installed"
}

# wait_port <id> <port>
wait_port() {
  i=0
  while [ "$i" -lt 20 ]; do
    if nc -z 127.0.0.1 "$2" 2>/dev/null; then echo "dashboard=$1 listening=1"; return 0; fi
    i=$((i + 1)); sleep 0.5
  done
  echo "dashboard=$1 listening=0"
}

# --- mission-control ---
if [ -x "$HOME/.cargo/bin/cheap-otto" ]; then
  install_service mission-control com.cheapotto.dashboards.mission-control 7420 <<PLIST
…template from 3.2…
PLIST
  wait_port mission-control 7420
else
  echo "dashboard=mission-control action=skipped reason=cheap-otto-not-installed"
fi

# --- pixel-office ---  [R1.6]
PIXEL_BIN=$(command -v pixel-agents 2>/dev/null || true)
if [ -n "$PIXEL_BIN" ]; then
  …pixel-agents config snippet, see below…
  install_service pixel-office com.cheapotto.dashboards.pixel-office 7421 <<PLIST
…template…
PLIST
  wait_port pixel-office 7421
else
  echo "dashboard=pixel-office action=skipped reason=pixel-agents-not-installed"
fi
```

- The heredoc delimiter is unquoted (`<<PLIST`) on purpose, so `$HOME` and `$PIXEL_BIN` expand.
- Strays are found by port with `lsof`, not by `pkill -f <pattern>`: the script's own text travels in the argv of the remote shell, so a pattern match would kill the installer.
- The pixel-agents config snippet is the one `run_ui` runs today, moved here unchanged. Store it as a raw string constant so no brace escaping is needed:
  ```rust
  const PIXEL_AGENTS_CONFIG_SNIPPET: &str = r#"if command -v node >/dev/null 2>&1; then
    mkdir -p "$HOME/.pixel-agents"
    node -e 'const fs=require("fs"),p=process.env.HOME+"/.pixel-agents/config.json";let c={};try{c=JSON.parse(fs.readFileSync(p,"utf8"))}catch{};c.standalone=Object.assign({},c.standalone,{watchAllSessions:true});c.hooksConsent=Object.assign({},c.hooksConsent,{claude:"granted"});fs.writeFileSync(p,JSON.stringify(c,null,2));'
  fi"#;
  ```
- Exit codes: 0 when every step succeeded (skips are reported, not failures), 3 for no GUI session, non-zero from `set -e` if `launchctl bootstrap` fails.

**Transports** [R4.4]:

- From the workstation: `build_install_plan(peer: &PeerNode, set: &DashboardSet) -> DashboardPlan`. Its `CommandSpec` is `ssh -p <port> <user@host> <remote>` where `remote` is `format!("/bin/sh -c {}", posix_single_quote(&script))`. Wrapping in `/bin/sh -c` matters: the Mac's login shell is zsh, and the script must run under `sh`. `posix_single_quote` (`src/worktree.rs`) makes the script one safe argument.
- On the Mac: `cheap-otto dashboards install-script` prints the script; provisioning pipes it to `/bin/sh` (section 3.10).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardPlan {
    pub set: DashboardSet,
    /// The POSIX script that runs on the Mac.
    pub script: String,
    /// Human-readable steps for the dry-run printout.
    pub display_steps: Vec<String>,
    /// `ssh … /bin/sh -c '<script>'`.
    pub spec: CommandSpec,
}
```

### 3.4 Uninstall, restart, status

- `render_uninstall_script(set)` / `build_uninstall_plan(peer, set)`: for each managed service, `launchctl bootout "gui/$UID_NUM/<label>" 2>/dev/null || true`, `rm -f` the plist, print `dashboard=<id> action=removed`. It never touches the reviewer's label. [R8.2]
- `build_restart_spec(peer, set, id)`: `launchctl kickstart -k "gui/$(id -u)/<label>"` over SSH. Works for all three ids.
- `render_status_script(set)` / `build_status_spec(peer, set)`: read-only. [R5.5] It prints one line per dashboard and one `host` line:

```text
dashboard=mission-control loaded=1 pid=812 listening=1
dashboard=pixel-office loaded=0 pid=- listening=0
dashboard=reviewer loaded=1 pid=640 listening=1
host cheap_otto_bin=1 pixel_agents_bin=1 gui_session=1 autologin_user=admin
```

  - `loaded`: exit status of `launchctl print "gui/$UID_NUM/<label>"`.
  - `pid`: `launchctl print … | awk '/^[[:space:]]*pid = /{print $3; exit}'`, or `-`.
  - `listening`: `nc -z 127.0.0.1 <port>`.
  - `autologin_user`: `defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null`, or `-`. [R5.2]
- `parse_status_output(raw: &str, set: &DashboardSet) -> DashboardStatusReport` is pure and tolerant: it splits each line on whitespace into `key=value` tokens, ignores unknown lines, keys and ids, and treats a missing key as false or `None`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRemoteStatus { pub id: DashboardId, pub loaded: bool, pub pid: Option<u32>, pub listening: bool }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardHostFacts { pub cheap_otto_bin: bool, pub pixel_agents_bin: bool, pub gui_session: bool, pub autologin_user: Option<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStatusReport { pub dashboards: Vec<DashboardRemoteStatus>, pub host: Option<DashboardHostFacts> }
```

The CLI adds `tunnelled: bool` per dashboard (does the port answer on the workstation) and the warning for [R5.3].

### 3.5 Tunnel

```rust
/// ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -o ServerAliveCountMax=3
///     -o BatchMode=yes -p <ssh_port> -L 127.0.0.1:P:127.0.0.1:P … <user@host>
pub fn build_tunnel_spec(peer: &PeerNode, set: &DashboardSet) -> CommandSpec

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelRecord { pub pid: u32, pub ports: Vec<u16>, pub destination: String, pub started_epoch_secs: u64 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelDecision {
    /// Every port answers locally.
    AlreadyUp,
    /// No port answers locally.
    Start,
    /// Some ports answer and some do not.
    Conflict { held: Vec<u16>, missing: Vec<u16> },
}
pub fn decide_tunnel(local_ports: &[(u16, bool)]) -> TunnelDecision
```

- The tunnel forwards every port in the set, whether or not that service is installed on the Mac. An SSH forward listens locally even when nothing answers remotely, so "every port answers locally" means "the tunnel is up". [R3.1, R3.2]
- `BatchMode=yes` stops a detached process from hanging on a password prompt. The destination is `user@host`, not the alias. [R3.3]
- `tunnel.json` lives in the state directory on the workstation. [R3.4]

The imperative part lives in `src/main.rs` as `ensure_dashboard_tunnel(state_dir, peer, set) -> Result<(), Box<dyn Error>>`:

1. Probe each port with `is_local_port_open` and call `decide_tunnel`.
2. `AlreadyUp`: print it and return.
3. `Conflict`: load `tunnel.json`. If its pid is alive **and is an ssh process**, terminate it, wait up to 2 s for the ports to close, and continue to step 4. Otherwise return an error naming `held` and printing how to find the holder: `netstat -ano | findstr :<port>` on Windows, `lsof -nP -iTCP:<port> -sTCP:LISTEN` elsewhere. [R3.5]
4. `Start`: spawn the `CommandSpec` with null stdio, write `tunnel.json` with `child.id()`, then poll for up to 5 s. If every port answers, succeed. If `child.try_wait()` reports an exit, delete `tunnel.json` and return an error that says the tunnel exited and how to test SSH (`ssh -p <port> <user@host> true`). [R3.6]

Process helpers, also in `src/main.rs` (the reviewer's `state.rs` has the same idea around line 430):

| Helper | Windows | Elsewhere |
| --- | --- | --- |
| `is_ssh_process_alive(pid) -> bool` | `tasklist /FI "PID eq <pid>" /NH` output contains `ssh` | `ps -p <pid> -o comm=` output contains `ssh` |
| `terminate_process(pid)` | `taskkill /PID <pid> /F` | `kill <pid>` |

Checking the image name guards against a recycled pid.

### 3.6 CLI

New subcommand group in `src/main.rs`, following rule 05 (doc comments are help text, dry-run by default, one `XOptions` struct per handler):

| Command | Effect | Gate |
| --- | --- | --- |
| `cheap-otto dashboards list [--json]` | Print the registry | none (pure) |
| `cheap-otto dashboards install` | Ensure the managed LaunchAgents on the Mac | `--execute` |
| `cheap-otto dashboards install-script` | Print the install script to stdout, nothing else | none (pure) |
| `cheap-otto dashboards status [--json]` | Report per [R5] | `--execute` runs the read-only SSH probe; without it, print the probe command and the local tunnel state |
| `cheap-otto dashboards restart --id <id>` | `launchctl kickstart -k` | `--execute` |
| `cheap-otto dashboards uninstall` | Remove the managed LaunchAgents | `--execute` |
| `cheap-otto dashboards tunnel [--stop]` | Ensure the tunnel, or stop the recorded one | `--execute` |
| `cheap-otto ui [--open <id>]` | Install, tunnel, open the browser | `--execute` |

Shared flags: `--state-dir`, `--port` (Mission Control), `--pixel-port`, `--reviewer-port`, each defaulting to the registry constants. `--execute` has `visible_alias = "apply"`. [R8.4]

### 3.7 `cheap-otto ui`

`run_ui` becomes:

1. Resolve the state dir and peer; build `DashboardSet::with_ports(port, pixel_port, reviewer_port)`.
2. Build the install plan and the tunnel spec. Print the peer, the three URLs, the plan's `display_steps` and the tunnel command.
3. Without `--execute`: print the hint and return.
4. Run the install plan with `execute_command_spec`. The script's `dashboard=… action=…` lines go to the terminal.
5. If `peer.json` exists, rewrite the managed `~/.ssh/config` block (now without forwards, section 3.8).
6. `ensure_dashboard_tunnel`.
7. Unless `--no-open`, open `http://localhost:<port>` for `--open <id>` (default `mission-control`).

The inline tmux startup script and the hand-rolled `ssh -N` in today's `run_ui` are deleted. [R3.1, R8.1]

### 3.8 SSH config block

`PeerNode::build_port_tunnel` stops writing `LocalForward` lines into `ssh_config_snippet`; the snippet keeps `Host`, `HostName`, `User`, `Port`, `ServerAliveInterval` and `ServerAliveCountMax`. `tunnel_spec` and `ssh_tunnel_command` are unchanged, so `cheap-otto pair --tunnel` still forwards the dev ports explicitly. `merge_ssh_config_block` already replaces everything between its markers, so the next `pair --apply` or `ui --execute` removes forwards an older version wrote. [R3.8]

This changes behaviour: a plain `ssh otto-mac` no longer forwards 3000, 5173 and 8080. The release notes and README must say that `cheap-otto pair --tunnel` is how to get them.

### 3.9 Mission Control: hub API and links

- `GET /api/dashboards` in `build_router` (`src/server.rs`). The handler builds the set from `state.config.port`, `state.config.pixel_port` and `REVIEWER_PORT`, probes `127.0.0.1:<port>` for each with a 250 ms timeout (the pattern `handle_pixel_office` uses), and returns:
  ```json
  {"dashboards":[
    {"id":"mission-control","name":"Mission Control","port":7420,"listening":true,"current":true},
    {"id":"pixel-office","name":"Pixel Office","port":7421,"listening":true,"current":false},
    {"id":"reviewer","name":"Reviewer","port":7430,"listening":false,"current":false}]}
  ```
  [R6.1]
- `MISSION_CONTROL_HTML` (`src/ui_assets.rs`): on load, call `/api/dashboards` and, for each entry with `listening && !current`, add a link in the header: `href = 'http://' + location.hostname + ':' + port + '/'`, `target="_blank"`, `rel="noopener"`, text through `escapeHtml`. [R6.2]
- Links are navigation only. No dashboard calls another's API from the browser, so no CORS is needed. [R6.4, R7.2]

### 3.10 `code-reviewer` alignment

All changes stay std-only (rule 05).

- **Deploy both binaries** [R4.1]. In `crates/code-reviewer/src/m1.rs`, extract the remote build script from `sync_everything_to_m1` into a pure `fn render_remote_build_script(remote_dir_expr: &str, brew: &str, cargo: &str, npm: &str) -> String`, and add `cargo install --path . --locked --force` after the existing `cargo install --path crates/code-reviewer --locked --force` (inside the `if [ -d crates/code-reviewer ]` branch). The first deploy after this compiles `cheap-otto` in release mode on the Mac, which takes a few minutes.
- **Provision the dashboards** [R4.2, R4.3]. New section 8 at the end of `crates/code-reviewer/scripts/m1-provision.sh`, before the final "Provisioning complete" line:
  ```sh
  # 8. Dashboard services (Mission Control + Pixel Office), generated by cheap-otto
  OTTO_BIN="$HOME/.cargo/bin/cheap-otto"
  if [ -x "$OTTO_BIN" ] && "$OTTO_BIN" dashboards install-script >/dev/null 2>&1; then
    if "$OTTO_BIN" dashboards install-script | /bin/sh; then
      echo "[m1-provision] Dashboard services are installed"
    else
      echo "[m1-provision] WARN: dashboard service install failed; reviewer deploy continues"
    fi
  else
    echo "[m1-provision] cheap-otto is missing or too old for dashboard services; skipping"
  fi
  ```
- **One tunnel** [R3.9]. Rewrite `open_tunneled_ui` (`crates/code-reviewer/src/server.rs`):
  1. If `127.0.0.1:<port>` answers locally, open the browser and return.
  2. Ensure the remote service (the existing `launchctl kickstart` script).
  3. Find `cheap-otto`: a file named `cheap-otto` or `cheap-otto.exe` beside `std::env::current_exe()`, else `cheap-otto` on `PATH`. If found, run `cheap-otto ui --execute --no-open` with inherited stdio and wait for it.
  4. If not found, spawn a fallback tunnel built by a pure `fn reviewer_fallback_tunnel_args(host: &str, port: u16) -> Vec<String>` that forwards only the reviewer port.
  5. Poll the local port. If it never answers, return an error; do not print "ready" or open a browser.
- **Pixel Office start button**. `start_pixel_agents_server` first runs `launchctl kickstart "gui/<uid>/com.cheapotto.dashboards.pixel-office"` (uid from `id -u`). Only if that fails does it fall back to today's direct spawn.
- **`scripts/open-ui.ps1`** becomes a thin wrapper: `& cargo run --quiet -- ui --host $HostAddr --port $Port` (plus `--no-open` when `-NoOpen` is set), run from the crate directory, checking `$LASTEXITCODE`.
- **Ports and link** [R2.5, R6.3]. Add `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = 7420;` beside `DEFAULT_UI_PORT` in `crates/code-reviewer/src/server.rs`, and a link to Mission Control in `DASHBOARD_HTML` built from `location.hostname` and that port (text through `esc`). A test in the root crate, `tests/dashboard_ports.rs`, uses `include_str!("../crates/code-reviewer/src/server.rs")` and asserts the literal declarations of `DEFAULT_UI_PORT`, `DEFAULT_PIXEL_PORT` and `DEFAULT_MISSION_CONTROL_PORT` carry the registry's values.

## 4. Security

- The origin and host guard from the preceding change stays as it is. This spec adds one `GET` route and no CORS. [R7]
- Every generated listener argument is `127.0.0.1`; tests assert `0.0.0.0` never appears in a plist or script. [R1.4]
- The install script is assembled from constants and `u16` ports. It takes no operator text, so there is nothing to inject. `--id` values are parsed into `DashboardId` before any command is built.
- The install script kills processes by port. That is deliberate and visible: each kill prints `stray_killed=<pid>`.
- The tunnel uses `BatchMode=yes` and discrete args. The terminate helper refuses to kill a pid whose image is not ssh.
- `status` reads `autoLoginUser`; nothing in this spec writes login settings.

## 5. Failure handling

| Situation | Behaviour |
| --- | --- |
| No GUI session on the Mac | Install script prints `error=no-gui-session` and a hint, exits 3; the CLI returns the error. [R1.7] |
| `cheap-otto` not installed on the Mac | Mission Control skipped with `reason=cheap-otto-not-installed`; CLI adds "run `code-reviewer m1 sync`". |
| `pixel-agents` not installed | Pixel Office skipped with a reason; other services installed. [R1.6] |
| Service installed but port never listens | `listening=0`; CLI points at `~/.cheap-otto/logs/dashboards/<id>.log`. |
| Workstation port held by a foreign process | Error naming the ports and the command to find the holder; nothing is killed. [R3.5] |
| Tunnel exits early | Error with the SSH test command; `tunnel.json` removed; no browser. [R3.6] |
| Mac unreachable | `execute_command_spec` returns the SSH failure; nothing else runs. |
| Dashboard install fails during provisioning | Warning; reviewer deploy continues. [R4.3] |

## 6. Testing

Nothing here needs a Mac or a network.

| Layer | Where | What |
| --- | --- | --- |
| Registry, templates, scripts, parser, tunnel decision | `#[cfg(test)]` in `src/dashboards.rs` | Names are given per task in `tasks.md` |
| Script syntax | `tests/dashboard_scripts.rs`, `#[cfg(unix)]` | Write each rendered script to a temp file and run `sh -n` on it |
| CLI | `tests/cli_integration.rs` | Dry runs with a temp `--state-dir`; assert on printed plans and that no tunnel is started |
| API | `#[tokio::test]` in `src/server.rs` | `GET /api/dashboards` through `oneshot` |
| Reviewer | `#[cfg(test)]` in `m1.rs` and `server.rs` | Build script installs both binaries; fallback tunnel args contain one `-L` |
| Cross-crate | `tests/dashboard_ports.rs` | Reviewer constants equal the registry |

Live behaviour (launchd actually restarting a process, one ssh process on the workstation) is verified by the owner in the **LIVE** tasks with the exact commands listed there.

## 7. Rollout and rollback

1. Merge the code tasks in order. Until the deploy, nothing changes on the Mac.
2. Deploy with `code-reviewer m1 sync` from `main` (**LIVE**, owner).
3. Run `cheap-otto ui --execute` on the workstation and walk the verification list in task T6.1.

Rollback: `cheap-otto dashboards uninstall --execute` removes the two managed LaunchAgents; the reviewer's own service is untouched. `cheap-otto restore --unload-schedules --remote --apply` still removes everything under `com.cheapotto.*`. [R8.2, R8.3]

## 8. Decisions

| # | Decision | Alternatives rejected |
| --- | --- | --- |
| D1 | LaunchAgents in `gui/<uid>` | LaunchDaemons need `sudo` and a system-level change, against the reversibility invariant. tmux is what fails today (P3). |
| D2 | One Rust-generated install script, sent over SSH or piped on the Mac | A second hand-written copy in `m1-provision.sh` would drift from the Rust one. A `--local` execution mode would need a local `sh -c`, which rule 02 forbids in the root crate. |
| D3 | Only `cheap-otto` opens dashboard tunnels; `code-reviewer ui` delegates | Making `code-reviewer` depend on the root library would pull tokio and axum into a crate that must stay std-only. |
| D4 | Remove `LocalForward` from the managed SSH config block | Keeping it is the root of P2. `ClearAllForwardings` on other commands would also clear the forwards a tunnel passes on its command line. |
| D5 | The reviewer's LaunchAgent stays in `m1-provision.sh` | Moving a working production service adds risk for no user-visible gain; it can be a later spec. |
| D6 | Free a port by killing its listener (`lsof`) | `pkill -f` would match the installer's own command line. |
| D7 | Keep one port per dashboard and link between them | A reverse proxy on one port is a new component to run and secure, and would put three apps under one origin. |
| D8 | Tunnel starts on demand | Auto-start at Windows login is persistent configuration the owner has not asked for. |

## 9. Open questions

Each has a default, so no task waits on an answer.

| # | Question | Default used by the tasks |
| --- | --- | --- |
| Q1 | Should the Mac log in automatically after a reboot? | Report only (`status` warns). The owner changes the setting by hand if wanted. |
| Q2 | Should the tunnel start when the workstation logs in? | No. |
| Q3 | Is it acceptable that `ssh otto-mac` stops forwarding 3000, 5173 and 8080? | Yes; `cheap-otto pair --tunnel` forwards them. |
| Q4 | Should the reviewer's service move into the registry's management later? | Not in this spec. |

## 10. Traceability

| Requirement | Design | Tasks |
| --- | --- | --- |
| R1 Services | 3.2, 3.3 | T1.3, T1.4, T2.1, T6.1 |
| R2 Registry | 3.1, 3.10 | T1.1, T1.2, T4.6 |
| R3 Tunnel | 3.5, 3.7, 3.8, 3.10 | T1.7, T1.8, T2.2, T2.3, T4.3, T4.5 |
| R4 Deploy | 3.3, 3.10 | T1.4, T2.1, T4.1, T4.2 |
| R5 Status | 3.4, 3.6 | T1.6, T2.1 |
| R6 Links | 3.9, 3.10 | T3.1, T3.2, T4.6 |
| R7 Security | 4 | T0.1, and every task's "no CORS, loopback only" check |
| R8 Compatibility, undo | 3.4, 3.6, 3.7 | T1.5, T2.1, T2.3 |
| R9 Docs | n/a | T5.1, T5.2 |
