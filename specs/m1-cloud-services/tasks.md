# Tasks: Docker, PostgreSQL and app servers served from the M1

Read `../README.md`, `requirements.md` and `design.md` before starting any task. Every PR stays within 600 changed lines (`Cargo.lock` excluded) and passes the three workspace gates (`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --locked`).

## PR plan

| PR | Tasks | Branch | Needs |
| --- | --- | --- | --- |
| 1 | T1.1 service config & port validation | `feat/m1-cloud-services-t1-1-config` | nothing |
| 2 | T1.2 Colima VM sizing & remote shell scripts | `feat/m1-cloud-services-t1-2-scripts` | PR 1 |
| 3 | T1.3 SSH socket/port tunnel & Docker context specs | `feat/m1-cloud-services-t1-3-tunnel` | PR 1 |
| 4 | T1.4 hardware memory budget & arbiter alignment | `feat/m1-cloud-services-t1-4-budget` | PR 2 |
| 5 | T2.1 `services status` and `services ports` CLI | `feat/m1-cloud-services-t2-1-cli-ports` | PRs 2, 3 |
| 6 | T2.2 `services up` and `services down` CLI | `feat/m1-cloud-services-t2-2-up-down` | PR 5 |
| 7 | T3.1 Mission Control `GET /api/services` & UI card | `feat/m1-cloud-services-t3-1-hub` | PR 3 |
| 8 | T4.1 user & agent docs | `docs/m1-cloud-services-t4-1-docs` | PRs 6, 7 |

After PR 8, the owner runs **LIVE** tasks T5.1 and T5.2 against the paired MacBook and workstation.

## Phase 1: pure planners (`src/services.rs`, `src/hardware.rs`)

### T1.1 Service config, port mappings and dashboard collision guard

- [ ] Open
- **Covers:** R1.1, R3.1, R3.2
- **Depends on:** nothing
- **Files:** `src/services.rs` (new), `src/lib.rs`
- **Steps:**
  1. Create `src/services.rs` with `DEFAULT_DOCKER_LOCAL_PORT` (`23750`), `REMOTE_POSTGRES_PORT` (`5432`), `DOCKER_CONTEXT_NAME` (`"otto-mac"`), `BackingServiceId` (`Colima`, `Postgres`), `PortMapping`, `ServicesConfig`, and `ServicesError`.
  2. Implement `ServicesConfig::default()`, `validate(&DashboardSet)`, `add_app_port`, and `remove_app_port`, rejecting port `0`, duplicates, and any local port in `DashboardSet::ports()`.
  3. Export `pub mod services;` in `src/lib.rs` in alphabetical order.
- **Tests** in `src/services.rs`:
  - `default_config_uses_standard_docker_and_postgres_ports`
  - `rejects_zero_duplicate_and_dashboard_colliding_ports`
  - `adds_and_removes_sorted_app_port_mappings`
- **Acceptance:** `cargo test -p cheap-otto --lib services::tests` passes.

### T1.2 Colima VM sizing, remote startup script and status parser

- [ ] Open
- **Covers:** R1.2, R1.3, R1.4, R1.5
- **Depends on:** T1.1
- **Files:** `src/services.rs`
- **Steps:**
  1. Add `ColimaVmSpec::for_hardware(&HardwareProfile)` (`4` CPU / `4` GiB on `>= 16` GiB Apple Silicon, `2` CPU / `2` GiB on smaller hosts, `vz` + `virtiofs`).
  2. Implement `render_services_up_script(&ColimaVmSpec)` and `render_services_status_script()`, binding strictly to loopback (`127.0.0.1`) and printing structured `KEY=VALUE` lines.
  3. Implement `parse_services_status(&str) -> ServicesStatusReport`.
- **Tests** in `src/services.rs`:
  - `sizes_colima_vm_for_16gb_m1_and_8gb_hosts`
  - `up_script_starts_postgres_and_colima_without_lan_binds` (also asserts the `listen_addresses = '127.0.0.1'` check and the restart when it differs)
  - `parses_remote_services_status_output` (also covers `POSTGRES_BIND`)
- **Acceptance:** `cargo test -p cheap-otto --lib services::tests` passes.

### T1.3 SSH Unix-socket + port tunnel spec and Docker context commands

- [ ] Open
- **Covers:** R2.1, R2.2, R2.3, R2.4, R3.4, R3.5
- **Depends on:** T1.1
- **Files:** `src/services.rs`
- **Steps:**
  1. Implement `validate_remote_unix_socket(&str)` accepting only absolute `/Users/...` or `/var/...` POSIX paths and rejecting colons, whitespace or shell metacharacters.
  2. Implement `build_services_tunnel_spec(&PeerNode, &ServicesConfig, &str)` with `-N -F none`, the five `TUNNEL_SSH_OPTIONS`, `127.0.0.1:<docker_port>:<remote_sock>`, optional Postgres forward, app forwards, and `-- <user>@<host>`.
  3. Add `ServicesTunnelRecord` (with `process_started_unix_ms` and `ssh_port`), a `command_line_is_services_tunnel` helper that matches the exact argv from `build_services_tunnel_spec`, and `build_docker_context_upsert_specs` / `build_docker_context_restore_spec`.
- **Tests** in `src/services.rs`:
  - `rejects_unsafe_remote_unix_socket_paths`
  - `builds_ssh_tunnel_with_socket_postgres_and_app_forwards`
  - `builds_docker_context_create_update_and_use_commands`
  - `services_tunnel_record_requires_exact_argv_and_matching_peer`
- **Acceptance:** `cargo test -p cheap-otto --lib services::tests` passes.

### T1.4 Unified memory budget & arbiter alignment for 4 GiB Colima

- [ ] Open
- **Covers:** R4.1
- **Depends on:** T1.2
- **Files:** `src/hardware.rs`, `src/arbiter.rs`
- **Steps:**
  1. Add a helper on `MemoryBudget` / `ColimaVmSpec` accounting for active 4 GiB Colima co-tenancy alongside CI/agent workloads without breaking existing `hardware::tests`.
  2. Add/ensure `WorkloadArbiter` pauses batch queues on `ThermalLevel::Serious` or low available unified memory when Colima is active.
- **Tests:**
  - `colima_active_budget_preserves_safety_headroom_on_16gb_air`
  - `colima_budget_headroom_boundary_is_inclusive_at_threshold`
  - `arbiter_pauses_batch_queues_when_colima_active_and_memory_low`
- **Acceptance:** `cargo test -p cheap-otto --lib hardware::tests` and `arbiter::tests` pass.

## Phase 2: CLI orchestration (`src/main.rs`)

### T2.1 `cheap-otto services status` and `services ports`

- [ ] Open
- **Covers:** R1.5, R3.1, R3.3, R4.3
- **Depends on:** T1.2, T1.3
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Add `Services` subcommand (`status`, `ports list|add|remove`) gated by `--execute` / `--apply`.
  2. Persist `state_dir.join("services.json")` and, on `status --execute`, `state_dir.join("services-status.json")`, each written to a sibling temp file in the same directory and then atomically renamed over the target, so `GET /api/services` never reads a torn file and maps absent or corrupt content to `health: null`. Hot-reload the services tunnel if `state_dir.join("services-tunnel.json")` is active. `state_dir` is the resolved state directory (`--state-dir`, then `./.cheap-otto`, then the repo root, then `~/.cheap-otto`).
- **Tests** in `tests/cli_integration.rs`: `services_ports_and_status_dry_run_and_temp_state_work` runs with a temp `--state-dir`, asserts `services-status.json` is written there and not under `./.cheap-otto`, and checks that no temp file is left behind
- **Acceptance:** `cargo test --test cli_integration services_ports` passes.

### T2.2 `cheap-otto services up` and `services down`

- [ ] Open
- **Covers:** R1.3, R2.3, R2.4, R2.5, R3.5, R4.3
- **Depends on:** T2.1
- **Files:** `src/main.rs`, `tests/cli_integration.rs`
- **Steps:**
  1. Implement `services up` (dry-run prints remote start script + SSH tunnel + `docker context` commands; `--execute` runs remote startup over SSH stdin, spawns detached `ssh -N -F none`, verifies local forwards open within 5 s, records `services-tunnel.json`, and configures `docker context` when `docker` is on `PATH`).
  2. Implement `services down` (verifies the recorded PID, start time, command line and peer before stopping, restores `default` Docker context if `otto-mac` was active, and optionally stops remote Colima on `--stop-remote`).
- **Tests** in `tests/cli_integration.rs`: `services_up_and_down_dry_run_and_isolated_stop_work`
- **Acceptance:** `cargo test --test cli_integration services_up_and_down` passes.

## Phase 3: Mission Control visibility & documentation

### T3.1 Mission Control `GET /api/services` and UI summary card

- [ ] Open
- **Covers:** R4.2
- **Depends on:** T1.3
- **Files:** `src/server.rs`, `src/ui.html` (or embedded Mission Control view)
- **Steps:**
  1. Add `GET /api/services` behind `RequestGuard`, reading `services.json`, `services-tunnel.json` and `services-status.json` under `state.config.state_dir`.
  2. Render a compact M1 Backing Services card (`Docker :23750`, `Postgres :5432`, forwarded app ports) in Mission Control.
- **Tests** in `src/server.rs`: `api_services_returns_configured_forwards_and_tunnel_state` (covers health present and null)
- **Acceptance:** `cargo test -p cheap-otto --lib server::tests` passes.

### T4.1 Operator and agent documentation

- [ ] Open
- **Covers:** R2.3, R2.5, R3.1
- **Depends on:** T2.2, T3.1
- **Files:** `README.md`, `CLAUDE.md`, `AGENTS.md`
- **Steps:** Document `cheap-otto services up --use-context --execute`, `services ports add 3000 --execute`, and the live-appliance safety rule for `services` commands.
- **Acceptance:** All three workspace CI gates pass.

## Phase 4: LIVE verification (owner-approved)

### T5.1 LIVE: Serve Docker, PostgreSQL 17 and a container HTTP port from M1 to Windows

- [ ] Open (**LIVE** — owner approval required)
- **Covers:** R1.3, R2.1, R2.3, R3.1
- **Acceptance:** On Windows, `cheap-otto services up --use-context --port 8088:8088 --execute` succeeds; `docker info` shows the M1's `aarch64` Colima server; `pg_isready` / TCP connect to `127.0.0.1:5432` reaches M1 PostgreSQL 17; `docker run -d --rm -p 127.0.0.1:8088:80 nginx:alpine` answers at `http://127.0.0.1:8088` on Windows while dashboard ports `7420`/`7430` stay uninterrupted.

### T5.2 LIVE: Clean teardown & reboot resilience

- [ ] Open (**LIVE** — owner approval required)
- **Covers:** R2.4, R3.5
- **Acceptance:** `cheap-otto services down --execute` closes `:23750` and `:8088`, leaves `.cheap-otto/tunnel.json` (`:7420`, `:7430`) up, and restores the workstation's default Docker context.
