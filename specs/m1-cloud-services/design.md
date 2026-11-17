# Design: Docker, PostgreSQL and app servers served from the M1

## 1. Current state and motivation

- **Dashboards are unified (`src/dashboards.rs`):** Mission Control (`7420`), Pixel Office (`7421`) and Reviewer (`7430`) run under `launchd` and share one tunnel recorded in `.cheap-otto/tunnel.json`.
- **Workloads lack a managed service bridge:** On the MacBook Air, Colima exposes Docker at `~/.colima/default/docker.sock` and Homebrew `postgresql@17` listens on `127.0.0.1:5432`. Today:
  1. Colima's default `6 GiB` VM allocation competes with parallel `code-reviewer` / `claude` / `agy` check gates on 16 GiB unified memory (`hardware.rs` currently budgets `1024 MiB` for containers).
  2. Non-interactive `ssh user@host docker system dial-stdio` fails on macOS unless `/opt/homebrew/bin` is on `sshd`'s minimal `PATH` and `DOCKER_HOST` points to `~/.colima/default/docker.sock`. Forwarding workstation `127.0.0.1:23750` directly to `/Users/<user>/.colima/default/docker.sock` via OpenSSH `-L` avoids `sshd` `PATH` fragility completely and works with Windows `docker` CLI, Docker Compose and Testcontainers.
  3. Re-creating the dashboard tunnel every time a developer adds or drops dev port `3000` or `8080` would disrupt open dashboard tabs. Keeping a dedicated `.cheap-otto/services-tunnel.json` decouples control-plane dashboards from dev/container ports while enforcing disjoint port validation against `DashboardSet`.

## 2. Architecture

```text
Workstation (Windows / Linux / macOS)                Paired MacBook (otto-mac, 127.0.0.1 only)
┌──────────────────────────────────────────┐         ┌──────────────────────────────────────────────┐
│ docker / compose (context: otto-mac)     │         │ Colima VM (vz + virtiofs, 4 CPU / 4 GiB)     │
│   └─► 127.0.0.1:23750 ───────────────────┼── SSH ──┼─► /Users/<user>/.colima/default/docker.sock  │
│ psql / ORM                               │  -F none│                                              │
│   └─► 127.0.0.1:5432 ────────────────────┼── -N  ──┼─► 127.0.0.1:5432 (postgresql@17 launchd)     │
│ Browser / API clients                    │         │                                              │
│   └─► 127.0.0.1:3000, :8080, ... ────────┼─────────┼─► 127.0.0.1:3000, :8080 (containers / apps)  │
└──────────────────────────────────────────┘         └──────────────────────────────────────────────┘
```

## 3. Pure core interfaces (`src/services.rs`) [R1, R2, R3, R4]

`src/services.rs` is 100% pure (no I/O, no clock, `#![forbid(unsafe_code)]`), mirroring `src/dashboards.rs`:

- **Constants:**
  - `DEFAULT_DOCKER_LOCAL_PORT: u16 = 23750`
  - `REMOTE_POSTGRES_PORT: u16 = 5432`
  - `DOCKER_CONTEXT_NAME: &str = "otto-mac"`
- **Configuration & validation (`ServicesConfig`):** [R2.2, R3.1, R3.2]
  - `docker_local_port: u16`
  - `postgres_local_port: Option<u16>` (`Some(5432)` by default)
  - `app_ports: Vec<PortMapping>` (`{ local: u16, remote: u16 }`, sorted & deduplicated by `local`)
  - `ServicesConfig::validate(&self, reserved: &DashboardSet) -> Result<(), ServicesError>` rejects port `0`, duplicate local ports across Docker/Postgres/app mappings, and any local port present in `reserved.ports()`.
- **Colima VM profile (`ColimaVmSpec::for_hardware(&HardwareProfile)`):** [R1.2, R4.1]
  - `>= 16 GiB` Apple Silicon: `cpus = 4`, `memory_gib = 4`, `disk_gib = 40`, `vm_type = "vz"`, `mount_type = "virtiofs"`.
  - `< 16 GiB`: `cpus = 2`, `memory_gib = 2`, `disk_gib = 30`.
- **Remote shell scripts (streamed over `ssh ... /bin/sh -s` stdin via `PeerNode::build_ssh_stdin_spec`):** [R1.3, R1.4, R1.5]
  - `render_services_up_script(&ColimaVmSpec) -> String`: sets `PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"`, ensures `postgresql@17` is started via `brew services start postgresql@17`, starts `colima start --cpu <c> --memory <m> --disk <d> --vm-type vz --mount-type virtiofs` when `colima status` is down, waits up to 30 s for `$HOME/.colima/default/docker.sock` (`[ -S ... ]`) and `127.0.0.1:5432`, and prints `SOCKET=$HOME/.colima/default/docker.sock`.
  - `render_services_status_script() -> String` & `parse_services_status(&str) -> ServicesStatusReport`: parses key-value status lines (`COLIMA=running|stopped`, `DOCKER_SOCK=/Users/...`, `CONTAINERS=<n>`, `POSTGRES=listening|down`, `SWAP_USED_MB=<n>`).
- **SSH services tunnel planner:** [R2.1, R2.2, R3.4, R3.5]
  - `validate_remote_unix_socket(path: &str) -> Result<&str, ServicesError>` requires a clean absolute path (`/Users/...` or `/private/var/...` or `/var/...`) containing only ASCII alphanumeric, `/`, `.`, `_`, `-` (no `:`, whitespace or quotes).
  - `build_services_tunnel_spec(peer: &PeerNode, cfg: &ServicesConfig, remote_docker_sock: &str) -> Result<CommandSpec, ServicesError>` produces:
    `ssh -N -F none -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o BatchMode=yes -o ConnectTimeout=10 -p <ssh_port> -L 127.0.0.1:23750:/Users/<user>/.colima/default/docker.sock [-L 127.0.0.1:5432:127.0.0.1:5432] [-L 127.0.0.1:P:127.0.0.1:R ...] -- <user>@<host>`.
  - `ServicesTunnelRecord` (`.cheap-otto/services-tunnel.json`): stores `pid`, `process_started_unix_ms` (matching PR #63's PID-reuse guard), `docker_local_port`, `remote_docker_sock`, `forwarded_ports`, and `peer_host`.
- **Docker context command specs:** [R2.3, R2.4]
  - `build_docker_context_upsert_specs(docker_local_port: u16, activate: bool) -> Vec<CommandSpec>` generates `docker context inspect otto-mac` / `docker context create` or `docker context update otto-mac --docker host=tcp://127.0.0.1:23750`, plus optional `docker context use otto-mac`.

## 4. CLI surface (`src/main.rs`) and Mission Control API (`src/server.rs`)

- `cheap-otto services status [--execute]` — prints remote probe plan or live status table (`colima`, `postgres`, local tunnel PID, Docker context, forwarded ports). [R1.5, R4.3]
- `cheap-otto services up [--use-context] [--no-postgres] [--postgres-port <u16>] [--docker-port <u16>] [--port <local[:remote]>...] [--execute]` — starts remote backing services, opens/refreshes `.cheap-otto/services-tunnel.json`, configures Docker context `otto-mac`. [R1.3, R2.3, R2.4, R3.1]
- `cheap-otto services down [--stop-remote] [--execute]` — terminates the verified `services-tunnel.json` SSH process, switches Docker context off `otto-mac` if active, and (only when `--stop-remote` is passed) stops Colima on the Mac. [R2.4, R3.5]
- `cheap-otto services ports <add|remove|list> [local[:remote]] [--execute]` — edits `app_ports` in `.cheap-otto/services.json` and hot-reloads the services tunnel if running. [R3.3]
- `GET /api/services` in `src/server.rs` — returns JSON `{ config, tunnel_active, forwards }` read from `.cheap-otto/services.json` and `services-tunnel.json` so Mission Control displays live Docker/Postgres/port bridge cards alongside dashboards. [R4.2]

## 5. Key decisions & open questions (with defaults)

1. **Separate `services-tunnel.json` vs mutating `tunnel.json`:** *Default: separate `services-tunnel.json`.* Adding/removing dev server ports (`3000`, `5173`) restarts only the services tunnel; Mission Control (`7420`) and Code Tender (`7430`) never flicker.
2. **Default local PostgreSQL port:** *Default: `5432`*, overridable via `--postgres-port 15432` or disabled via `--no-postgres` when a workstation runs a local Postgres instance.
3. **Colima memory cap on 16 GiB M1 Air (`hardware.rs`):** *Default: `4 GiB` (`4096 MiB`)* with Apple `vz` dynamic ballooning; updates `HardwareProfile::compute_memory_budget` or `ServicesBudget` while preserving existing unit-test invariants unless explicitly updated in T1.4.
