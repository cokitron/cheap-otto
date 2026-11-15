# Tasks: dashboards served from the M1

Tracking issue: #6. Each task below has its own issue.

Read `../README.md` (how to carry out a task), then `requirements.md` and `design.md`. Section numbers below, such as "design 3.3", refer to `design.md`.

**Rules for every task**

- One row of the PR plan below, one branch, one PR, inside the PR size budget in `../README.md` (at most 600 changed lines (tests included, `Cargo.lock` excluded) and one theme). Use the branch name the plan gives. `<type>` is a prefix CI accepts (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`, `ci`, `perf`); a branch starting with `m1-dashboards/` fails the PR governance check.
- The gate must pass before you open the PR:
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-targets --locked
  ```
- Use the exact test names given. Add more tests if you need them; do not rename or drop the listed ones.
- Outside a **LIVE** task, never pass `--execute` or `--apply` against the real MacBook or real GitHub repos, and never run live `code-reviewer m1 …`, `poll`, `tend-poll`, `review`, `tend`, `sync-docs` or `ui` (`sync-docs` has no dry run). A test may pass `--execute` only with an isolated temp `--state-dir` and no live endpoint, as T2.2's `tunnel --stop --execute` test does. Only tasks marked **LIVE** touch the real MacBook, and only the owner runs them.
- No CORS headers, no bind address other than `127.0.0.1`, no new dependencies, no `.unwrap()` or `.expect()` outside tests.
- When done: tick the box of every task (or part) in the PR and add `PR: #NN` under each. The PR closes each task's issue, except for a part of a split task that is not the last part: that PR says `part of #NN`, and the last part closes the issue.

## PR plan

The 21 open tasks before the LIVE rollout ship as 15 PRs. Small tasks on one theme share a PR; T4.3 is split into two parts (PRs 4 and 14) because its second half needs the new `ui`. T1.5 moved from PR 7 into PR 9 so that PRs 6, 10 and 12 can run in parallel. PR 7 follows PR 6, because T1.4 and T1.6 both edit `src/dashboards.rs` and `tests/dashboard_scripts.rs`. Take the first row whose tasks are all unchecked in this file, whose issues are all open and unclaimed, and whose "needs" are merged. A row that runs past 600 lines is split at a green seam (`../README.md`, PR size budget).

| PR | Tasks | Branch | Needs |
| --- | --- | --- | --- |
| 1 | T1.1 registry (merged in #41) | `feat/m1-dashboards-t1-1-registry` | nothing |
| 2 | T1.8 SSH config without forwards (merged in #42) | `fix/m1-dashboards-t1-8-ssh-config-forwards` | nothing |
| 3 | T4.1 testable two-binary deploy (merged in #43) | `refactor/m1-dashboards-t4-1-build-script` | nothing |
| 4 | T4.3 part a: reviewer fallback tunnel (merged in #44) | `fix/m1-dashboards-t4-3a-fallback-tunnel` | nothing |
| 5 | T1.2 reserved name, T1.3 LaunchAgent templates (merged in #45→#41) | `feat/m1-dashboards-t1-2-t1-3-plists` | PR 1 |
| 6 | T1.4 install script and plan (merged in #51) | `feat/m1-dashboards-t1-4-install-script` | PR 5 |
| 7 | T1.6 status script, parser and diagnosis (merged in #52) | `feat/m1-dashboards-t1-6-status` | PR 6 |
| 8 | T1.7 tunnel spec, decision and record (merged in #47) | `feat/m1-dashboards-t1-7-tunnel` | PR 1 |
| 9 | T1.5 uninstall and restart, T2.1 `dashboards` subcommands | `feat/m1-dashboards-t1-5-t2-1-subcommands` | PRs 6, 7 |
| 10 | T2.2 tunnel management (merged in #50) | `feat/m1-dashboards-t2-2-tunnel` | PR 8 |
| 11 | T3.1 hub API, T3.2 Mission Control links, T4.6 shared ports and link back (merged in #48→#41) | `feat/m1-dashboards-t3-links` | PR 1 |
| 12 | T4.4 Pixel Office button (merged in #49) | `feat/m1-dashboards-t4-4-pixel-button` | PR 1 |
| 13 | T2.3 new `ui` | `feat/m1-dashboards-t2-3-ui` | PRs 2, 9, 10 |
| 14 | T4.2 provisioning, T4.3 part b, T4.5 `open-ui.ps1` | `feat/m1-dashboards-t4-reviewer-alignment` | PRs 3, 4, 9, 13 |
| 15 | T5.1 user docs, T5.2 agent docs | `docs/m1-dashboards-t5-docs` | PRs 13, 14 |

After PR 15 the owner runs T6.1 and T6.2 on the real Mac; they produce no PR unless they find a bug.

## Phase 0: prerequisite

### T0.1 Origin and host guard on both dashboard servers

- [x] Done
- **PR:** #31, landed on `main` through #4
- **Issue:** #7
- **Covers:** R7.1
- **Depends on:** nothing
- **Status:** done. #31 was merged into PR #4's branch and #4 is merged into `main`.
- **Goal:** `cheap-otto serve` and `code-reviewer serve` reject state-changing requests from a foreign `Origin` and any request with a non-loopback `Host`; `CorsLayer::permissive()` is gone.
- **Acceptance:**
  - #4, which carries #31, is merged into `main`.
  - `grep -n "CorsLayer::permissive" src/server.rs` prints nothing.
- **Why it came first:** T3.1, T4.3, T4.4 and T4.6 edit the same two `server.rs` files. They can start now.

## Phase 1: core planners (root crate, pure code)

All of Phase 1 lives in the functional core. No task here spawns a process, reads a file or opens a socket outside tests.

### T1.1 Dashboard registry

- [x] Done
- PR: #41
- **Issue:** #8
- **Covers:** R2.1, R2.2, R2.3
- **Depends on:** nothing
- **Files:** `src/dashboards.rs` (new), `src/lib.rs`
- **Steps:**
  1. Create `src/dashboards.rs` with a `//!` module header and the constants, `DashboardError`, `DashboardId`, `ServiceManager`, `DashboardService` and `DashboardSet` exactly as in design 3.1.
  2. Implement `DashboardId::all`, `slug`, `display_name`, `parse`, and `DashboardSet::standard`, `with_ports`, `get`, `ports`, `managed`. `standard()` builds from the three constants through the private constructor that `with_ports` uses after validating, so it needs no `.expect()`; a test proves the constants pass `with_ports`.
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

- [x] Done
- PR: #45
- **Issue:** #9
- **Covers:** R2.4
- **Depends on:** T1.1
- **Files:** `src/schedule.rs`
- **Steps:**
  1. Add the variant `ReservedProjectName(String)` to `ScheduleError` with the message `project name '{0}' is reserved for dashboard services`.
  2. In `build_schedule_install_plan`, after the project name is validated, return that error when it equals `crate::dashboards::RESERVED_PROJECT_NAME`, ignoring ASCII case (APFS file names are case-insensitive).
- **Tests** in `src/schedule.rs`:
  - `rejects_schedule_for_reserved_dashboards_project`
- **Acceptance:** `cargo test -p cheap-otto --lib schedule::tests` passes and includes the new test.

### T1.3 LaunchAgent templates

- [x] Done
- PR: #45
- **Issue:** #10
- **Covers:** R1.1, R1.2, R1.3, R1.4, R1.8
- **Depends on:** T1.1
- **Files:** `src/dashboards.rs`
- **Steps:**
  1. Add `pub fn render_service_plist_template(service: &DashboardService, set: &DashboardSet) -> Option<String>` with `#[must_use]`.
  2. Return the XML in design 3.2 for Mission Control and the Pixel Office variant; return `None` for the reviewer.
  3. Take ports from `set`. Keep `$HOME_XML` and `$PIXEL_BIN_XML` as literal placeholders; the install script fills them with XML-escaped values (T1.4), so the templates never contain a raw `$HOME` or `$PIXEL_BIN`.
- **Tests** in `src/dashboards.rs`:
  - `mission_control_plist_keeps_alive_and_binds_loopback`: contains the label, `<key>KeepAlive</key>` followed by `<true/>`, `<key>RunAtLoad</key>`, `<string>127.0.0.1</string>`, the log path, and no `0.0.0.0`.
  - `pixel_office_plist_uses_resolved_binary_placeholder`: contains `<string>$PIXEL_BIN_XML</string>` and `--no-terminal`.
  - `plist_templates_use_escaped_placeholders_not_raw_home`: the rendered Mission Control and Pixel Office templates contain no raw `$HOME` or `$PIXEL_BIN` (only the `$HOME_XML` and `$PIXEL_BIN_XML` forms).
  - `reviewer_has_no_template_because_provisioner_manages_it`
  - `plist_ports_follow_the_set`: with `with_ports(8420, 8421, 8430)` the Mission Control template contains `8420` and `8421` and not `7420`.
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.4 Install script and install plan

- [x] Done
- **PR:** #51
- **Issue:** #11
- **Covers:** R1.5, R1.6, R1.7, R1.9, R1.10, R4.4
- **Depends on:** T1.3
- **Files:** `src/dashboards.rs`, `tests/dashboard_scripts.rs` (new)
- **Steps:**
  1. Add `PIXEL_AGENTS_CONFIG_SNIPPET` as in design 3.3.
  2. Add `pub fn render_install_script(set: &DashboardSet) -> String` producing the script in design 3.3, in that order: header, GUI-session guard, legacy tmux cleanup, `install_service`, `wait_port`, Mission Control block, Pixel Office block. Labels and ports come from `set`. The header sets up `xml_escape` and the `HOME_XML` and `PIXEL_BIN_XML` values used to fill the plist placeholders, and `FAILED=0`. Before bootstrap, a port still held is freed by listener, waiting 5 s and then 2 s more for the port to be released, with `kill -9` as the last resort. If the port stays held, `install_service` prints the `cause=port-held` line with `last_exit` and `log`, sets `FAILED=1` and returns 1; each call site is `if install_service …; then wait_port …; fi`, so `wait_port` is skipped for that service. Because `set -e` is off inside an `if` condition, `install_service` checks `cat`, `mv` and `launchctl bootstrap` explicitly and reports `cause=plist-write-failed` or `cause=bootstrap-failed` the same way. The script ends with `exit 4` when `FAILED` is 1.
  3. Add `DashboardPlan` and `pub fn build_install_plan(peer: &PeerNode, set: &DashboardSet) -> DashboardPlan`. The spec is `CommandSpec::new("Install dashboard services on MacBook", "ssh", ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-p", <port>, <user@host>, <remote>])` with `remote = format!("/bin/sh -c {}", posix_single_quote(&script))`.
  4. `display_steps` has one line per managed service: `Ensure LaunchAgent <label> -> 127.0.0.1:<port>`.
  5. Create `tests/dashboard_scripts.rs` with `#![forbid(unsafe_code)]`. Under `#[cfg(unix)]`, write the rendered script to a temp file and assert `sh -n <file>` exits 0.
- **Tests:**
  - in `src/dashboards.rs`:
    - `install_script_is_posix_and_guards_gui_session`: starts with `set -eu`; contains `error=no-gui-session` and `exit 3`; contains no `[[`.
    - `install_script_skips_unchanged_services_and_reports_actions`: contains `cmp -s`, `action=unchanged`, `action=installed`.
    - `install_script_skips_pixel_office_when_binary_missing`: contains `reason=pixel-agents-not-installed`.
    - `install_script_frees_ports_by_listener_not_by_name`: contains `lsof -nP -iTCP:` and no `pkill`.
    - `install_script_never_binds_non_loopback`: no `0.0.0.0`.
    - `install_script_waits_for_port_release_before_bootstrap`: contains `kill -9` and `cause=port-held`, and the `lsof` wait loop appears before `launchctl bootstrap`.
    - `install_script_reports_port_held_with_diagnostics_and_skips_wait`: the port-held line contains `listening=0`, `cause=port-held`, `last_exit=` and `log=$LOG_DIR/`; that branch ends in `return 1`; and each `install_service` call is the condition of an `if` whose body is the `wait_port` call.
    - `install_script_checks_each_step_because_errexit_is_off_in_conditions`: `install_service` contains `if ! cat > "$tmp"`, `if ! mv "$tmp" "$plist"` and `if ! launchctl bootstrap`, each followed by a `cause=` line, `FAILED=1` and `return 1`, and `action=installed` appears only after the bootstrap check.
    - `install_script_exits_four_when_a_service_never_listens`: contains `FAILED=1` and `exit 4`.
    - `install_script_reports_cause_log_and_last_exit_when_a_service_never_listens`: the failure line contains `listening=0`, `cause=not-listening-after-10s`, `last_exit=` and `log=$LOG_DIR/`, and both `wait_port` calls pass the service label.
    - `install_script_escapes_xml_values_for_plists`: contains `sed -e 's/&/\&amp;/g'`.
    - `install_plan_wraps_script_for_remote_sh`: program is `ssh`; the last arg starts with `/bin/sh -c '`.
  - in `tests/dashboard_scripts.rs`: `rendered_install_script_passes_sh_syntax_check`
- **Acceptance:**
  - `cargo test -p cheap-otto --lib dashboards::tests` passes.
  - On Linux or macOS, `cargo test -p cheap-otto --test dashboard_scripts` passes. On Windows it compiles and runs 0 tests.

### T1.5 Uninstall and restart plans

- [x] Done
- PR: #54
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

- [x] Done
- PR: #52
- **Issue:** #13
- **Covers:** R5.1, R5.2, R5.5
- **Depends on:** T1.1, T1.4 (same files as T1.4, so it follows PR 6)
- **Files:** `src/dashboards.rs`, `tests/dashboard_scripts.rs`
- **Steps:**
  1. Add `render_status_script(set)` and `build_status_spec(peer, set) -> CommandSpec`, printing the lines in design 3.4, including `last_exit` on every dashboard line. Use `BatchMode=yes` and `ConnectTimeout=3` like `PeerNode::build_ssh_probe_spec`.
  2. Add `DashboardRemoteStatus`, `DashboardHostFacts`, `DashboardStatusReport` and `pub fn parse_status_output(raw: &str, set: &DashboardSet) -> DashboardStatusReport`.
  3. The parser never panics: unknown lines, keys and ids are ignored; `pid=-` and unparsable pids become `None`; `autologin_user=-` and `last_exit=-` become `None`.
  4. Add `DashboardDiagnosis`, `explain_unavailable` and `render_status_rows` as in design 3.4. `explain_unavailable` returns `None` for a loaded and listening dashboard and otherwise the first matching row of the table there, always with the log path.
- **Tests:**
  - `status_script_is_read_only`: contains none of `bootstrap`, `bootout`, `kickstart`, `kill `, `rm `, `mv `.
  - `parses_status_lines_for_all_dashboards`: the four sample lines in design 3.4 give three dashboards and the host facts.
  - `ignores_unknown_lines_keys_and_ids`
  - `missing_host_line_yields_none`
  - `handles_empty_and_garbage_output_without_panicking`: `""`, `"\n\n"`, `"dashboard="`, `"dashboard=reviewer pid=abc loaded=maybe"`.
  - `explains_every_unavailable_dashboard_with_reason_and_log`: one case per row of the table in design 3.4 (no GUI session; `cheap-otto` missing; `pixel-agents` missing; row 4a, not loaded but `listening` true, whose reason says another process holds the port and names the `lsof` command [R5.1, R1.10]; not loaded and not listening; loaded but not listening with a `last_exit`), each with the expected reason and log path; a missing `host` line skips the first three.
  - `listening_dashboard_needs_no_explanation`
  - `status_rows_explain_port_held_skipped_and_unloaded_dashboards`: feed `parse_status_output` the probe output a Mac would print after a port-held Mission Control install and a skipped Pixel Office, pass the report to `render_status_rows`, and assert each of those two rows carries a reason and a log path while the listening Reviewer row has neither. This is the end-to-end check for R5.1; it needs no Mac because the probe output is the input.
  - in `tests/dashboard_scripts.rs`: `rendered_status_script_passes_sh_syntax_check`
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.7 Tunnel spec, decision and record

- [x] Done
- PR: #47
- **Issue:** #14
- **Covers:** R3.2, R3.3, R3.4, R3.5, R3.6
- **Depends on:** T1.1
- **Files:** `src/dashboards.rs`
- **Steps:**
  1. Add `build_tunnel_spec`, `TunnelRecord` (with `ssh_port`), `TunnelDecision`, `recorded_tunnel_covers(record: Option<&TunnelRecord>, ssh_alive: bool, set: &DashboardSet, peer: &PeerNode) -> bool`, `decide_tunnel(local_ports, recorded_tunnel_covers)`, `TunnelFailure`, `describe_tunnel_failure` and `command_line_is_recorded_tunnel` as in design 3.5.
  2. Argument order: `-N`, the five `-o` options (the four below plus `-o ConnectTimeout=10`), `-p <ssh_port>`, one `-L 127.0.0.1:P:127.0.0.1:P` per port in set order, then `user@host` last.
  3. `decide_tunnel`: all true and `recorded_tunnel_covers` → `AlreadyUp`; all true without a covering record → `Conflict` with every port in `held` and an empty `missing`; all false or an empty slice → `Start`; otherwise `Conflict` with `held` (true) and `missing` (false) in input order.
  4. `recorded_tunnel_covers`: true only when the record exists, `ssh_alive` is true, the record's `destination` equals `peer.ssh_destination()`, its `ssh_port` equals `peer.ssh_port`, and its ports include every port in the set.
  5. `describe_tunnel_failure`: one message for both failure kinds. It always lists the unanswered ports and ends with the SSH test command.
  6. `command_line_is_recorded_tunnel`: true only when the command line contains `ssh`, the record's `destination`, and `127.0.0.1:<port>:127.0.0.1:<port>` for every recorded port.
- **Tests:**
  - `tunnel_spec_forwards_every_port_on_loopback_with_safe_options`: three `-L` args, each starting and continuing with `127.0.0.1:`; contains `ExitOnForwardFailure=yes`, `ServerAliveInterval=30`, `ServerAliveCountMax=3`, `BatchMode=yes`, `ConnectTimeout=10`; the last arg is `user@host`.
  - `decides_already_up_start_and_conflict`
  - `all_ports_answering_without_a_recorded_tunnel_is_a_conflict`: three answering ports with `recorded_tunnel_covers = false` give `Conflict` holding all three ports, so no tunnel is started.
  - `recorded_tunnel_must_be_alive_ssh_and_cover_every_port`: no record, a dead or non-ssh pid, and a record missing one port are all false; a live ssh record covering all ports is true.
  - `recorded_tunnel_for_another_peer_or_ssh_port_does_not_cover`: a live ssh record that covers every port but has another `destination`, or the same destination with another `ssh_port`, is false.
  - `tunnel_failure_message_names_unanswered_ports_for_exit_and_timeout`: for both `ExitedEarly` and `TimedOut`, the message contains every unanswered port and the SSH test command.
  - `command_line_must_match_recorded_destination_and_forwards`: the recorded tunnel's own command line is true; an unrelated `ssh user@other-host`, the right host with one forward missing, an empty string and a non-ssh process are all false.
  - `tunnel_record_round_trips_as_json`
- **Acceptance:** `cargo test -p cheap-otto --lib dashboards::tests` passes.

### T1.8 Stop writing `LocalForward` into the SSH config block

- [x] Done
- **PR:** #42
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

- [x] Done
- PR: #54
- **Issue:** #16
- **Covers:** R1.1, R4.4, R5.1 to R5.5, R8.2, R8.4
- **Depends on:** T1.4, T1.5, T1.6
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Add `Commands::Dashboards { #[command(subcommand)] action: DashboardCommands }` and the enum with `List`, `Install`, `InstallScript`, `Status`, `Restart`, `Uninstall` (design 3.6). Every variant and flag gets a `///` help line. Shared flags: `--state-dir`, `--port`, `--pixel-port` (defaults from the registry constants; the Reviewer port is the `REVIEWER_PORT` constant, not a flag); `--execute` with `visible_alias = "apply"` on install, status, restart, uninstall; `--json` on list and status; `--id` on restart.
  2. One handler per subcommand taking one options struct, following the pattern of `run_delegate`; print the human-readable plan first unless `--json` is selected, then emit only JSON. Check `execute` only after the plan is printed.
  3. `install-script` prints `render_install_script` to stdout and nothing else (no banner), so it can be piped to `sh`.
  4. `status` without `--execute`: print the probe command only. A dry run opens no socket, so it must not call `is_local_port_open` (design 3.6). With `--execute`: check which ports answer locally, run the probe with `capture_spec_stdout`, parse it, and print the rows from `render_status_rows`: `loaded`, `pid`, `listening`, `tunnelled`, and for every dashboard that is not listening its reason and log path (design 3.4). Print the warning for R5.3 when `autologin_user` is `None`.
  5. `status --json` prints one JSON object (`{"dashboards":[…],"host":…}`) and nothing else on stdout: no plan, banner or warning lines. Machine-readable mode skips the human-readable plan; any diagnostics go to stderr. With `--execute`, each dashboard object also carries `reason` and `log`, both `null` when it is listening. Without `--execute`, `--json` prints the three registry entries with `loaded`, `pid`, `listening`, `tunnelled`, `reason` and `log` all `null`, and `host` `null`.
- **Tests** in `tests/cli_integration.rs` (temp dir; create a peer first with `pair --host 192.168.1.42 --out-dir <tmp>`; dry runs open no socket, so no port override is needed and the fixed Reviewer port is never touched):
  - `dashboards_list_and_install_script_are_pure`: `list --json` parses and has three entries; `install-script` stdout starts with `set -eu` and contains both managed labels.
  - `dashboards_install_status_restart_uninstall_are_dry_runs_without_execute`: each exits 0, prints an `ssh` command, and the temp dir gains no `tunnel.json`.
  - `dashboards_status_json_stdout_is_json_only`: `status --json` (no `--execute`) exits 0 and the whole of stdout parses as one JSON object with a `dashboards` array of three entries.
  - `dashboards_restart_rejects_unknown_id`: `restart --id nope` exits non-zero and stderr names the three valid ids.
  - `dashboards_dry_runs_open_no_socket`: bind a `TcpListener` on `127.0.0.1:0`, set it non-blocking, pass its port as `--port`, run `status`, `install` and `uninstall` without `--execute`, then assert `accept()` returns `WouldBlock`: nothing connected.
- **Acceptance:**
  - `cargo test -p cheap-otto --test cli_integration dashboards_` passes.
  - `cargo run -q -p cheap-otto -- dashboards install-script | head -1` prints `set -eu`.
  - `cargo run -q -p cheap-otto -- dashboards --help` lists the six subcommands.

### T2.2 Tunnel management

- [x] Done
- PR: #50
- **Issue:** #17
- **Covers:** R3.1 to R3.7
- **Depends on:** T1.7
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Add `load_tunnel_record(state_dir) -> Option<TunnelRecord>`, `save_tunnel_record`, `remove_tunnel_record` for `<state_dir>/tunnel.json` (pretty JSON, like the other registries).
  2. Add `process_command_line(pid: u32) -> Option<String>`, `recorded_tunnel_is_running(record: &TunnelRecord) -> bool` (the command line passed to `command_line_is_recorded_tunnel`) and `terminate_process(pid: u32)`, using the commands in the table in design 3.5, spawned with discrete args. Never terminate a recorded pid that `recorded_tunnel_is_running` rejects: remove the stale `tunnel.json` instead.
  3. Add `ensure_dashboard_tunnel(state_dir, peer, set)` implementing the four steps in design 3.5. It computes `recorded_tunnel_covers` from `tunnel.json` and `recorded_tunnel_is_running`; when every port answers but no verified recorded tunnel covers them, it starts nothing and returns an error naming the ports and the holder-finding command (R3.2). A live recorded tunnel to a different destination or SSH port is stopped and replaced (R3.5). When the child exits early or the poll times out (terminate the child first in that case), remove `tunnel.json` and return the message from `describe_tunnel_failure`, which names the unanswered ports in both cases (R3.6). `save_tunnel_record` stores `peer.ssh_destination()` and `peer.ssh_port`.
  4. Add `DashboardCommands::Tunnel { stop: bool, execute: bool, … }`. Without `--execute`: print the ssh command and the recorded tunnel if any (read from `tunnel.json`); do not probe ports. With `--execute`: call `ensure_dashboard_tunnel`. With `--stop --execute`: terminate the recorded pid only if `recorded_tunnel_is_running` accepts it, and remove `tunnel.json` either way; if there is no record, print `No recorded tunnel.` and exit 0.
- **Tests** in `tests/cli_integration.rs` (dry runs open no socket):
  - `dashboards_tunnel_dry_run_prints_one_ssh_command_with_three_forwards`: stdout has exactly three occurrences of `-L 127.0.0.1:`; no `tunnel.json` is written.
  - `dashboards_tunnel_stop_without_record_is_a_no_op`: `tunnel --stop --execute` exits 0 and prints `No recorded tunnel.`
- **Acceptance:** `cargo test -p cheap-otto --test cli_integration dashboards_tunnel` passes. Do not test starting a real tunnel; that is T6.1.

### T2.3 Rewrite `cheap-otto ui`

- [x] Done
- PR: #56
- **Issue:** #18
- **Covers:** R3.1, R3.10, R8.1
- **Depends on:** T1.8, T2.1, T2.2
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Replace the body of `run_ui` with the eight steps in design 3.7. If the install step fails, stop with the install error before touching the ssh config, tunnel or browser. After the tunnel is up, run the status probe and print a warning for each dashboard not listening. Delete the inline `remote_start_script` and the hand-built `ssh -N` block.
  2. Add `--open <id>` (default `mission-control`, parsed with `DashboardId::parse`) to `Commands::Ui`. Keep `--port`, `--pixel-port`, `--state-dir`, `--no-open`, `--execute`.
  3. Update the `///` help line of `Ui`: "Ensure every dashboard service on the paired Mac, open one SSH tunnel for all of them, and open Mission Control".
- **Tests** in `tests/cli_integration.rs`:
  - `ui_dry_run_plans_services_then_one_tunnel_and_no_tmux`: stdout mentions all three ports, contains exactly one `ssh -N` line with three `-L`, and does not contain `tmux new-session`.
  - `ui_dry_run_opens_no_socket`: with listeners bound on the `--port` and `--pixel-port` values (and the Reviewer port when free), `ui` without `--execute` exits 0 and no listener accepts a connection.
  - Update `multi_harness_delegate_inspect_ask_and_templates_work_end_to_end` or any other existing test only if it asserted on the old `ui` output.
- **Acceptance:**
  - `cargo test -p cheap-otto --test cli_integration` passes.
  - `grep -n "cheapotto-serve\|cheapotto-pixel" src/main.rs` prints nothing (those names now exist only in `src/dashboards.rs`).

## Phase 3: Mission Control as the hub

### T3.1 `GET /api/dashboards`

- [x] Done
- PR: #48
- **Issue:** #19
- **Covers:** R6.1
- **Depends on:** T0.1, T1.1
- **Files:** `src/server.rs`
- **Steps:**
  1. Add the route to `build_router` and a handler that returns the JSON in design 3.9. The `serve` startup in `src/server.rs` calls `DashboardSet::with_ports(config.port, config.pixel_port, REVIEWER_PORT)` once the listener has resolved `--port 0`, and returns its error before it creates any directory or serves a request, so an invalid port configuration never starts a server. Keep the resulting set in the app state; the handler returns HTTP 500 with `{"error":"<message>"}` on `Err` and never falls back to `standard()`.
  2. Probe each port with `tokio::time::timeout(Duration::from_millis(250), TcpStream::connect(("127.0.0.1", port)))`.
  3. `current` is true for Mission Control.
  4. Add the route to the module's `//!` header list.
- **Tests** in `src/server.rs`:
  - `api_lists_dashboards_with_listening_state`: `oneshot` a `GET /api/dashboards` **with a `Host: 127.0.0.1:7420` header** (the request guard from T0.1 answers 403 without a loopback `Host`); status 200; three entries with the ids in registry order; `current` true only for `mission-control`; every `listening` is a boolean.
  - `api_dashboards_reports_invalid_port_configuration`: a state whose `pixel_port` equals `port`; `oneshot` a `GET /api/dashboards` with the `Host: 127.0.0.1:7420` header; status 500 and the JSON body has an `error` string.
- **Acceptance:**
  - `cargo test -p cheap-otto --lib server::tests` passes.
  - Start the `mission-control` preview (`.claude/launch.json`, port 7440) and `curl -s http://127.0.0.1:7440/api/dashboards` returns the three entries with `"port":7440` for Mission Control.

### T3.2 Links between dashboards in Mission Control

- [x] Done
- PR: #48
- **Issue:** #20
- **Covers:** R6.2, R6.4
- **Depends on:** T3.1
- **Files:** `src/ui_assets.rs`
- **Steps:**
  1. Add an empty container in the header of `MISSION_CONTROL_HTML`, for example `<span id="siblingDashboards"></span>`.
  2. On load, `fetch('/api/dashboards')`; for each entry with `listening && !current`, append a link styled like the existing header pills: `href = 'http://' + location.hostname + ':' + d.port + '/'`, `target="_blank"`, `rel="noopener"`, text `escapeHtml(d.name) + ' ↗'`.
  3. If the fetch fails, leave the container empty; do not show an error.
  4. The string `"####` must not appear in the page (rule 08).
- **Acceptance:**
  - Gate passes (the HTML is compiled in).
  - Make a sibling visible locally: start the `reviewer-dashboard` preview (port 7441), then run Mission Control by hand so it treats 7441 as a sibling: `cargo run -p cheap-otto -- serve --port 7440 --pixel-port 7441 --state-dir target/preview-state --workspaces-root target/preview-workspaces`. Load `http://localhost:7440/` and confirm a "Pixel Office ↗" link appears and points at port 7441. The browser console shows no errors.
  - With no sibling listening, the header looks as it did before.

## Phase 4: `code-reviewer` alignment

Everything in this phase stays synchronous and avoids async runtimes, HTTP clients, and TLS stacks.

### T4.1 `m1 sync` installs both binaries

- [x] Done
- **PR:** #43
- **Issue:** #21
- **Covers:** R4.1
- **Depends on:** nothing
- **Files:** `crates/code-reviewer/src/m1.rs`
- **Steps:**
  1. Move the `format!` that builds `build_and_provision` out of `sync_everything_to_m1` into `fn render_remote_build_script(remote_dir_expr: &str, brew: &str, cargo: &str, npm: &str) -> String`. The generated script must stay byte-for-byte the same.
  2. Do not add install lines: since #2 the script already runs `cargo install --path . --locked --force` and then `cargo install --path crates/code-reviewer --locked --force`. This task only makes that testable.
  3. Update the doc comment on `sync_everything_to_m1`: step 2 of its list still says only `~/.cargo/bin/code-reviewer` is installed.
- **Tests** in `crates/code-reviewer/src/m1.rs`:
  - `remote_build_script_installs_both_binaries_before_provisioning`: the script contains `cargo install --path . --locked --force` and `cargo install --path crates/code-reviewer --locked --force`, **in that order** (root crate first), and both appear before `m1-provision.sh`.
- **Acceptance:** `cargo test -p code-reviewer m1::tests` passes. Do not run `m1 sync`.

### T4.2 Provisioning installs the dashboard services

- [ ] Done
- **Issue:** #22
- **Covers:** R4.2, R4.3
- **Depends on:** T2.1, T4.1
- **Files:** `crates/code-reviewer/scripts/m1-provision.sh`
- **Steps:**
  1. Add section 8 from design 3.10 immediately before the final `echo "[m1-provision] Provisioning complete."` line.
  2. A non-zero exit from the install script is turned into a warning by provisioning (R4.3); it does not abort the rest of provisioning.
  3. Keep the file POSIX `sh` with LF line endings.
- **Acceptance:**
  - `sh -n crates/code-reviewer/scripts/m1-provision.sh` exits 0 (Git Bash on Windows works).
  - `git ls-files --eol crates/code-reviewer/scripts/m1-provision.sh` shows `i/lf`.
  - `grep -n "dashboards install-script" crates/code-reviewer/scripts/m1-provision.sh` shows the new lines.

### T4.3 `code-reviewer ui` never opens a second tunnel

- [x] Part a done
- PR: #44
- [ ] Part b done
- **Issue:** #23
- **Covers:** R3.6, R3.9
- **Depends on:** T0.1, T2.3
- **Files:** `crates/code-reviewer/src/server.rs`
- **Steps:**
  1. Add `fn reviewer_fallback_tunnel_args(host: &str, port: u16) -> Vec<String>`: `-N`, `-o ExitOnForwardFailure=yes`, `-o ServerAliveInterval=30`, `-o ServerAliveCountMax=3`, `-o ConnectTimeout=10`, `-o BatchMode=yes`, `-L 127.0.0.1:<port>:127.0.0.1:<port>`, `<host>`. Only one `-L`.
  2. Add `fn find_cheap_otto_binary() -> Option<PathBuf>`: a file named `cheap-otto` or `cheap-otto.exe` in the directory of `std::env::current_exe()`; otherwise `Some(PathBuf::from("cheap-otto"))` if `cheap-otto --version` runs successfully; otherwise `None`. Build that probe in `fn cheap_otto_version_probe() -> Command` and pass it through `crate::brains::strip_github_credentials`, so the probe does not inherit `GH_TOKEN` or `GITHUB_TOKEN`.
  3. Add `fn should_delegate_to_cheap_otto(requested_host: Option<&str>, peer_host: Option<&str>, port: u16) -> bool` (design 3.10 step 3): true only when the host is none or the peer in `peer.json` and `port == DEFAULT_UI_PORT`. Read the real `ui` arguments first: `cheap-otto ui` accepts `--port`, `--pixel-port`, `--state-dir`, `--no-open`, `--execute` and no `--host`.
  4. Rewrite `open_tunneled_ui` to the five steps in design 3.10. Delegate with `cheap-otto ui --execute --no-open` (plus `--state-dir <dir>` when given) only when `should_delegate_to_cheap_otto` is true; otherwise use the fallback. Build the delegated command in `fn cheap_otto_ui_command(bin: &Path, state_dir: Option<&Path>) -> Command` and the fallback in `fn reviewer_fallback_tunnel_command(host: &str, port: u16) -> Command`; both call `strip_github_credentials` before returning. Remove the `DEFAULT_PIXEL_PORT` forward.
  5. Add `fn fallback_poll_step(port_open: bool, child_exited: bool, deadline_passed: bool) -> FallbackStep` (`Ready`, `Wait`, `FailExited`, `FailTimedOut`; an open port wins over the other two) and poll with it for up to 5 s. On either failure, kill the fallback child if it is still running and `wait()` on it, so no ssh process is left behind, then return `Err` with a message that names the port and suggests `cheap-otto dashboards status --execute`. Print "ready" and open the browser only on `Ready`.
- **Tests** in `crates/code-reviewer/src/server.rs` (add a `#[cfg(test)] mod tests` if the file has none):
  - `fallback_tunnel_forwards_only_the_reviewer_port`: exactly one `-L`, it names the given port, `7421` does not appear, and the args contain `ServerAliveCountMax=3` and `ConnectTimeout=10`.
  - `delegation_is_skipped_for_custom_host_or_port`: a host other than the peer, or a port other than 7430, returns false; no host or the peer host on 7430 returns true.
  - `fallback_poll_step_decides_ready_wait_exited_or_timed_out`: all eight input combinations give the expected step.
  - `delegation_and_fallback_commands_strip_github_credentials`: for the version probe, the delegated `ui` command and the fallback tunnel command, `Command::get_envs()` shows both `GH_TOKEN` and `GITHUB_TOKEN` removed (the same assertion style as the existing `strip_github_credentials` test in `brains.rs`).
- **Parts** (one PR each, in this order; see the PR plan):
  - **a. Fallback tunnel** (PR 4, needs only T0.1): steps 1 and 5, and from step 4 `reviewer_fallback_tunnel_command` (through `strip_github_credentials`) and removing the `DEFAULT_PIXEL_PORT` forward. `open_tunneled_ui` always takes the fallback path for now. `cheap-otto dashboards status` does not exist yet, so the failure message names the port and suggests checking the Reviewer service with `code-reviewer m1 status`; part b switches the hint to `cheap-otto dashboards status --execute`. Tests: `fallback_tunnel_forwards_only_the_reviewer_port`, `fallback_poll_step_decides_ready_wait_exited_or_timed_out`. The PR says `part of #23`.
  - **b. Delegate to `cheap-otto ui`** (PR 14, needs T2.3): steps 2 and 3, the rest of step 4, and the step 5 hint naming `cheap-otto dashboards status --execute`. Tests: `delegation_is_skipped_for_custom_host_or_port`, `delegation_and_fallback_commands_strip_github_credentials`. The PR closes #23.
- **Acceptance:** `cargo test -p code-reviewer server::tests` passes. Do not run `code-reviewer ui`.

### T4.4 The Pixel Office button starts the service, not an orphan

- [x] Done
- PR: #49
- **Issue:** #24
- **Covers:** R1.2 (keeps Pixel Office under `launchd`)
- **Depends on:** T0.1, T1.1
- **Files:** `crates/code-reviewer/src/server.rs`, `tests/dashboard_labels.rs` (new, root crate)
- **Steps:**
  1. Add `pub const PIXEL_OFFICE_LAUNCHD_LABEL: &str = "com.cheapotto.dashboards.pixel-office";` with a comment naming `src/dashboards.rs` in the root crate as the source of truth. A comment is not enough to keep the two in step, so add the parity test in step 3.
  2. In `start_pixel_agents_server`, after the "already listening" check: on macOS (`cfg!(target_os = "macos")`), get the uid from `id -u` and run `launchctl kickstart gui/<uid>/<label>`. If it exits 0, go to the existing wait loop. If it fails, return `Err` saying the Pixel Office service is not installed and naming `cheap-otto dashboards install --execute`; do **not** fall through to the direct spawn, which would leave an orphan outside `launchd` holding the port. Keep the direct spawn only for non-macOS hosts. Decide between the three outcomes with a pure `fn pixel_start_action(is_macos: bool, kickstart_ok: Option<bool>) -> PixelStartAction` (`WaitForService`, `ReportNotInstalled`, `SpawnDirectly`).
  3. Create `tests/dashboard_labels.rs` in the root crate with `#![forbid(unsafe_code)]`: `include_str!("../crates/code-reviewer/src/server.rs")` and assert it contains the literal declaration of `PIXEL_OFFICE_LAUNCHD_LABEL` with the label the registry gives for `DashboardId::PixelOffice` (`DashboardSet::standard()`).
- **Tests:**
  - in `crates/code-reviewer/src/server.rs`: `pixel_start_never_spawns_directly_on_macos`: on macOS a successful kickstart gives `WaitForService` and a failed one gives `ReportNotInstalled`; off macOS the result is `SpawnDirectly`.
  - in `tests/dashboard_labels.rs`: `reviewer_pixel_office_label_matches_the_registry`.
- **Acceptance:** Gate passes, including `cargo test -p cheap-otto --test dashboard_labels`. `grep -n "PIXEL_OFFICE_LAUNCHD_LABEL" crates/code-reviewer/src/server.rs` shows the constant and its use.

### T4.5 `open-ui.ps1` becomes a wrapper

- [ ] Done
- **Issue:** #25
- **Covers:** R3.9
- **Depends on:** T4.3
- **Files:** `crates/code-reviewer/scripts/open-ui.ps1`
- **Steps:**
  1. Keep the `param` block and `$ErrorActionPreference = "Stop"`.
  2. The script is a wrapper over `code-reviewer ui` and does not manage tunnels itself (R3.9). Replace the body: build an argument array `run --quiet -- ui --host $HostAddr --port $Port` (add `--no-open` when `-NoOpen`), run `& cargo @argsList` from the crate directory inside `Push-Location`/`Pop-Location`, and `throw` when `$LASTEXITCODE` is not 0.
  3. Remove the `Start-Process ssh` block and the health polling.
- **Acceptance:** `grep -n "Start-Process" crates/code-reviewer/scripts/open-ui.ps1` prints nothing. Do not run the script.

### T4.6 Shared ports, and a link back to Mission Control

- [x] Done
- PR: #48
- **Issue:** #26
- **Covers:** R2.5, R6.3
- **Depends on:** T0.1, T1.1
- **Files:** `crates/code-reviewer/src/server.rs`, `crates/code-reviewer/src/ui_html.rs`, `tests/dashboard_ports.rs` (new, root crate)
- **Steps:**
  1. Add `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = 7420;` beside `DEFAULT_UI_PORT`, with a comment naming `src/dashboards.rs` as the source of truth.
  2. In `DASHBOARD_HTML`, add a link to Mission Control in the top bar: `href = 'http://' + location.hostname + ':7420/'`, `target="_blank"`, `rel="noopener"`, label `MISSION CONTROL ↗`, styled like the existing top-bar buttons. The string `"##` must not appear in the page.
  3. Create `tests/dashboard_ports.rs` with `#![forbid(unsafe_code)]`: `include_str!("../crates/code-reviewer/src/server.rs")` and assert it contains `pub const DEFAULT_UI_PORT: u16 = {REVIEWER_PORT};`, `pub const DEFAULT_PIXEL_PORT: u16 = {PIXEL_OFFICE_PORT};` and `pub const DEFAULT_MISSION_CONTROL_PORT: u16 = {MISSION_CONTROL_PORT};`, formatting the values from `cheap_otto::dashboards`.
- **Tests:**
  - `reviewer_ports_match_the_registry` in `tests/dashboard_ports.rs`.
  - `dashboard_html_links_mission_control_on_the_shared_port` in the `#[cfg(test)] mod tests` of `crates/code-reviewer/src/server.rs`: asserts `crate::ui_html::DASHBOARD_HTML` contains `format!(":{DEFAULT_MISSION_CONTROL_PORT}/")`.
- **Acceptance:**
  - `cargo test -p cheap-otto --test dashboard_ports` passes.
  - `cargo test -p code-reviewer server::tests` passes and includes the new test.
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
  3. `docs/index.html`: in the `QS` array, update the "Watch" tab to the same commands. Every command must run as written (rule 09).
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
- **Files:** `CLAUDE.md`, `.claude/rules/05-crate-ecosystem-and-cli-patterns.md`, `.claude/rules/07-code-reviewer.md`, `.claude/rules/08-embedded-web-ui.md`, `specs/README.md`, `specs/m1-dashboards/README.md`
- **Steps:**
  1. `CLAUDE.md`: in the naming contract add `com.cheapotto.dashboards.<id>` and `~/.cheap-otto/logs/dashboards/<id>.log`; add `tunnel.json` to the local state list; add `dashboards.rs` where the architecture names the registry; add `dashboards install|uninstall|restart|tunnel` to the commands that need `--execute`.
  2. Rule 05: add `dashboards` to the dry-run examples and note the reserved project name.
  3. Rule 07: update the `m1 sync` description (both binaries, dashboard services) and the launchd label list.
  4. Rule 08: replace the "how it runs" notes with the service model; the preview table is unchanged.
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
  8. KeepAlive: stop only the Mission Control job, with single quotes so `$(id -u)` runs on the Mac: `cargo run -p code-reviewer -- m1 exec --cmd 'launchctl kill SIGTERM gui/$(id -u)/com.cheapotto.dashboards.mission-control'`. Wait 15 seconds, then `dashboards status --execute` shows Mission Control `listening=yes` with a new pid. Do not use `pkill -f`: its pattern also matches the remote shell's own command line and any other `cheap-otto serve`.
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
