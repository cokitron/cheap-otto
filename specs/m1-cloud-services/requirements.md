# Requirements: Docker, PostgreSQL and app servers served from the M1

## Introduction

`m1-dashboards` unified the three control-plane dashboards (`7420`, `7421`, `7430`) behind `launchd` services and one SSH tunnel. Development workloads still need the MacBook (`otto-mac`) to act as the workstation's remote container, database and dev-server host: running Colima (Docker Engine), PostgreSQL 17, and container/app HTTP servers on Apple Silicon while exposing them on `127.0.0.1` on the Windows workstation without opening unauthenticated LAN ports.

This spec adds a pure planner and CLI workflow (`cheap-otto services ...`) that manages Colima and PostgreSQL on the Mac, forwards the remote Colima Unix socket, PostgreSQL port and registered app ports over one authenticated SSH tunnel, wires a `docker` CLI context (`otto-mac`) on the workstation, and surfaces service health in Mission Control.

## Glossary

| Term | Meaning |
| --- | --- |
| Mac | The paired headless MacBook (`PeerNode` in `src/bridge.rs`, alias `otto-mac`) |
| Workstation | The operator's Windows (or Linux/macOS) machine running `cheap-otto` |
| Backing service | A daemon managed on the Mac for workloads: `colima` (Docker Engine) or `postgres` (`postgresql@17`) |
| Services tunnel | One `ssh -N -F none` process recorded in `.cheap-otto/services-tunnel.json` forwarding Docker, Postgres and app ports |
| Docker forward | Local TCP `127.0.0.1:23750` forwarded by SSH directly to the Mac's `/Users/<user>/.colima/default/docker.sock` Unix socket |
| App port | A loopback TCP port (`1..=65535`) registered by the operator to reach a container or dev server running on the Mac |
| Execute gate | `--execute` (alias `--apply`); without it, mutating commands only print their deterministic plan |

## Requirements

### R1. Backing services on the Mac (`colima` and `postgres`)

**User story:** As the operator, I want Colima and PostgreSQL 17 started, right-sized for unified memory, and checked on the Mac from one command, so containers and databases run off my workstation.

1. The system SHALL model the backing services in a pure module `src/services.rs` with stable ids `colima` and `postgres`.
2. WHEN planning Colima startup for a [`HardwareProfile`] (`src/hardware.rs`) THEN the system SHALL size the VM within unified memory (`4` CPUs and `4` GiB RAM on 16 GiB hosts, `2` CPUs and `2` GiB RAM on 8 GiB hosts) using `--vm-type vz --mount-type virtiofs` on Apple Silicon.
3. WHEN `cheap-otto services up --execute` runs THEN the system SHALL start PostgreSQL (`brew services start postgresql@17`) and Colima on the Mac if not already running, wait until `~/.colima/default/docker.sock` exists and `127.0.0.1:5432` accepts connections (up to 30 s), and verify that PostgreSQL's `listen_addresses` is loopback-only (set to `127.0.0.1` and restart PostgreSQL if it is not), and exit non-zero with diagnosis if any of these fails.
4. The system SHALL NOT configure Colima or PostgreSQL to listen on a non-loopback Mac network interface.
5. WHEN `cheap-otto services status --execute` runs THEN the system SHALL probe the Mac over SSH stdin and report `colima` state, Docker socket path, running container count, `postgres` listener state on `127.0.0.1:5432`, and unified swap usage.

### R2. Remote Docker Engine context over SSH socket forwarding

**User story:** As a developer on Windows, I want `docker ps`, `docker compose up` and `docker build` to run on the M1 without Docker Desktop installed locally or TCP `2375` exposed on Wi-Fi.

1. The system SHALL forward workstation `127.0.0.1:<docker_port>` (default `23750`) to the Mac's absolute Colima Unix socket path (`/Users/<user>/.colima/default/docker.sock`) inside the services SSH tunnel.
2. The system SHALL reject any remote socket path that is not an absolute `/Users/...` or `/var/...` POSIX path free of whitespace, colons and shell metacharacters.
3. WHEN `cheap-otto services up --execute` establishes the services tunnel and `docker` is installed on the workstation THEN the system SHALL create or update the Docker context `otto-mac` pointing to `tcp://127.0.0.1:<docker_port>`.
4. IF `--use-context` is passed THEN `services up --execute` SHALL also run `docker context use otto-mac`; WHEN `services down --execute` stops the tunnel and the active Docker context is `otto-mac` THEN it SHALL switch back to `default` if present.
5. IF `docker` CLI is not installed on the workstation THEN `services up --execute` SHALL still open the SSH forwards, warn that local `docker` CLI was not found, and print `DOCKER_HOST=tcp://127.0.0.1:<docker_port>`.

### R3. PostgreSQL and app-server port forwarding

**User story:** As a developer, I want PostgreSQL and container/dev-server HTTP ports on the M1 reachable at `localhost` on my workstation without colliding with the dashboard tunnel.

1. The system SHALL persist desired service forwards in `.cheap-otto/services.json`: `docker_local_port` (default `23750`), `postgres_local_port` (default `5432`, optional `--no-postgres`), and an ordered set of `app_ports` (`local_port -> remote_port`, default `P:P`).
2. The system SHALL reject port `0`, duplicate local ports within `services.json`, and any local port that collides with [`DashboardSet`]`::ports()` (`7420`, `7421`, `7430` by default).
3. WHEN `cheap-otto services ports add <port>` or `remove <port>` runs with `--execute` THEN the system SHALL update `.cheap-otto/services.json` and, if the services tunnel is active, restart the services tunnel with the updated forward list without interrupting the dashboard tunnel (`tunnel.json`).
4. The services tunnel SHALL pass `-N -F none`, `ExitOnForwardFailure=yes`, `ServerAliveInterval=30`, `ServerAliveCountMax=3`, `BatchMode=yes`, `ConnectTimeout=10`, and `--` before `<user>@<host>`, binding every `-L` specification strictly to `127.0.0.1` on the workstation.
5. The system SHALL record the services tunnel in `.cheap-otto/services-tunnel.json` including OS process start time and forwarded targets, and SHALL verify ownership before terminating a recorded PID.

### R4. Memory & thermal coordination and Mission Control visibility

**User story:** As the operator, I want Mission Control (`:7420`) and the workload arbiter to account for Colima and PostgreSQL so parallel agent/CI runs do not push the 16 GB Air into swap thrashing.

1. [`HardwareProfile::compute_memory_budget`] (`src/hardware.rs`) or a companion `ServicesBudget` helper SHALL expose the Colima VM ceiling (`4096` MiB on 16 GiB hosts) so [`WorkloadArbiter`] (`src/arbiter.rs`) pauses background queues when thermal level reaches `Serious` or free unified memory drops below safety headroom.
2. Mission Control (`src/server.rs`) SHALL expose `GET /api/services` returning the configured service ports, the tunnel state and the last cached backing-service health persisted by `services status --execute` (`null` when never probed), guarded by `RequestGuard`, and the endpoint SHALL NOT probe the Mac itself.
3. Every mutating `services` subcommand without `--execute` (or `--apply`) SHALL perform zero network, process or state-file writes and print the exact commands it would run.
