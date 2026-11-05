# cheap-otto 🦀💻

**Turn that spare MacBook (even with a broken screen) or old laptop into a 24/7 headless AI agent host, residential browser worker, local AI gateway, and CI runner—and control it from your main Windows PC or phone over your local network or Tailscale.**

Why pay **$600–$800** for an Intel N95 "AI Agent Box" when a spare M1 MacBook Air in your drawer is **3–4x faster**, draws **~3W at idle**, has a **built-in battery UPS**, and has **Apple Silicon Unified Memory** that can run local Whisper, embeddings, and 8B LLMs for **$0**?

| Feature | Commercial "AI Agent Box" ($599–$799) | Your Old M1 Air + `cheap-otto` ($0) |
| :--- | :--- | :--- |
| **CPU / Compute** | Intel N95 (4 slow Alder Lake-N cores) | **Apple M1 (8 cores + 16-core Neural Engine)** |
| **Local AI Inference** | None (Cloud APIs only) | **Native Whisper + Embeddings + 4-bit 8B LLM (~40 tok/s)** |
| **Built-in UPS (Power Outage)** | None (dies on power flicker) | **Built-in MacBook Battery (10+ hours backup)** |
| **Residential IP Browser** | Yes | **Yes (Persistent Playwright server over home Wi-Fi)** |
| **Reversible OS Changes** | Locked appliance | **100% reversible (`cheap-otto restore`)** |

---

## Core Architecture & The 4 Appliance Pillars

### 0. Cross-Network Workstation $\leftrightarrow$ Appliance Bridge (`src/network.rs`)
* Pair your main **Windows workstation** (or phone/iPad) with the headless M1 MacBook over your home LAN (`192.168.x.x`) or Tailscale mesh (`100.x.y.z`).
* Automatically generates a `.env` bridge (`OPENAI_BASE_URL`, `OLLAMA_HOST`, `CHEAP_OTTO_WHISPER_URL`, `PLAYWRIGHT_WS_ENDPOINT`) so tools on your main PC seamlessly consume the appliance's local AI and browser workers over the network.

### 1. Autonomous Dev & PR Worker — "Overnight Junior Engineer" (`src/dev_worker.rs`)
* **Self-Hosted GitHub Actions Runner:** Native ARM64 build runner container with automatic Arbiter RAM preemption.
* **Background Agent Sandbox:** Isolated OrbStack container workflow watching `agent-fix` / `triage` GitHub labels to pull branches, run tests, fix lint/type errors, and open draft PRs overnight.
* **Scheduled Canary & E2E Monitor:** Cron-scheduled Playwright probes running from your home residential IP that page you on Slack/Telegram only when a check fails or latency spikes.

### 2. 24/7 "Chief of Staff" Agent — The Otto Replacement (`src/chief_of_staff.rs`)
* **Inbox & Calendar Triage:** Every 15 minutes, classifies incoming messages (`Ignore` / `InfoOnly` / `ActionRequired`), drafts replies, and sends a 3-bullet summary to Telegram/Slack with interactive `[Approve Send]` buttons.
* **Residential Browser Automation:** Persistent Playwright server on home Wi-Fi that avoids datacenter IP blocks for price tracking, invoice downloads, and form automation.

### 3. Free Local "Background AI" Microservices — $0 API Cost (`src/ai_gateway.rs`)
* **4-Bit 7B–8B Local LLM (`qwen2.5:7b-instruct-q4_K_M`):** Runs in ~5GB unified RAM at ~40 tokens/sec with 5-minute idle auto-eviction (`OLLAMA_KEEP_ALIVE=300s`).
* **Unlimited Local Whisper (`local-whisper`):** Dedicated speech-to-text endpoint (`:8089`) and watched folder (`./data/voice-inbox` $\rightarrow$ `./data/transcripts`).
* **Always-On Local Embeddings (`nomic-embed-text:v1.5`):** Continuous private RAG indexing over PDFs, docs, and codebases queryable over your LAN.
* **High-Volume Data Classifier / Scraper Filter:** Scores 100% of scraped pages/logs locally for $0 and escalates only the top 1% (`score >= 95`) to frontier cloud models.

### 4. Remote Dev Environment From Anywhere (`src/remote_dev.rs`)
* **Tailscale SSH + `tmux` + VS Code Remote Tunnel:** Connect from your Windows PC, iPad, or phone to an always-awake dev environment with local port forwarding.
* **Reversible Clamshell & Battery Guardrails (`src/macos.rs`, `src/doctor.rs`):** Snapshots original `pmset` state to `.cheap-otto/state-backup.json`, disables AC lid-sleep, caps battery charge at **78%** to prevent swelling, and keeps 5-minute battery sleep if wall power is lost.

---

## Quickstart

### Build & Run with Cargo
```bash
# 1. Generate the reversible clamshell plan & docker-compose stack for a 16GB M1 Air
cargo run -- init --ram-gb 16 --battery-cap 78

# 2. Pair your Windows PC with the M1 appliance on your LAN or Tailscale mesh
cargo run -- connect --host 100.101.102.103 --name cheap-otto-m1 --ssh-user admin

# 3. Dispatch workloads from your workstation to the appliance across the 4 pillars
cargo run -- dispatch pr-fix --repo cokitron/cheap-otto --issue 42 --label agent-fix
cargo run -- dispatch canary --suite prod-checkout --url https://github.com/cokitron/cheap-otto
cargo run -- dispatch triage --sender founder@acme.dev --subject "Ship ARM64 runner today?"
cargo run -- dispatch scraper-filter --min-score 95

# 4. Run the 8-point appliance health check & test the RAM/Thermal Arbiter
cargo run -- doctor
cargo run -- arbiter --available-mb 6144 --ci-running --llm-loaded --llm-idle-secs 15

# 5. Revert your MacBook back to factory sleep behavior at any time
cargo run -- restore
```

### Run on your M1 MacBook Air over SSH
```bash
chmod +x scripts/bootstrap-macos.sh
./scripts/bootstrap-macos.sh
```
