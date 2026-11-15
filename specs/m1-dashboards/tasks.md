# Tasks: dashboards served from the M1

Tracking issue: #6. Each task below has its own issue.

Read `../README.md` (how to carry out a task), then `requirements.md` and `design.md`. Section numbers below, such as "design 3.3", refer to `design.md`.

**Rules for every task**

- One task, one branch, one PR. Branch name: `m1-dashboards/<task-id>-<slug>`, for example `m1-dashboards/t1-3-plist-templates`.
- The gate must pass before you open the PR:
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets
  ```
- Use the exact test names given. Add more tests if you need them; do not rename or drop the listed ones.
- Never pass `--execute` or `--apply`, and never run `code-reviewer m1 …`, `poll`, `tend-poll`, `review`, `tend` or `ui`. Only tasks marked **LIVE** touch the real MacBook, and only the owner runs them.
- No CORS headers, no bind address other than `127.0.0.1`, no new dependencies, no `.unwrap()` or `.expect()` outside tests.
- When done: tick the box here, add `PR: #NN` under the task, and close the issue from the PR.

## Phase 0: prerequisite

### T0.1 Origin and host guard on both dashboard servers

- [ ] Done
- **Issue:** #7
- **Covers:** R7.1
- **Depends on:** nothing
- **Status:** in progress in its own session; the PR is stacked on #4.
- **Goal:** `cheap-otto serve` and `code-reviewer serve` reject state-changing requests from a foreign `Origin` and any request with a non-loopback `Host`; `CorsLayer::permissive()` is gone.
- **Acceptance:**
  - The PR titled `fix(security): reject cross-origin requests to local control servers` is merged.
  - `grep -n "CorsLayer::permissive" src/server.rs` prints nothing.
- **Why it comes first:** T3.1, T4.3, T4.4 and T4.6 edit the same two `server.rs` files. Start them after this merges.

## Phase 1: core planners (root crate, pure code)

All of Phase 1 lives in the functional core. No task here spawns a process, reads a file or opens a socket outside tests.

### T1.1 Dashboard registry

- [ ] Done
- **Issue:** #8
- **Covers:** R2.1, R2.2, R2.3
- **Depends on:** nothing
- **Files:** `src/dashboards.rs` (new), `src/lib.rs`
- **Steps:**
  1. Create `src/dashboards.rs` with a `//!` module header and the constants, `DashboardError`, `DashboardId`, `ServiceManager`, `DashboardService` and `DashboardSet` exactly as in design 3.1.
  2. Implement `DashboardId::all`, `slug`, `display_name`, `parse`, and `DashboardSet::standard`, `with_ports`, `get`, `ports`, `managed`. `standard()` calls `with_ports` with the three constants.
  3. Mission Control and Pixel Office use `ServiceManager::CheapOtto` and the label `com.cheapotto.dashboards.<slug>`. Reviewer uses `ServiceManager::CodeReviewerProvisioner` and `REVIEWER_LAUNCHD_LABEL`.
  4. Add `pub mod dashboards;` to `src/lib.rs` (keep the list alphabetical).
- **Tests** in `src/dashboards.rs`:
  - `standard_set_lists_three_dashboards_with_default_ports`
  - `rejects_zero_and_duplicate_ports`
  - `managed_labels_use_reserved_dashboards_namespace`
  - `parses_dashboard_ids_and_rejects_unknown`
  - `set_serializes_with_kebab_case_ids`
- **Acceptance:**
  - `cargo test -p cheap-otto --lib dashboards::tests` passes with at least 5 tests.
  - `grep -c "Command::new\|std::fs" src/dashboards.rs` prints `0`.

### T1.2 Reserve the `dashboards` project name

- [ ] Done
- **Issue:** #9
- **Covers:** R2.4
- **Depends on:** T1.1
- **Files:** `src/schedule.rs`
- **Steps:**
  1. Add the variant `ReservedProjectName(String)` to `ScheduleError` with the message `project name '{0}' is reserved for dashboard services`.
  2. In `build_schedule_install_plan`, after the project name is validated, return that error when it equals `crate::dashboards::RESERVED_PROJECT_NAME`.
- **Tests** in `src/schedule.rs`:
  - `rejects_schedule_for_reserved_dashboards_project`
- **Acceptance:** `cargo test -p cheap-otto --lib schedule::tests` passes and includes the new test.

### T1.3 LaunchAgent templates

- [ ] Done
- **Issue:** #10
- **Covers:** R1.1, R1.2, R1.3, R1.4, R1.8
- **Depends on:** T1.1
- **Files:** `src/dashboards.rs`
- **Steps:**
  1. Add `pub fn render_service_plist_template(service: &DashboardService, set: &DashboardSet) -> Option<String>` with `#[must_use]`.
  2. Return the XML in design 3.2 for Mission Control and the Pixel Office variant; return `None` for the reviewer.
  3. Take ports from `set`. Keep `$HOME` and `$PIXEL_BIN` as literal text.
- **Tests** in `src/dashboards.rs`:
  - `mission_control_plist_keeps_alive_and_binds_loopback`: contains the label, `<key>KeepAlive</key>` followed by `<true/>`, `<key>RunAtLoad</key>`, `<string>127.0.0.1</string>`, the log path, and no `0.0.0.0`.
  - `pixel_office_plist_uses_resolved_binary_placeholder`: contains `<string>$PIXEL_BIN</string>` and `--no-terminal`.
  - `reviewer_has_no_template_because_provisioner_manages_it`
  - `plist_ports_follow_the_set`: with `with_ports(8420, 8421, 8430)` the Mission Control template contains `8420` and `8421` and not `7420`.
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.4 Install script and install plan

- [ ] Done
- **Issue:** #11
- **Covers:** R1.5, R1.6, R1.7, R1.9, R4.4
- **Depends on:** T1.3
- **Files:** `src/dashboards.rs`, `tests/dashboard_scripts.rs` (new)
- **Steps:**
  1. Add `PIXEL_AGENTS_CONFIG_SNIPPET` as in design 3.3.
  2. Add `pub fn render_install_script(set: &DashboardSet) -> String` producing the script in design 3.3, in that order: header, GUI-session guard, legacy tmux cleanup, `install_service`, `wait_port`, Mission Control block, Pixel Office block. Labels and ports come from `set`.
  3. Add `DashboardPlan` and `pub fn build_install_plan(peer: &PeerNode, set: &DashboardSet) -> DashboardPlan`. The spec is `CommandSpec::new("Install dashboard services on MacBook", "ssh", ["-p", <port>, <user@host>, <remote>])` with `remote = format!("/bin/sh -c {}", posix_single_quote(&script))`.
  4. `display_steps` has one line per managed service: `Ensure LaunchAgent <label> -> 127.0.0.1:<port>`.
  5. Create `tests/dashboard_scripts.rs` with `#![forbid(unsafe_code)]`. Under `#[cfg(unix)]`, write the rendered script to a temp file and assert `sh -n <file>` exits 0.
- **Tests:**
  - in `src/dashboards.rs`:
    - `install_script_is_posix_and_guards_gui_session`: starts with `set -eu`; contains `error=no-gui-session` and `exit 3`; contains no `[[`.
    - `install_script_skips_unchanged_services_and_reports_actions`: contains `cmp -s`, `action=unchanged`, `action=installed`.
    - `install_script_skips_pixel_office_when_binary_missing`: contains `reason=pixel-agents-not-installed`.
    - `install_script_frees_ports_by_listener_not_by_name`: contains `lsof -nP -iTCP:` and no `pkill`.
    - `install_script_never_binds_non_loopback`: no `0.0.0.0`.
    - `install_plan_wraps_script_for_remote_sh`: program is `ssh`; the last arg starts with `/bin/sh -c '`.
  - in `tests/dashboard_scripts.rs`: `rendered_install_script_passes_sh_syntax_check`
- **Acceptance:**
  - `cargo test -p cheap-otto --lib dashboards::tests` passes.
  - On Linux or macOS, `cargo test -p cheap-otto --test dashboard_scripts` passes. On Windows it compiles and runs 0 tests.

### T1.5 Uninstall and restart plans

- [ ] Done
- **Issue:** #12
- **Covers:** R8.2
- **Depends on:** T1.4
- **Files:** `src/dashboards.rs`, `tests/dashboard_scripts.rs`
- **Steps:**
  1. Add `render_uninstall_script(set)` and `build_uninstall_plan(peer, set) -> DashboardPlan` (design 3.4). Only managed services; print `dashboard=<id> action=removed`.
  2. Add `build_restart_spec(peer: &PeerNode, set: &DashboardSet, id: DashboardId) -> Option<CommandSpec>` running `launchctl kickstart -k "gui/$(id -u)/<label>"` over SSH. It works for all three ids and returns `None` only if the id is not in the set.
  3. Add a syntax-check test for the uninstall script.
- **Tests:**
  - `uninstall_script_removes_only_managed_agents`: contains both `com.cheapotto.dashboards.` labels and does not contain `com.cheapotto.code-reviewer.ui`.
  - `restart_spec_kickstarts_requested_label`: for `Reviewer` the remote command contains `kickstart -k` and `com.cheapotto.code-reviewer.ui`.
  - in `tests/dashboard_scripts.rs`: `rendered_uninstall_script_passes_sh_syntax_check`
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.6 Status script and parser

- [ ] Done
- **Issue:** #13
- **Covers:** R5.1, R5.2, R5.5
- **Depends on:** T1.1
- **Files:** `src/dashboards.rs`, `tests/dashboard_scripts.rs`
- **Steps:**
  1. Add `render_status_script(set)` and `build_status_spec(peer, set) -> CommandSpec`, printing the lines in design 3.4. Use `BatchMode=yes` and `ConnectTimeout=3` like `PeerNode::build_ssh_probe_spec`.
  2. Add `DashboardRemoteStatus`, `DashboardHostFacts`, `DashboardStatusReport` and `pub fn parse_status_output(raw: &str, set: &DashboardSet) -> DashboardStatusReport`.
  3. The parser never panics: unknown lines, keys and ids are ignored; `pid=-` and unparsable pids become `None`; `autologin_user=-` becomes `None`.
- **Tests:**
  - `status_script_is_read_only`: contains none of `bootstrap`, `bootout`, `kickstart`, `kill `, `rm `, `mv `.
  - `parses_status_lines_for_all_dashboards`: the four sample lines in design 3.4 give three dashboards and the host facts.
  - `ignores_unknown_lines_keys_and_ids`
  - `missing_host_line_yields_none`
  - `handles_empty_and_garbage_output_without_panicking`: `""`, `"\n\n"`, `"dashboard="`, `"dashboard=reviewer pid=abc loaded=maybe"`.
  - in `tests/dashboard_scripts.rs`: `rendered_status_script_passes_sh_syntax_check`
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.7 Tunnel spec, decision and record

- [ ] Done
- **Issue:** #14
- **Covers:** R3.2, R3.3, R3.4
- **Depends on:** T1.1
- **Files:** `src/dashboards.rs`
- **Steps:**
  1. Add `build_tunnel_spec`, `TunnelRecord`, `TunnelDecision` and `decide_tunnel` as in design 3.5.
  2. Argument order: `-N`, the four `-o` options, `-p <ssh_port>`, one `-L 127.0.0.1:P:127.0.0.1:P` per port in set order, then `user@host` last.
  3. `decide_tunnel`: all true → `AlreadyUp`; all false or an empty slice → `Start`; otherwise `Conflict` with `held` (true) and `missing` (false) in input order.
- **Tests:**
  - `tunnel_spec_forwards_every_port_on_loopback_with_safe_options`: three `-L` args, each starting and continuing with `127.0.0.1:`; contains `ExitOnForwardFailure=yes`, `ServerAliveInterval=30`, `ServerAliveCountMax=3`, `BatchMode=yes`; the last arg is `user@host`.
  - `decides_already_up_start_and_conflict`
  - `tunnel_record_round_trips_as_json`
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.8 Stop writing `LocalForward` into the SSH config block

- [ ] Done
- **Issue:** #15
- **Covers:** R3.8
- **Depends on:** nothing
- **Files:** `src/bridge.rs`, `tests/cli_integration.rs` (only if an assertion there mentions `LocalForward`)
- **Steps:**
  1. In `PeerNode::build_port_tunnel`, stop building `ssh_config_forwards` and drop it from `ssh_config_snippet`. Leave `tunnel_spec`, `ssh_tunnel_command` and `ports` as they are.
  2. Update the rustdoc on `PortTunnelPlan::ssh_config_snippet` to say it carries no forwards and why (design 3.8).
  3. In the existing test `builds_remote_exec_and_port_tunnel_commands`, replace the `LocalForward` assertion: the snippet must not contain `LocalForward`, and `tunnel_spec` args must contain `127.0.0.1:5173:127.0.0.1:5173`.
- **Tests** in `src/bridge.rs`:
  - `merged_block_drops_local_forwards_written_by_older_versions`: start from a config whose managed block (between the `# --- BEGIN cheap-otto (otto-mac) ---` and `# --- END …` markers) contains two `LocalForward` lines, plus an unrelated `Host other` block that also has a `LocalForward`. After `merge_ssh_config_block` with a new snippet, the managed block has none and the unrelated block is untouched.
- **Acceptance:**
  - `cargo test -p cheap-otto --lib bridge::tests` passes.
  - `grep -n "LocalForward" src/bridge.rs` shows matches only in doc comments or below the `#[cfg(test)]` line.
  - Dry run prints no forwards: `cargo run -q -p cheap-otto -- pair --host 192.168.1.42 --out-dir <tmp>` output contains no `LocalForward`.

## Phase 2: CLI (root crate, `src/main.rs`)

### T2.1 `dashboards` subcommands

- [ ] Done
- **Issue:** #16
- **Covers:** R1.1, R4.4, R5.1 to R5.5, R8.2, R8.4
- **Depends on:** T1.4, T1.5, T1.6
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Add `Commands::Dashboards { #[command(subcommand)] action: DashboardCommands }` and the enum with `List`, `Install`, `InstallScript`, `Status`, `Restart`, `Uninstall` (design 3.6). Every variant and flag gets a `///` help line. Shared flags: `--state-dir`, `--port`, `--pixel-port`, `--reviewer-port` (defaults from the registry constants); `--execute` with `visible_alias = "apply"` on install, status, restart, uninstall; `--json` on list and status; `--id` on restart.
  2. One handler per subcommand taking one options struct, following the pattern of `run_delegate`. Each prints the plan first and only then checks `execute`.
  3. `install-script` prints `render_install_script` to stdout and nothing else (no banner), so it can be piped to `sh`.
  4. `status` without `--execute`: print the probe command and, per dashboard, whether its port answers locally (`is_local_port_open`). With `--execute`: run the probe with `capture_spec_stdout`, parse it, and print one row per dashboard with `loaded`, `pid`, `listening`, `tunnelled`. Print the warning for R5.3 when `autologin_user` is `None`, and the hints in design section 5 when a binary is missing or a service is loaded but not listening.
  5. `status --json` prints one JSON object (`{"dashboards":[…],"host":…}`) and nothing else on stdout.
- **Tests** in `tests/cli_integration.rs` (temp dir; create a peer first with `pair --host 192.168.1.42 --out-dir <tmp>`; pass ports 47420, 47421, 47430 so nothing real is probed):
  - `dashboards_list_and_install_script_are_pure`: `list --json` parses and has three entries; `install-script` stdout starts with `set -eu` and contains both managed labels.
  - `dashboards_install_status_restart_uninstall_are_dry_runs_without_execute`: each exits 0, prints an `ssh` command, and the temp dir gains no `tunnel.json`.
  - `dashboards_restart_rejects_unknown_id`: `restart --id nope` exits non-zero and stderr names the three valid ids.
- **Acceptance:**
  - `cargo test -p cheap-otto --test cli_integration dashboards_` passes.
  - `cargo run -q -p cheap-otto -- dashboards install-script | head -1` prints `set -eu`.
  - `cargo run -q -p cheap-otto -- dashboards --help` lists the six subcommands.

### T2.2 Tunnel management

- [ ] Done
- **Issue:** #17
- **Covers:** R3.1 to R3.7
- **Depends on:** T1.7
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Add `load_tunnel_record(state_dir) -> Option<TunnelRecord>`, `save_tunnel_record`, `remove_tunnel_record` for `<state_dir>/tunnel.json` (pretty JSON, like the other registries).
  2. Add `is_ssh_process_alive(pid: u32) -> bool` and `terminate_process(pid: u32)` using the commands in the table in design 3.5, spawned with discrete args.
  3. Add `ensure_dashboard_tunnel(state_dir, peer, set)` implementing the four steps in design 3.5.
  4. Add `DashboardCommands::Tunnel { stop: bool, execute: bool, … }`. Without `--execute`: print the decision that would be taken, the ssh command, and the recorded tunnel if any. With `--execute`: call `ensure_dashboard_tunnel`. With `--stop --execute`: terminate the recorded pid if it is a live ssh process and remove `tunnel.json`; if there is no record, print `No recorded tunnel.` and exit 0.
- **Tests** in `tests/cli_integration.rs` (ports 47420, 47421, 47430):
  - `dashboards_tunnel_dry_run_prints_one_ssh_command_with_three_forwards`: stdout has exactly three occurrences of `-L 127.0.0.1:`; no `tunnel.json` is written.
  - `dashboards_tunnel_stop_without_record_is_a_no_op`: `tunnel --stop --execute` exits 0 and prints `No recorded tunnel.`
- **Acceptance:** `cargo test -p cheap-otto --test cli_integration dashboards_tunnel` passes. Do not test starting a real tunnel; that is T6.1.

### T2.3 Rewrite `cheap-otto ui`

- [ ] Done
- **Issue:** #18
- **Covers:** R3.1, R8.1
- **Depends on:** T1.8, T2.1, T2.2
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Replace the body of `run_ui` with the seven steps in design 3.7. Delete the inline `remote_start_script` and the hand-built `ssh -N` block.
  2. Add `--reviewer-port` (default from the registry) and `--open <id>` (default `mission-control`, parsed with `DashboardId::parse`) to `Commands::Ui`. Keep `--port`, `--pixel-port`, `--state-dir`, `--no-open`, `--execute`.
  3. Update the `///` help line of `Ui`: "Ensure every dashboard service on the paired Mac, open one SSH tunnel for all of them, and open Mission Control".
- **Tests** in `tests/cli_integration.rs`:
  - `ui_dry_run_plans_services_then_one_tunnel_and_no_tmux`: stdout mentions all three ports, contains exactly one `ssh -N` line with three `-L`, and does not contain `tmux new-session`.
  - Update `multi_harness_delegate_inspect_ask_and_templates_work_end_to_end` or any other existing test only if it asserted on the old `ui` output.
- **Acceptance:**
  - `cargo test -p cheap-otto --test cli_integration` passes.
  - `grep -n "cheapotto-serve\|cheapotto-pixel" src/main.rs` prints nothing (those names now exist only in `src/dashboards.rs`).

## Phase 3: Mission Control as the hub

### T3.1 `GET /api/dashboards`

- [ ] Done
- **Issue:** #19
- **Covers:** R6.1
- **Depends on:** T0.1, T1.1
- **Files:** `src/server.rs`
- **Steps:**
  1. Add the route to `build_router` and a handler that returns the JSON in design 3.9. Build the set with `DashboardSet::with_ports(state.config.port, state.config.pixel_port, REVIEWER_PORT)`; on error fall back to `DashboardSet::standard()`.
  2. Probe each port with `tokio::time::timeout(Duration::from_millis(250), TcpStream::connect(("127.0.0.1", port)))`.
  3. `current` is true for Mission Control.
  4. Add the route to the module's `//!` header list.
- **Tests** in `src/server.rs`:
  - `api_lists_dashboards_with_listening_state`: `oneshot` a `GET /api/dashboards`; status 200; three entries with the ids in registry order; `current` true only for `mission-control`; every `listening` is a boolean.
- **Acceptance:**
  - `cargo test -p cheap-otto --lib server::tests` passes.
  - Start the `mission-control` preview (`.claude/launch.json`, port 7440) and `curl -s http://127.0.0.1:7440/api/dashboards` returns the three entries with `"port":7440` for Mission Control.

### T3.2 Links between dashboards in Mission Control

- [ ] Done
- **Issue:** #20
- **Covers:** R6.2, R6.4
- **Depends on:** T3.1
- **Files:** `src/ui_assets.rs`
- **Steps:**
  1. Add an empty container in the header of `MISSION_CONTROL_HTML`, for example `<span id="siblingDashboards"></span>`.
  2. On load, `fetch('/api/dashboards')`; for each entry with `listening && !current`, append a link styled like the existing header pills: `href = 'http://' + location.hostname + ':' + d.port + '/'`, `target="_blank"`, `rel="noopener"`, text `escapeHtml(d.name) + ' ↗'`.
  3. If the fetch fails, leave the container empty; do not show an error.
  4. The string `"####` must not appear in the page (rule 07).
- **Acceptance:**
  - Gate passes (the HTML is compiled in).
  - Make a sibling visible locally: start the `reviewer-dashboard` preview (port 7441), then run Mission Control by hand so it treats 7441 as a sibling: `cargo run -p cheap-otto -- serve --port 7440 --pixel-port 7441 --state-dir target/preview-state --workspaces-root target/preview-workspaces`. Load `http://localhost:7440/` and confirm a "Pixel Office ↗" link appears and points at port 7441. The browser console shows no errors.
  - With no sibling listening, the header looks as it did before.

## Phase 4: `code-reviewer` alignment

Everything in this phase stays std-only and synchronous.

### T4.1 `m1 sync` installs both binaries

- [ ] Done
- **Issue:** #21
- **Covers:** R4.1
- **Depends on:** nothing
- **Files:** `crates/code-reviewer/src/m1.rs`
- **Steps:**
  1. Move the `format!` that builds `build_and_provision` out of `sync_everything_to_m1` into `fn render_remote_build_script(remote_dir_expr: &str, brew: &str, cargo: &str, npm: &str) -> String`. Behaviour must stay identical apart from step 2.
  2. Inside the `if [ -d crates/code-reviewer ]; then` branch, add `cargo install --path . --locked --force` on the line after `cargo install --path crates/code-reviewer --locked --force`.
  3. Update the doc comment on `sync_everything_to_m1` (step 2 of its list) to say both binaries are installed.
- **Tests** in `crates/code-reviewer/src/m1.rs`:
  - `remote_build_script_installs_both_binaries_before_provisioning`: the script contains both `cargo install` lines, in that order, and both appear before `m1-provision.sh`.
- **Acceptance:** `cargo test -p code-reviewer m1::tests` passes. Do not run `m1 sync`.

### T4.2 Provisioning installs the dashboard services

- [ ] Done
- **Issue:** #22
- **Covers:** R4.2, R4.3
- **Depends on:** T2.1, T4.1
- **Files:** `crates/code-reviewer/scripts/m1-provision.sh`
- **Steps:**
  1. Add section 8 from design 3.10 immediately before the final `echo "[m1-provision] Provisioning complete."` line.
  2. Keep the file POSIX `sh` with LF line endings.
- **Acceptance:**
  - `sh -n crates/code-reviewer/scripts/m1-provision.sh` exits 0 (Git Bash on Windows works).
  - `git ls-files --eol crates/code-reviewer/scripts/m1-provision.sh` shows `i/lf`.
  - `grep -n "dashboards install-script" crates/code-reviewer/scripts/m1-provision.sh` shows the new lines.

### T4.3 `code-reviewer ui` never opens a second tunnel

- [ ] Done
- **Issue:** #23
- **Covers:** R3.6, R3.9
- **Depends on:** T0.1, T2.3
- **Files:** `crates/code-reviewer/src/server.rs`
- **Steps:**
  1. Add `fn reviewer_fallback_tunnel_args(host: &str, port: u16) -> Vec<String>`: `-N`, `-o ExitOnForwardFailure=yes`, `-o ServerAliveInterval=30`, `-o BatchMode=yes`, `-L 127.0.0.1:<port>:127.0.0.1:<port>`, `<host>`. Only one `-L`.
  2. Add `fn find_cheap_otto_binary() -> Option<PathBuf>`: a file named `cheap-otto` or `cheap-otto.exe` in the directory of `std::env::current_exe()`; otherwise `Some(PathBuf::from("cheap-otto"))` if `cheap-otto --version` runs successfully; otherwise `None`.
  3. Rewrite `open_tunneled_ui` to the five steps in design 3.10. Remove the `DEFAULT_PIXEL_PORT` forward.
  4. If the local port never answers, return `Err` with a message that names the port and suggests `cheap-otto dashboards status --execute`. Print "ready" and open the browser only after the port answers.
- **Tests** in `crates/code-reviewer/src/server.rs` (add a `#[cfg(test)] mod tests` if the file has none):
  - `fallback_tunnel_forwards_only_the_reviewer_port`: exactly one `-L`, it names the given port, and `7421` does not appear.
- **Acceptance:** `cargo test -p code-reviewer server::tests` passes. Do not run `code-reviewer ui`.

### T4.4 The Pixel Office button starts the service, not an orphan

- [ ] Done
- **Issue:** #24
- **Covers:** R1.2 (keeps Pixel Office under `launchd`)
- **Depends on:** T0.1, T1.1
- **Files:** `crates/code-reviewer/src/server.rs`
- **Steps:**
  1. Add `const PIXEL_OFFICE_LAUNCHD_LABEL: &str = "com.cheapotto.dashboards.pixel-office";` with a comment that it must match `src/dashboards.rs` in the root crate.
  2. In `start_pixel_agents_server`, after the "already listening" check and before the direct spawn: on macOS, get the uid from `id -u`, run `launchctl kickstart gui/<uid>/<label>`, and if it exits 0 go straight to the existing wait loop. On other systems, or if it fails, fall through to today's spawn.
- **Acceptance:** Gate passes. `grep -n "PIXEL_OFFICE_LAUNCHD_LABEL" crates/code-reviewer/src/server.rs` shows the constant and its use.

### T4.5 `open-ui.ps1` becomes a wrapper

- [ ] Done
- **Issue:** #25
- **Covers:** R3.9
- **Depends on:** T4.3
- **Files:** `crates/code-reviewer/scripts/open-ui.ps1`
- **Steps:**
  1. Keep the `param` block and `$ErrorActionPreference = "Stop"`.
  2. Replace the body: build an argument array `run --quiet -- ui --host $HostAddr --port $Port` (add `--no-open` when `-NoOpen`), run `& cargo @argsList` from the crate directory inside `Push-Location`/`Pop-Location`, and `throw` when `$LASTEXITCODE` is not 0.
  3. Remove the `Start-Process ssh` block and the health polling.
- **Acceptance:** `grep -n "Start-Process" crates/code-reviewer/scripts/open-ui.ps1` prints nothing. Do not run the script.

### T4.6 Shared ports, and a link back to Mission Control

- [ ] Done
- **Issue:** #26
- **Covers:** R2.5, R6.3
- **Depends on:** T0.1, T1.1
- **Files:** `crates/code-reviewer/src/server.rs`, `crates/code-reviewer/src/ui_html.rs`, `tests/dashboard_ports.rs` (new, root crate)
- **Steps:**
  1. Add `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = 7420;` beside `DEFAULT_UI_PORT`, with a comment naming `src/dashboards.rs` as the source of truth.
  2. In `DASHBOARD_HTML`, add a link to Mission Control in the top bar: `href = 'http://' + location.hostname + ':7420/'`, `target="_blank"`, `rel="noopener"`, label `MISSION CONTROL ↗`, styled like the existing top-bar buttons. The string `"##` must not appear in the page.
  3. Create `tests/dashboard_ports.rs` with `#![forbid(unsafe_code)]`: `include_str!("../crates/code-reviewer/src/server.rs")` and assert it contains `pub const DEFAULT_UI_PORT: u16 = {REVIEWER_PORT};`, `pub const DEFAULT_PIXEL_PORT: u16 = {PIXEL_OFFICE_PORT};` and `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = {MISSION_CONTROL_PORT};`, formatting the values from `cheap_otto::dashboards`.
- **Tests:** `reviewer_ports_match_the_registry` in `tests/dashboard_ports.rs`.
- **Acceptance:**
  - `cargo test -p cheap-otto --test dashboard_ports` passes.
  - Start the `reviewer-dashboard` preview (port 7441), load it, and confirm the link is in the top bar. Send no `POST` to that server.

## Phase 5: documentation

### T5.1 User documentation

- [ ] Done
- **Issue:** #27
- **Covers:** R9.1
- **Depends on:** T2.3, T4.3
- **Files:** `README.md`, `docs/index.html`, `crates/code-reviewer/README.md`, `crates/code-reviewer/.claude/skills/update-m1/SKILL.md`, `crates/code-reviewer/.agents/skills/update-m1/SKILL.md`
- **Steps:**
  1. README section 4 (Mission Control): describe `cheap-otto ui --execute` as "ensures the three dashboard services on the Mac, opens one tunnel, opens Mission Control", and add `cheap-otto dashboards status --execute`, `cheap-otto dashboards tunnel --stop --execute` and `cheap-otto dashboards uninstall --execute`. Remove wording about starting daemons in tmux.
  2. README: add a short note that `ssh otto-mac` no longer forwards dev ports and that `cheap-otto pair --tunnel` does.
  3. `docs/index.html`: in the `QS` array, update the "Watch" tab to the same commands. Every command must run as written (rule 08).
  4. `crates/code-reviewer/README.md` quickstart step 4: say that `code-reviewer ui` uses the shared tunnel.
  5. Both `update-m1` skill files: note that `m1 sync` installs both binaries and the dashboard services. The two files must stay byte-identical (`diff -r crates/code-reviewer/.claude/skills crates/code-reviewer/.agents/skills` prints nothing).
- **Acceptance:**
  - `grep -n "tmux" README.md` shows no line about starting Mission Control or pixel-agents in tmux.
  - The `diff -r` above prints nothing.
  - The site script still parses: load `docs/index.html` in a browser and confirm the Install tabs work and the console is clean.

### T5.2 Agent documentation

- [ ] Done
- **Issue:** #28
- **Covers:** R9.1
- **Depends on:** T2.3, T4.2
- **Files:** `CLAUDE.md`, `.claude/rules/05-crate-ecosystem-and-cli-patterns.md`, `.claude/rules/06-code-reviewer.md`, `.claude/rules/07-embedded-web-ui.md`, `specs/README.md`, `specs/m1-dashboards/README.md`
- **Steps:**
  1. `CLAUDE.md`: in the naming contract add `com.cheapotto.dashboards.<id>` and `~/.cheap-otto/logs/dashboards/<id>.log`; add `tunnel.json` to the local state list; add `dashboards.rs` where the architecture names the registry; add `dashboards install|uninstall|restart|tunnel` to the commands that need `--execute`.
  2. Rule 05: add `dashboards` to the dry-run examples and note the reserved project name.
  3. Rule 06: update the `m1 sync` description (both binaries, dashboard services) and the launchd label list.
  4. Rule 07: replace the "how it runs" notes with the service model; the preview table is unchanged.
  5. Mark this spec's status in `specs/README.md` and `specs/m1-dashboards/README.md`.
- **Acceptance:** Every path and identifier the edited files mention exists in the repo. `grep -rn "cheapotto-serve" CLAUDE.md .claude/rules` prints nothing.

## Phase 6: LIVE rollout (owner only)

These tasks act on the real MacBook. An agent may prepare the commands and read the output with the owner, but must not run them without being told to.

### T6.1 Deploy and verify on the real Mac

- [ ] Done
- **Issue:** #29
- **Covers:** R1.1 to R1.9, R3.1 to R3.9, R4.1 to R4.3, R5.1 to R5.3, R8.1
- **Depends on:** every task in Phases 0 to 5
- **LIVE**
- **Steps and expected results** (run from `main` after everything is merged):
  1. `cargo run -p code-reviewer -- m1 sync` → ends with `Provisioning complete`, and the output includes `Dashboard services are installed`. The first run compiles `cheap-otto` on the Mac and takes a few minutes.
  2. `cargo run -p cheap-otto -- dashboards status --execute` → three rows, each `loaded=yes listening=yes`. If Pixel Office says skipped, install `pixel-agents` on the Mac and run step 1 again.
  3. `cargo run -p cheap-otto -- ui --execute` → the browser opens Mission Control; `http://127.0.0.1:7420`, `:7421` and `:7430` all load.
  4. One tunnel: in PowerShell, `Get-CimInstance Win32_Process -Filter "name='ssh.exe'" | Select-Object ProcessId, CommandLine` shows exactly one process with three `-L` forwards. Close any older tunnels first.
  5. `cargo run -p code-reviewer -- ui` → the reviewer opens and step 4 still shows one process.
  6. `ssh otto-mac true` prints no "Address already in use" or "cannot listen" warning.
  7. Idempotence: run `cargo run -p cheap-otto -- dashboards install --execute` twice; the second run prints `action=unchanged` for each installed service and the pids in `status` do not change.
  8. KeepAlive: `cargo run -p code-reviewer -- m1 exec --cmd "pkill -f 'cheap-otto serve'"`, wait 15 seconds, then `dashboards status --execute` shows Mission Control `listening=yes` with a new pid.
  9. Links: Mission Control shows links to Pixel Office and Reviewer; the reviewer shows a link to Mission Control; each opens the right page.
  10. Stop and start: `dashboards tunnel --stop --execute` makes the three URLs unreachable; `ui --execute --no-open` brings them back.
- **Acceptance:** all ten results observed. Paste the output of steps 2, 4 and 7 into the issue (remove host names and IPs first).
- **Rollback:** `cargo run -p cheap-otto -- dashboards uninstall --execute`.

### T6.2 Reboot behaviour

- [ ] Done
- **Issue:** #30
- **Covers:** R1.3, R5.3
- **Depends on:** T6.1
- **LIVE**
- **Steps:**
  1. Run `dashboards status --execute` and note whether it warns about automatic login.
  2. Reboot the Mac. Before logging in, `dashboards status --execute` is expected to fail or show services not loaded; after logging in (or with automatic login on), all three are `listening=yes` within a minute, with no command run.
  3. Decide design question Q1: leave automatic login off and accept a manual login after power loss, or turn it on in System Settings by hand. Record the decision in `design.md` section 9.
- **Acceptance:** the observed behaviour and the decision are written in the issue and in `design.md`.
