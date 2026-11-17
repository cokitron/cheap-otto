# m1-cloud-services: Docker, PostgreSQL and app servers served from the Mac

**Status:** planned (`requirements.md`, `design.md` and `tasks.md` ready; Phases 1–3 ship as 8 PRs under the 600-line cap, followed by two owner-approved **LIVE** verification tasks T5.1 and T5.2).

## The goal in one paragraph

Turn the paired Apple Silicon MacBook (`otto-mac`) into the workstation's remote container, database and dev-server host alongside `m1-dashboards`. One command (`cheap-otto services up --use-context --execute`) ensures PostgreSQL 17 and a memory-bounded Colima VM (`4` CPU / `4` GiB `vz` + `virtiofs`) are running on the Mac, opens one dedicated `ssh -N -F none` tunnel forwarding `127.0.0.1:23750` directly to the Mac's `~/.colima/default/docker.sock` Unix socket plus `127.0.0.1:5432` and any registered container/dev-server ports (`3000`, `8080`, …), and wires a `docker` CLI context (`otto-mac`) on Windows so `docker compose up`, `docker build`, ORMs and browsers work against `localhost` with zero Docker Desktop overhead on Windows and zero LAN port exposure on Wi-Fi.

## What "done" looks like

- `cheap-otto services up --use-context --execute` on Windows starts or verifies `postgresql@17` and right-sized Colima (`4` GiB) on the Mac, opens `.cheap-otto/services-tunnel.json`, and configures Docker context `otto-mac` (`tcp://127.0.0.1:23750`).
- `docker ps`, `docker build` and `docker compose up` on Windows execute natively on the M1's ARM64 Colima engine over the SSH socket forward.
- `127.0.0.1:5432` (or `--postgres-port <port>`) on Windows reaches PostgreSQL 17 on the Mac.
- `cheap-otto services ports add 3000 --execute` forwards container or dev-server port `3000` to `127.0.0.1:3000` on Windows and hot-reloads only `.cheap-otto/services-tunnel.json`, never interrupting the dashboard tunnel (`7420`, `7421`, `7430`).
- Mission Control (`:7420`) surfaces live backing-service and port-bridge state via `GET /api/services`.

## Read in this order

1. [`../README.md`](../README.md): the specs protocol and PR size budget (`<= 600` lines).
2. [`requirements.md`](requirements.md): four requirements (`R1`–`R4`) with EARS acceptance criteria.
3. [`design.md`](design.md): architecture, pure `src/services.rs` interfaces, CLI commands, and key decisions.
4. [`tasks.md`](tasks.md): 10 tasks across Phases 1–4 and an 8-PR rollout table.
