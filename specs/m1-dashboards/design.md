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
- **P4. Fixed on `main` by #2, but untested.** `sync_everything_to_m1` (`crates/code-reviewer/src/m1.rs`) now runs `cargo install --path .` before `cargo install --path crates/code-reviewer`, so a deploy puts `cheap-otto` on the Mac. The remote script is still an inline `format!` with no test, so a later edit can drop that line unnoticed.
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
    #[error(
        "a dashboard set must match the registry apart from ports: mission-control, pixel-office and reviewer once each, in that order, with the registry's names, labels and managers"
    )]
    UnexpectedServices,
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
#[serde(try_from = "DashboardSetFields")]   // deserializing runs the same checks as with_ports
pub struct DashboardSet { services: Vec<DashboardService> }   // private: built only through validation
// DashboardSet::services(&self) -> &[DashboardService]
// DashboardSet::standard() -> Self
// DashboardSet::with_ports(mission_control: u16, pixel_office: u16, reviewer: u16) -> Result<Self, DashboardError>
// DashboardSet::get(&self, id: DashboardId) -> Option<&DashboardService>
// DashboardSet::ports(&self) -> Vec<u16>
// DashboardSet::managed(&self) -> impl Iterator<Item = &DashboardService>   (manager == CheapOtto)
```

- Labels: `com.cheapotto.dashboards.mission-control`, `com.cheapotto.dashboards.pixel-office`, and `REVIEWER_LAUNCHD_LABEL` for the reviewer. [R2.3]
- `with_ports` rejects 0 and duplicates, and so does deserialization (through `TryFrom<DashboardSetFields>`). Deserialization also rejects any set that is not exactly the canonical registry output of `from_ports` for those ports, meaning the same `id` order, `display_name`, `launchd_label` and `manager`, else `DashboardError::UnexpectedServices`. The `services` field is private, so no set skips these checks. [R2.2]
- Every public function that returns a value carries `#[must_use = "…"]`, as the rest of the core does.
- `build_schedule_install_plan` (`src/schedule.rs`) gains a check: a project equal to `RESERVED_PROJECT_NAME`, ignoring ASCII case because APFS file names are case-insensitive (`Dashboards` would overwrite the same plist file), returns the new variant `ScheduleError::ReservedProjectName(String)`. A schedule whose label would be `REVIEWER_LAUNCHD_LABEL` (project `code-reviewer`, id `ui`, any case) returns `ScheduleError::ReservedLaunchdLabel(String)`. `build_schedule_remove_plan` and `build_schedule_kickstart_spec` run the same check, so only the dashboard paths boot out or kick a dashboard LaunchAgent. [R2.4]

### 3.2 LaunchAgent templates

`render_service_plist_template(service: &DashboardService, set: &DashboardSet) -> Option<String>` returns `None` for a service that cheap-otto does not manage. The text contains the literal placeholders `$HOME_XML` (in place of every `$HOME` inside the plist text) and, for Pixel Office, `$PIXEL_BIN_XML` (in place of `$PIXEL_BIN`). It is a template: the install script writes it through an unquoted heredoc, so the Mac's shell fills those in with XML-escaped values. `launchd` itself does not expand variables, which is why the expansion happens at install time.

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
    <string>/opt/homebrew/bin:/usr/local/bin:$HOME_XML/.cargo/bin:$HOME_XML/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
  </dict>
  <key>ProgramArguments</key>
  <array>
    <string>$HOME_XML/.cargo/bin/cheap-otto</string>
    <string>serve</string>
    <string>--host</string>
    <string>127.0.0.1</string>
    <string>--port</string>
    <string>7420</string>
    <string>--pixel-port</string>
    <string>7421</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$HOME_XML</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>StandardOutPath</key>
  <string>$HOME_XML/.cheap-otto/logs/dashboards/mission-control.log</string>
  <key>StandardErrorPath</key>
  <string>$HOME_XML/.cheap-otto/logs/dashboards/mission-control.log</string>
</dict>
</plist>
```

Pixel Office `ProgramArguments`: `$PIXEL_BIN_XML`, `--port`, `7421`, `--host`, `127.0.0.1`, `--no-terminal` (the flags `run_ui` passes today).

- Ports come from the `DashboardSet`, never from literals in the template code.
- `KeepAlive` and `RunAtLoad` satisfy [R1.1, R1.2, R1.3]; the `127.0.0.1` arguments satisfy [R1.4]; the log paths satisfy [R1.8].
- `ThrottleInterval` 10 keeps a crash loop from spinning.
- Ports (`u16`) and constant labels need no escaping. The shell values `$HOME_XML` and `$PIXEL_BIN_XML` are XML-escaped by the install script before they reach the heredoc (section 3.3), because a valid install path may contain `&`, `<` or `>`.

### 3.3 Install script

`render_install_script(set: &DashboardSet) -> String` returns one POSIX `sh` script. It must run under `/bin/sh` on macOS with `set -eu` and contain no bashisms. Structure, in order:

```sh
set -eu
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
UID_NUM=$(id -u)
AGENTS_DIR="$HOME/Library/LaunchAgents"
LOG_DIR="$HOME/.cheap-otto/logs/dashboards"
mkdir -p "$AGENTS_DIR" "$LOG_DIR"
FAILED=0
# Values written into plist XML must be escaped; plain "$HOME" stays for shell use.
xml_escape() { printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
HOME_XML=$(xml_escape "$HOME")

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
  tmp="$plist.tmp.$"
  # This function is called as an `if` condition, and POSIX turns `set -e` off inside a condition,
  # so every step that can fail is checked here and reported with a cause.
  if ! cat > "$tmp"; then
    echo "dashboard=$id listening=0 cause=plist-write-failed log=$LOG_DIR/$id.log"
    FAILED=1; rm -f "$tmp"; return 1
  fi
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
  # The killed listener may still be exiting; wait for the port to close before bootstrapping.
  n=0
  while [ -n "$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)" ] && [ "$n" -lt 10 ]; do
    n=$((n + 1)); sleep 0.5
  done
  held=$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)
  if [ -n "$held" ]; then
    for pid in $held; do kill -9 "$pid" 2>/dev/null || true; done
    n=0
    while [ -n "$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)" ] && [ "$n" -lt 4 ]; do
      n=$((n + 1)); sleep 0.5
    done
    held=$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null || true)
    if [ -n "$held" ]; then
      # [R1.9, R1.10] The port cannot be freed. Say why, where the log is and what launchd last saw,
      # and return non-zero so the caller skips wait_port: the foreign listener would answer the
      # port probe and this service would be reported as listening.
      held=$(printf '%s' "$held" | tr '\n' ',')
      last_exit=$(launchctl print "gui/$UID_NUM/$label" 2>/dev/null | awk -F'= ' '/last exit code = /{print $2; exit}' | tr ' ' '_')
      echo "dashboard=$id listening=0 cause=port-held pid=$held last_exit=${last_exit:--} log=$LOG_DIR/$id.log"
      FAILED=1
      rm -f "$tmp"
      return 1
    fi
  fi
  if ! mv "$tmp" "$plist"; then
    echo "dashboard=$id listening=0 cause=plist-write-failed log=$LOG_DIR/$id.log"
    FAILED=1; rm -f "$tmp"; return 1
  fi
  if ! launchctl bootstrap "gui/$UID_NUM" "$plist"; then
    echo "dashboard=$id listening=0 cause=bootstrap-failed log=$LOG_DIR/$id.log"
    FAILED=1; return 1
  fi
  echo "dashboard=$id action=installed"
}

# wait_port <id> <label> <port>
wait_port() {
  i=0
  while [ "$i" -lt 20 ]; do
    if nc -z 127.0.0.1 "$3" 2>/dev/null; then echo "dashboard=$1 listening=1"; return 0; fi
    i=$((i + 1)); sleep 0.5
  done
  # [R1.10] A service that never listens is a failure the caller must be able to diagnose:
  # say why, where its log is, and what launchd last saw.
  last_exit=$(launchctl print "gui/$UID_NUM/$2" 2>/dev/null | awk -F'= ' '/last exit code = /{print $2; exit}' | tr ' ' '_')
  echo "dashboard=$1 listening=0 cause=not-listening-after-10s last_exit=${last_exit:--} log=$LOG_DIR/$1.log"
  FAILED=1
}

# --- mission-control ---
if [ -x "$HOME/.cargo/bin/cheap-otto" ]; then
  # install_service returns non-zero when the service could not be installed: port held, plist not
  # written, or bootstrap failed. Calling it in an `if` lets the other services proceed and skips
  # wait_port for a service that did not start. It also disables `set -e` inside the function, which
  # is why install_service checks each step itself.
  if install_service mission-control com.cheapotto.dashboards.mission-control 7420 <<PLIST
…template from 3.2…
PLIST
  then
    wait_port mission-control com.cheapotto.dashboards.mission-control 7420
  fi
else
  echo "dashboard=mission-control action=skipped reason=cheap-otto-not-installed"
fi

# --- pixel-office ---  [R1.6]
PIXEL_BIN=$(command -v pixel-agents 2>/dev/null || true)
if [ -n "$PIXEL_BIN" ]; then
  PIXEL_BIN_XML=$(xml_escape "$PIXEL_BIN")
  …pixel-agents config snippet, see below…
  if install_service pixel-office com.cheapotto.dashboards.pixel-office 7421 <<PLIST
…template…
PLIST
  then
    wait_port pixel-office com.cheapotto.dashboards.pixel-office 7421
  fi
else
  echo "dashboard=pixel-office action=skipped reason=pixel-agents-not-installed"
fi

if [ "$FAILED" -ne 0 ]; then exit 4; fi
```

- The heredoc delimiter is unquoted (`<<PLIST`) on purpose, so `$HOME_XML` and `$PIXEL_BIN_XML` expand. Shell paths used outside XML (the `$HOME/.cargo/bin/cheap-otto -x` test, `mkdir`) keep plain `$HOME`.
- Strays are found by port with `lsof`, not by `pkill -f <pattern>`: the script's own text travels in the argv of the remote shell, so a pattern match would kill the installer.
- The pixel-agents config snippet is the one `run_ui` runs today, moved here unchanged. Store it as a raw string constant so no brace escaping is needed:
  ```rust
  const PIXEL_AGENTS_CONFIG_SNIPPET: &str = r#"if command -v node >/dev/null 2>&1; then
    mkdir -p "$HOME/.pixel-agents"
    node -e 'const fs=require("fs"),p=process.env.HOME+"/.pixel-agents/config.json";let c={};try{c=JSON.parse(fs.readFileSync(p,"utf8"))}catch{};c.standalone=Object.assign({},c.standalone,{watchAllSessions:true});c.hooksConsent=Object.assign({},c.hooksConsent,{claude:"granted"});fs.writeFileSync(p,JSON.stringify(c,null,2));'
  fi"#;
  ```
- Exit codes: 0 when every step succeeded (skips are reported), 3 for no GUI session, 4 when a managed service could not be installed or never listens (port held, plist not written, bootstrap failed, or not listening after 10 s); the other services are still attempted. Nothing in `install_service` relies on `set -e`, because it is off inside an `if` condition. Provisioning (3.10) treats any non-zero as a warning [R4.3]; `ui` treats it as fatal [R3.10].

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
dashboard=mission-control loaded=1 pid=812 listening=1 last_exit=-
dashboard=pixel-office loaded=0 pid=- listening=0 last_exit=-
dashboard=reviewer loaded=1 pid=- listening=0 last_exit=78:_EX_CONFIG
host cheap_otto_bin=1 pixel_agents_bin=0 gui_session=1 autologin_user=admin
```

  - `loaded`: exit status of `launchctl print "gui/$UID_NUM/<label>"`.
  - `pid`: `launchctl print … | awk '/^[[:space:]]*pid = /{print $3; exit}'`, or `-`.
  - `listening`: `nc -z 127.0.0.1 <port>`.
  - `last_exit`: `launchctl print … | awk -F'= ' '/last exit code = /{print $2; exit}' | tr ' ' '_'`, or `-`. The same expression the install script uses.
  - `autologin_user`: `defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser 2>/dev/null`, or `-`. [R5.2]
- `parse_status_output(raw: &str, set: &DashboardSet) -> DashboardStatusReport` is pure and tolerant: it splits each line on whitespace into `key=value` tokens, ignores unknown lines, keys and ids, and treats a missing key as false or `None`.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardRemoteStatus { pub id: DashboardId, pub loaded: bool, pub pid: Option<u32>, pub listening: bool, pub last_exit: Option<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardHostFacts { pub cheap_otto_bin: bool, pub pixel_agents_bin: bool, pub gui_session: bool, pub autologin_user: Option<String> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardStatusReport { pub dashboards: Vec<DashboardRemoteStatus>, pub host: Option<DashboardHostFacts> }
```

`status` never shows an unavailable dashboard without a reason. [R5.1] Two more pure functions do that:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DashboardDiagnosis { pub reason: String, pub log: String }

/// `None` when the dashboard is listening. Otherwise the first cause that applies, in this order.
pub fn explain_unavailable(service: &DashboardService, status: &DashboardRemoteStatus, host: Option<&DashboardHostFacts>) -> Option<DashboardDiagnosis>

/// One printable row per dashboard: loaded, pid, listening, tunnelled, and the diagnosis when there is one.
pub fn render_status_rows(report: &DashboardStatusReport, set: &DashboardSet, tunnelled: &[(DashboardId, bool)]) -> Vec<String>
```

| Order | Condition | Reason |
| --- | --- | --- |
| 1 | `host.gui_session` is false | No GUI session for the Mac user; log in to the Mac desktop or enable automatic login |
| 2 | Mission Control and `host.cheap_otto_bin` is false | `cheap-otto` is not installed on the Mac; run `code-reviewer m1 sync` |
| 3 | Pixel Office and `host.pixel_agents_bin` is false | `pixel-agents` is not installed on the Mac, so the service was skipped |
| 4 | `loaded` is false | The LaunchAgent is not loaded; run `cheap-otto dashboards install --execute` (for the Reviewer: `code-reviewer m1 sync`). This is also what a failed or port-held install leaves behind |
| 5 | `loaded` is true and `listening` is false | Loaded but not listening, with `last_exit` when there is one |

Rows 1 to 3 are skipped when the `host` line is missing. `log` is `~/.cheap-otto/logs/dashboards/<id>.log` for managed services and `~/.cheap-otto/logs/code-reviewer/ui-server.log` for the Reviewer; it is always given, so the operator knows where to look whatever the reason.

The CLI passes its local port checks in as `tunnelled` and adds the warning for [R5.3].

### 3.5 Tunnel

```rust
/// ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -o ServerAliveCountMax=3
///     -o BatchMode=yes -o ConnectTimeout=10 -p <ssh_port> -L 127.0.0.1:P:127.0.0.1:P … <user@host>
pub fn build_tunnel_spec(peer: &PeerNode, set: &DashboardSet) -> CommandSpec

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelRecord { pub pid: u32, pub ports: Vec<u16>, pub destination: String, pub ssh_port: u16, pub started_epoch_secs: u64 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelDecision {
    /// Every port answers locally and the recorded tunnel is a live ssh process that covers them.
    AlreadyUp,
    /// No port answers locally.
    Start,
    /// Some ports answer and some do not.
    Conflict { held: Vec<u16>, missing: Vec<u16> },
}
/// `recorded_tunnel_covers` is the result of `recorded_tunnel_covers(record, ssh_alive, set, peer)`: the record exists, its pid is a
/// live ssh process, its `destination` and `ssh_port` equal the current peer's, and its ports include every port in the set.
pub fn decide_tunnel(local_ports: &[(u16, bool)], recorded_tunnel_covers: bool) -> TunnelDecision

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelFailure { ExitedEarly { status: String }, TimedOut { waited_secs: u64 } }
/// Always names the ports that did not answer and the command that tests SSH by hand.
pub fn describe_tunnel_failure(failure: &TunnelFailure, unanswered: &[u16], ssh_test_command: &str) -> String

/// True only when `command_line` is an ssh invocation that names the record's destination and carries
/// `127.0.0.1:<port>:127.0.0.1:<port>` for every recorded port. Guards against a recycled pid.
pub fn command_line_is_recorded_tunnel(command_line: &str, record: &TunnelRecord) -> bool
```

- The tunnel forwards every port in the set, whether or not that service is installed on the Mac. An SSH forward listens locally even when nothing answers remotely, so every port answering locally is evidence that a tunnel is up only when the recorded tunnel is a live ssh process that covers the ports. Three unrelated listeners must not be mistaken for the tunnel: with all ports answering and no such record, `decide_tunnel` returns `Conflict { held: <every port>, missing: [] }`. [R3.1, R3.2]
- `BatchMode=yes` stops a detached process from hanging on a password prompt, and `ConnectTimeout=10` stops it from hanging on an unreachable host. The destination is `user@host`, not the alias. [R3.3]
- `tunnel.json` lives in the state directory on the workstation. [R3.4]
- A record made for another Mac must never be reused. After re-pairing, every port can still answer through the old tunnel, so `recorded_tunnel_covers` also requires `record.destination == peer.ssh_destination()` and `record.ssh_port == peer.ssh_port`. A mismatch makes the decision a `Conflict`; step 3 below stops the recorded tunnel and step 4 starts one to the current peer. [R3.2, R3.5]

The imperative part lives in `src/main.rs` as `ensure_dashboard_tunnel(state_dir, peer, set) -> Result<(), Box<dyn Error>>`:

1. Probe each port with `is_local_port_open`, load `tunnel.json`, compute `recorded_tunnel_covers`, and call `decide_tunnel`.
2. `AlreadyUp`: print it and return.
3. `Conflict`: load `tunnel.json`. If `recorded_tunnel_is_running(record)` is true (the pid is alive **and its command line is the recorded tunnel's**, see below), terminate it, wait up to 2 s for the ports to close, and continue to step 4. Otherwise start nothing and return an error naming `held` (every port, when all of them answer without a recorded tunnel) and printing how to find the holder: `netstat -ano | findstr :<port>` on Windows, `lsof -nP -iTCP:<port> -sTCP:LISTEN` elsewhere. [R3.5]
4. `Start`: spawn the `CommandSpec` with null stdio, write `tunnel.json` with `child.id()`, then poll for up to 5 s. If every port answers, succeed. Both failure paths delete `tunnel.json` and return the message from `describe_tunnel_failure`, which names the ports that did not answer and the SSH test command (`ssh -p <port> <user@host> true`): if `child.try_wait()` reports an exit, that is `TunnelFailure::ExitedEarly`; if the deadline passes with the child still alive, call `terminate_process(child.id())` first, then report `TunnelFailure::TimedOut`. [R3.6]

Process helpers, also in `src/main.rs` (the reviewer's `state.rs` has the same idea around line 430):

| Helper | Windows | Elsewhere |
| --- | --- | --- |
| `process_command_line(pid) -> Option<String>` | `powershell -NoProfile -Command "(Get-CimInstance Win32_Process -Filter 'ProcessId=<pid>').CommandLine"` | `ps -p <pid> -o args=` |
| `terminate_process(pid)` | `taskkill /PID <pid> /F` | `kill <pid>` |

`recorded_tunnel_is_running(record) -> bool` is `process_command_line(record.pid)` passed to the pure `command_line_is_recorded_tunnel`. That is the `ssh_alive` input of `recorded_tunnel_covers`. `<pid>` is a `u32`, so nothing operator-supplied reaches these commands.

An image name is not enough: after a reboot the recorded pid can belong to an unrelated ssh session, and killing it would drop someone's shell. The command line must carry the recorded destination and every recorded forward. A record that cannot be verified (process gone, command line unreadable or different) is stale: remove `tunnel.json` and signal nothing. If ports are still held after that, it is the foreign-holder error in step 3. [R3.5]

### 3.6 CLI

New subcommand group in `src/main.rs`, following rule 05 (doc comments are help text, dry-run by default, one `XOptions` struct per handler):

| Command | Effect | Gate |
| --- | --- | --- |
| `cheap-otto dashboards list [--json]` | Print the registry | none (pure) |
| `cheap-otto dashboards install` | Ensure the managed LaunchAgents on the Mac | `--execute` |
| `cheap-otto dashboards install-script` | Print the install script to stdout, nothing else | none (pure) |
| `cheap-otto dashboards status [--json]` | Report per [R5] | `--execute` runs the read-only SSH probe and checks which ports answer locally; without it, print the probe command only |
| `cheap-otto dashboards restart --id <id>` | `launchctl kickstart -k` | `--execute` |
| `cheap-otto dashboards uninstall` | Remove the managed LaunchAgents | `--execute` |
| `cheap-otto dashboards tunnel [--stop]` | Ensure the tunnel, or stop the recorded one | `--execute` |
| `cheap-otto ui [--open <id>]` | Install, tunnel, open the browser | `--execute` |

Shared flags: `--state-dir`, `--port` (Mission Control) and `--pixel-port`, each defaulting to the registry constants. The Reviewer port is always the `REVIEWER_PORT` constant (3.7 step 1), because the provisioner-managed Reviewer LaunchAgent is fixed at 7430. `--execute` has `visible_alias = "apply"`. [R8.4]

**A dry run opens no socket.** Without `--execute`, no command probes a local port or contacts the Mac: it prints commands and reads only local state files. That keeps the CLI tests hermetic even though the Reviewer port is a fixed constant. [R8.4]

### 3.7 `cheap-otto ui`

`run_ui` becomes:

1. Resolve the state dir and peer; build `DashboardSet::with_ports(port, pixel_port, REVIEWER_PORT)`.
2. Build the install plan and the tunnel spec. Print the peer, the three URLs, the plan's `display_steps` and the tunnel command.
3. Without `--execute`: print the hint and return.
4. Run the install plan with `execute_command_spec`. The script's `dashboard=… action=…` lines go to the terminal. If it exits non-zero, return the error immediately (the script's `error=` and `dashboard=…` lines are already on the terminal) and do not touch the ssh config, tunnel or browser. [R3.10]
5. If `peer.json` exists, rewrite the managed `~/.ssh/config` block (now without forwards, section 3.8).
6. `ensure_dashboard_tunnel`.
7. Run the status probe (`build_status_spec` + `parse_status_output`) and print a warning for every dashboard with `listening=false`, naming `~/.cheap-otto/logs/dashboards/<id>.log` for managed ones. The warning is non-blocking. [R3.10]
8. Unless `--no-open`, open `http://localhost:<port>` for `--open <id>` (default `mission-control`).

The inline tmux startup script and the hand-rolled `ssh -N` in today's `run_ui` are deleted. [R3.1, R8.1]

### 3.8 SSH config block

`PeerNode::build_port_tunnel` stops writing `LocalForward` lines into `ssh_config_snippet`; the snippet keeps `Host`, `HostName`, `User`, `Port`, `ServerAliveInterval` and `ServerAliveCountMax`. `tunnel_spec` and `ssh_tunnel_command` are unchanged, so `cheap-otto pair --tunnel` still forwards the dev ports explicitly. `merge_ssh_config_block` already replaces everything between its markers, so the next `pair --apply` or `ui --execute` removes forwards an older version wrote. [R3.8]

This changes behaviour: a plain `ssh otto-mac` no longer forwards 3000, 5173 and 8080. The release notes and README must say that `cheap-otto pair --tunnel` is how to get them.

### 3.9 Mission Control: hub API and links

- `GET /api/dashboards` in `build_router` (`src/server.rs`). The server builds the `DashboardSet` with `with_ports` from `state.config.port`, `state.config.pixel_port` and `REVIEWER_PORT` during startup and refuses to start on an invalid configuration (port 0 or duplicate). The handler returns HTTP 500 with `{"error":"…"}` if it ever sees an invalid set; it never falls back to default ports. For a valid set it probes `127.0.0.1:<port>` for each with a 250 ms timeout (the pattern `handle_pixel_office` uses), and returns:
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

All changes remain synchronous and avoid adding async runtimes, HTTP clients, or TLS stacks (rule 05).

- **Guard the two-binary deploy** [R4.1]. Since #2, `sync_everything_to_m1` (`crates/code-reviewer/src/m1.rs`) installs both binaries: `cargo install --path . --locked --force`, then `cargo install --path crates/code-reviewer --locked --force`. Extract the remote build script into a pure `fn render_remote_build_script(remote_dir_expr: &str, brew: &str, cargo: &str, npm: &str) -> String` with identical output, and test that both install lines come before `m1-provision.sh`.
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
  3. Decide whether to delegate with a pure `fn should_delegate_to_cheap_otto(requested_host: Option<&str>, peer_host: Option<&str>, port: u16) -> bool`: true only when the requested host is none or equals the peer recorded in `peer.json`, and `port == DEFAULT_UI_PORT` (7430). `cheap-otto ui` has no `--host` and always uses `REVIEWER_PORT`, so any other host or port cannot be expressed to it. When true, find `cheap-otto` (a file named `cheap-otto` or `cheap-otto.exe` beside `std::env::current_exe()`, else `cheap-otto` on `PATH`) and run `cheap-otto ui --execute --no-open`, adding `--state-dir <dir>` when one was given, with inherited stdio, and wait for it. Every process this path spawns (the `cheap-otto --version` probe on `PATH`, the delegated `cheap-otto ui`, and the fallback `ssh` in step 4) is built by a small function and passed through `strip_github_credentials` first, so none of them inherits `GH_TOKEN` or `GITHUB_TOKEN` from `code-reviewer`.
  4. If delegation is skipped (custom host or port) or `cheap-otto` is not found, spawn a fallback tunnel built by a pure `fn reviewer_fallback_tunnel_args(host: &str, port: u16) -> Vec<String>` that forwards only the requested reviewer port to the requested host. The fallback never forwards Pixel Office and is never started when step 1 found the Reviewer port answering.
  5. Poll the local port for up to 5 s. The decision each tick is a pure `fn fallback_poll_step(port_open: bool, child_exited: bool, deadline_passed: bool) -> FallbackStep` (`Ready`, `Wait`, `FailExited`, `FailTimedOut`). On either failure, kill the fallback child if it is still running and `wait()` on it so no ssh process is left behind, then return an error that names the port. Print "ready" and open a browser only on `Ready`. [R3.9]
- **Pixel Office start button**. On macOS, `start_pixel_agents_server` runs `launchctl kickstart "gui/<uid>/com.cheapotto.dashboards.pixel-office"` (uid from `id -u`). If that succeeds it waits for the port as today. If it fails it returns an error that says the service is not installed and names `cheap-otto dashboards install --execute`; it does **not** spawn `pixel-agents` itself, because a process started that way lives outside `launchd` and would hold the port the service needs. The direct spawn remains only on non-macOS hosts, where there is no LaunchAgent. The label is a constant in the reviewer crate, checked against the registry by a test in the root crate (`tests/dashboard_labels.rs`), like the ports.
- **`scripts/open-ui.ps1`** becomes a thin wrapper over `code-reviewer ui`, so it follows the same rules (reuse the existing tunnel when the Reviewer port answers; otherwise delegation or the single-port fallback), and never forwards Pixel Office itself: `& cargo run --quiet -- ui --host $HostAddr --port $Port` (plus `--no-open` when `-NoOpen` is set), run from the crate directory, checking `$LASTEXITCODE`.
- **Ports and link** [R2.5, R6.3]. Add `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = 7420;` beside `DEFAULT_UI_PORT` in `crates/code-reviewer/src/server.rs`, and a link to Mission Control in `DASHBOARD_HTML` built from `location.hostname` and that port (text through `esc`). A test in the root crate, `tests/dashboard_ports.rs`, uses `include_str!("../crates/code-reviewer/src/server.rs")` and asserts the literal declarations of `DEFAULT_UI_PORT`, `DEFAULT_PIXEL_PORT` and `DEFAULT_MISSION_CONTROL_PORT` carry the registry's values.

## 4. Security

- The origin and host guard from the preceding change stays as it is. This spec adds one `GET` route and no CORS. [R7]
- That guard is `RequestGuard` (`src/request_guard.rs`, held in `ControlPlaneState::request_guard`; the reviewer has its own copy in `crates/code-reviewer/src/request_guard.rs`). Its allowlist is keyed on the listening port, which is one more reason the tunnel must forward each port to the same number. No task here extends the allowlist, because no dashboard calls another dashboard's API.
- Router tests must send a loopback `Host` header, and a `POST` body to the reviewer must be `Content-Type: application/json`, or the guard refuses the request.
- Every generated listener argument is `127.0.0.1`; tests assert `0.0.0.0` never appears in a plist or script. [R1.4]
- The install script is assembled from constants and `u16` ports. It takes no operator text, so there is nothing to inject. `--id` values are parsed into `DashboardId` before any command is built.
- The install script kills processes by port. That is deliberate and visible: each kill prints `stray_killed=<pid>`.
- The existing `cheap-otto serve --host` and `code-reviewer serve --bind` overrides can still bind a non-loopback address; this spec does not close them, and they are tracked in a separate follow-up issue.
- The tunnel uses `BatchMode=yes`, `ConnectTimeout=10` and discrete args. The terminate helper refuses to kill a pid whose image is not ssh.
- `status` reads `autoLoginUser`; nothing in this spec writes login settings.

## 5. Failure handling

| Situation | Behaviour |
| --- | --- |
| No GUI session on the Mac | Install script prints `error=no-gui-session` and a hint, exits 3; the CLI returns the error. [R1.7] |
| `cheap-otto` not installed on the Mac | Mission Control skipped with `reason=cheap-otto-not-installed`; CLI adds "run `code-reviewer m1 sync`". |
| `pixel-agents` not installed | Pixel Office skipped with a reason; other services installed. [R1.6] |
| Service never listens | Install script prints `listening=0`, still attempts the other services, and exits 4; `ui` stops before the tunnel and browser and points at `~/.cheap-otto/logs/dashboards/<id>.log`. [R1.10, R3.10] |
| Port still held after `kill -9` | Install script prints `dashboard=<id> listening=0 cause=port-held pid=<pid> last_exit=<…> log=<path>`, skips that service without bootstrapping or probing its port, and exits 4. [R1.9, R1.10] |
| LaunchAgent cannot be written or bootstrapped | Install script prints `dashboard=<id> listening=0 cause=plist-write-failed` or `cause=bootstrap-failed` with the log path, does not print `action=installed`, continues with the other services, and exits 4. [R1.10] |
| Workstation port held by a foreign process | Error naming the ports and the command to find the holder; nothing is killed. [R3.5] |
| Tunnel exits early or times out | Error with the SSH test command (a timeout also names the ports that did not answer); a live child is terminated; `tunnel.json` removed; no browser. [R3.6] |
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
| Cross-crate | `tests/dashboard_ports.rs`, `tests/dashboard_labels.rs` | Reviewer port constants and the Pixel Office LaunchAgent label equal the registry |

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
| D3 | Only `cheap-otto` opens dashboard tunnels; `code-reviewer ui` delegates | Making `code-reviewer` depend on the root library would pull tokio and axum into a crate that must stay synchronous and free of async runtimes, HTTP clients and TLS stacks. |
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
