//! Embedded Single-Page Mission Control Web UI (`GET /` on `cheap-otto serve`).
//!
//! Renders the unified control surface combining:
//! - Live M1 hardware/thermal/battery telemetry bar
//! - Pluggable `pixel-agents` (`v1.6.0`) Flat & Diorama visual office viewport
//! - Fleet list + 1-on-1 Direct Per-Agent Query & Diff Console (`/api/agents/:project/:id/ask`)
//! - Multi-Harness (`claude`, `agy`, `kiro`, `custom`) + Model + Effort switcher & Repeatable Templates
//! - Recurring `launchd` schedule manager + `history.jsonl` run ledger
//! - Interactive M1 `tmux` terminal console (`manual` shell + `otto-*` panes)

/// Self-contained HTML/CSS/JS single-page application served by `cheap-otto serve`.
pub const MISSION_CONTROL_HTML: &str = r####"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>cheap-otto · M1 Agent Office & Mission Control</title>
<style>
  :root {
    --bg: #0b0f17;
    --panel: #121826;
    --panel-alt: #182033;
    --border: #26324d;
    --text: #e6edf7;
    --muted: #8ea0c0;
    --accent: #58a6ff;
    --accent-hover: #79b8ff;
    --ok: #3fb950;
    --warn: #d29922;
    --err: #f85149;
    --purple: #bc8cff;
    --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    --sans: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: var(--sans);
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    height: 52px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    gap: 12px;
    flex-shrink: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    font-size: 15px;
    letter-spacing: -0.01em;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 600;
    background: var(--panel-alt);
    border: 1px solid var(--border);
    color: var(--muted);
  }
  .pill.ok { color: var(--ok); border-color: rgba(63,185,80,0.35); }
  .pill.warn { color: var(--warn); border-color: rgba(210,153,34,0.35); }
  .pill.err { color: var(--err); border-color: rgba(248,81,73,0.35); }
  .pill.purple { color: var(--purple); border-color: rgba(188,140,255,0.35); }
  .hud-pills { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .view-modes { display: flex; gap: 4px; background: var(--bg); padding: 3px; border-radius: 8px; border: 1px solid var(--border); }
  .view-modes button {
    background: transparent;
    color: var(--muted);
    border: none;
    padding: 5px 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .view-modes button.active { background: var(--panel-alt); color: var(--text); }
  main {
    flex: 1;
    display: grid;
    grid-template-columns: 1.15fr 1fr;
    min-height: 0;
  }
  main.mode-office { grid-template-columns: 1fr 0px; }
  main.mode-office .console-pane { display: none; }
  main.mode-console { grid-template-columns: 0px 1fr; }
  main.mode-console .office-pane { display: none; }
  .office-pane {
    position: relative;
    border-right: 1px solid var(--border);
    background: #070a10;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .office-bar {
    height: 36px;
    padding: 0 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    color: var(--muted);
  }
  .office-frame {
    flex: 1;
    width: 100%;
    border: none;
    background: #070a10;
  }
  .office-fallback {
    flex: 1;
    display: none;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    padding: 24px;
    text-align: center;
    color: var(--muted);
    gap: 10px;
  }
  .console-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg);
  }
  .tabs {
    display: flex;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    padding: 0 10px;
    gap: 4px;
    flex-shrink: 0;
  }
  .tabs button {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--muted);
    padding: 10px 12px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }
  .tab-body {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
    display: none;
    flex-direction: column;
    gap: 14px;
  }
  .tab-body.active { display: flex; }
  .card {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 13px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .card-title { font-size: 13.5px; font-weight: 700; }
  .muted { color: var(--muted); font-size: 12px; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: 11.5px; font-weight: 600; color: var(--muted); }
  input, select, textarea {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text);
    padding: 7px 10px;
    font-size: 12.5px;
    font-family: inherit;
  }
  textarea { font-family: var(--sans); resize: vertical; min-height: 68px; }
  input:focus, select:focus, textarea:focus { outline: 1px solid var(--accent); border-color: var(--accent); }
  .grid-2 { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .grid-3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px; }
  .btn-row { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; }
  button.btn {
    background: var(--panel-alt);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  button.btn:hover { border-color: var(--accent); }
  button.btn.primary { background: #1f6feb; border-color: #388bfd; color: #fff; }
  button.btn.primary:hover { background: #388bfd; }
  button.btn.danger { color: var(--err); border-color: rgba(248,81,73,0.4); }
  .agent-list { display: flex; flex-direction: column; gap: 8px; }
  .agent-row {
    background: var(--panel-alt);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .agent-row.selected { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  .pre-box {
    background: #070a10;
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 10px;
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 260px;
    overflow-y: auto;
    color: #d0d9e8;
  }
  .term-box {
    flex: 1;
    min-height: 320px;
    max-height: none;
    background: #05070c;
    color: #9ef0a8;
  }
  .toast {
    position: fixed;
    bottom: 14px;
    right: 14px;
    background: var(--panel-alt);
    border: 1px solid var(--accent);
    padding: 9px 14px;
    border-radius: 8px;
    font-size: 12.5px;
    display: none;
    z-index: 99;
  }
</style>
</head>
<body>
<header>
  <div class="brand">
    <span>🦀 cheap-otto</span>
    <span class="pill purple">M1 Mission Control</span>
  </div>
  <div class="hud-pills" id="hudPills">
    <span class="pill">Connecting to M1...</span>
  </div>
  <div class="view-modes">
    <button id="vmSplit" class="active" onclick="setViewMode('split')">Split Office + Console</button>
    <button id="vmOffice" onclick="setViewMode('office')">Full Pixel Office</button>
    <button id="vmConsole" onclick="setViewMode('console')">Pure Fleet Console</button>
  </div>
</header>

<main id="mainGrid">
  <section class="office-pane">
    <div class="office-bar">
      <span><strong>Visual Layer:</strong> <code>pixel-agents v1.6.0</code> (Flat &amp; Diorama Office — Purely Visual Viewport)</span>
      <div class="btn-row">
        <button class="btn" style="padding:3px 8px;font-size:11px;" onclick="reloadOfficeFrame()">Reload Office</button>
        <a id="openOfficeExt" href="#" target="_blank" rel="noopener" style="color:var(--accent);text-decoration:none;font-size:11.5px;">Open Tab ↗</a>
      </div>
    </div>
    <iframe id="officeFrame" class="office-frame" title="Pixel Agents Visual Office"></iframe>
    <div id="officeFallback" class="office-fallback">
      <div style="font-size:15px;font-weight:700;color:var(--text);">Pixel Office Visual Renderer Offline</div>
      <div>All agents, harnesses, models, direct queries, and schedules remain 100% operational in the Fleet Console.</div>
      <button class="btn primary" onclick="reloadOfficeFrame()">Retry Visual Connection</button>
    </div>
  </section>

  <section class="console-pane">
    <nav class="tabs">
      <button id="tabBtnFleet" class="active" onclick="switchTab('fleet')">🤖 Fleet &amp; Query Agent</button>
      <button id="tabBtnLaunch" onclick="switchTab('launch')">🚀 Launch &amp; Templates</button>
      <button id="tabBtnSched" onclick="switchTab('sched')">⏰ Schedules &amp; Runs</button>
      <button id="tabBtnTerm" onclick="switchTab('term')">💻 M1 Terminal</button>
    </nav>

    <!-- TAB 1: FLEET & DIRECT PER-AGENT QUERY -->
    <div id="tab-fleet" class="tab-body active">
      <div class="card">
        <div class="card-header">
          <div>
            <div class="card-title">Addressable Agent Fleet</div>
            <div class="muted">Select any agent to inspect its isolated Git worktree or query it directly on its own.</div>
          </div>
          <button class="btn" onclick="refreshAll()">↻ Refresh</button>
        </div>
        <div id="agentList" class="agent-list">
          <div class="muted">No worktree agents launched yet. Use "Launch &amp; Templates" to start one!</div>
        </div>
      </div>

      <div class="card" id="agentInspectorCard" style="display:none;">
        <div class="card-header">
          <div>
            <div class="card-title" id="inspTitle">Agent Inspector</div>
            <div class="muted" id="inspMeta"></div>
          </div>
          <div class="btn-row">
            <button class="btn" onclick="openAgentInTerminal()">Open tmux Pane</button>
            <button class="btn danger" onclick="dismissSelectedAgent()">Dismiss</button>
          </div>
        </div>

        <div>
          <div class="muted" style="margin-bottom:4px;font-weight:600;">Latest Agent Response / Output</div>
          <div id="inspResponse" class="pre-box">(waiting for output...)</div>
        </div>

        <div>
          <div class="muted" style="margin-bottom:4px;font-weight:600;">Worktree Git Status &amp; Diff</div>
          <div id="inspGit" class="pre-box">(clean worktree)</div>
        </div>

        <div style="border-top:1px solid var(--border);padding-top:10px;display:flex;flex-direction:column;gap:8px;">
          <div class="card-title">Ask / Instruct This Agent Directly</div>
          <div class="grid-3">
            <label>Harness Override
              <select id="askHarness" onchange="populateAskModels()">
                <option value="">Keep Agent's Harness</option>
                <option value="claude">Claude Code (claude)</option>
                <option value="agy">Antigravity (agy)</option>
                <option value="kiro">Kiro CLI (kiro)</option>
              </select>
            </label>
            <label>Model Override
              <input id="askModel" list="askModelList" placeholder="Keep current (or e.g. opus)" />
              <datalist id="askModelList"></datalist>
            </label>
            <label>Effort Override
              <select id="askEffort">
                <option value="">Keep current</option>
                <option value="low">low</option>
                <option value="medium">medium</option>
                <option value="high">high</option>
                <option value="xhigh">xhigh</option>
                <option value="max">max</option>
              </select>
            </label>
          </div>
          <label>Question or Follow-Up Coding Instruction
            <textarea id="askPrompt" placeholder="e.g. Why did test X fail? Switch to approach Y and run cargo test."></textarea>
          </label>
          <div class="btn-row">
            <button class="btn primary" id="askBtn" onclick="sendAgentQuery(false)">💬 Ask Synchronously (Get Reply)</button>
            <button class="btn" onclick="sendAgentQuery(true)">⚡ Dispatch Async in Worktree tmux</button>
          </div>
        </div>
      </div>
    </div>

    <!-- TAB 2: LAUNCH & REPEATABLE TEMPLATES -->
    <div id="tab-launch" class="tab-body">
      <div class="card">
        <div class="card-title">Multi-Harness Task &amp; Playbook Composer</div>
        <div class="muted">Switch freely between Claude Code, Antigravity, and Kiro—and pick exact models &amp; reasoning effort.</div>

        <div class="grid-2">
          <label>Project Workspace (in ~/otto-workspaces/)
            <input id="lnProject" list="projectList" value="cheapotto" />
            <datalist id="projectList"></datalist>
          </label>
          <label>Task / Agent ID (optional slug)
            <input id="lnId" placeholder="auto-generated from prompt" />
          </label>
        </div>

        <div class="grid-3">
          <label>AI App / Harness
            <select id="lnHarness" onchange="onHarnessChanged()">
              <option value="claude">Claude Code (claude)</option>
              <option value="agy">Antigravity (agy)</option>
              <option value="kiro">Kiro CLI (kiro)</option>
              <option value="custom">Custom Shell</option>
            </select>
          </label>
          <label>Model (select or type custom ID)
            <input id="lnModel" list="lnModelList" placeholder="sonnet" />
            <datalist id="lnModelList"></datalist>
          </label>
          <label>Reasoning Effort
            <select id="lnEffort">
              <option value="">Default</option>
              <option value="low">low</option>
              <option value="medium">medium</option>
              <option value="high">high</option>
              <option value="xhigh">xhigh</option>
              <option value="max">max</option>
            </select>
          </label>
        </div>
        <div id="harnessNote" class="muted"></div>

        <label>Task Prompt / Shell Command
          <textarea id="lnPrompt" placeholder="Describe the feature, bug fix, or audit to execute in an isolated M1 Git worktree..."></textarea>
        </label>

        <div class="grid-2">
          <label>Verification Gate Command
            <input id="lnVerify" value="cargo test" />
          </label>
          <label>Schedule Cadence (only if scheduling)
            <input id="lnCadence" placeholder="every:15m  or  daily:02:00  or  weekly:MON@03:00" />
          </label>
        </div>

        <div class="btn-row">
          <button class="btn primary" onclick="launchAgentNow()">▶ Launch Worktree Agent</button>
          <button class="btn" onclick="saveAsTemplate()">💾 Save as Repeatable Template</button>
          <button class="btn" onclick="scheduleFromComposer()">⏰ Register M1 launchd Schedule</button>
        </div>
      </div>

      <div class="card">
        <div class="card-header">
          <div>
            <div class="card-title">Saved Repeatable Templates</div>
            <div class="muted">Reusable across both this GUI and <code>cheap-otto template run --id &lt;name&gt;</code>.</div>
          </div>
        </div>
        <div id="templateList" class="agent-list">
          <div class="muted">No templates saved yet.</div>
        </div>
      </div>
    </div>

    <!-- TAB 3: SCHEDULES & RUN HISTORY -->
    <div id="tab-sched" class="tab-body">
      <div class="card">
        <div class="card-header">
          <div>
            <div class="card-title">Configured M1 launchd Schedules</div>
            <div class="muted">AC-power &amp; passive-thermal gated recurring CI and autonomous agent jobs.</div>
          </div>
          <button class="btn" onclick="loadSchedulesAndRuns()">↻ Refresh</button>
        </div>
        <div id="scheduleList" class="agent-list"></div>
      </div>

      <div class="card">
        <div class="card-title">Recent Execution Ledger (history.jsonl)</div>
        <div id="runsBox" class="pre-box">No scheduled runs recorded yet.</div>
      </div>
    </div>

    <!-- TAB 4: INTERACTIVE M1 TMUX TERMINAL -->
    <div id="tab-term" class="tab-body">
      <div class="card" style="flex:1;">
        <div class="card-header">
          <div class="btn-row">
            <span class="card-title">M1 tmux Session:</span>
            <select id="termSession" onchange="refreshTerminalPane()">
              <option value="manual">manual (Interactive M1 Shell)</option>
            </select>
          </div>
          <button class="btn" onclick="refreshTerminalPane()">↻ Capture</button>
        </div>
        <div id="termOutput" class="pre-box term-box">Loading tmux pane...</div>
        <div class="btn-row">
          <input id="termInput" style="flex:1;font-family:var(--mono);" placeholder="Type shell command to run inside selected M1 tmux session (Enter to send)..." onkeydown="if(event.key==='Enter')sendTerminalCommand()" />
          <button class="btn primary" onclick="sendTerminalCommand()">Send ↵</button>
          <button class="btn danger" onclick="sendTerminalCtrlC()">Ctrl+C</button>
        </div>
      </div>
    </div>
  </section>
</main>

<div id="toast" class="toast"></div>

<script>
let harnesses = [];
let agents = [];
let selectedKey = null; // "project/agent_id"
let pixelOfficeUrl = null;

function showToast(msg) {
  const el = document.getElementById('toast');
  el.textContent = msg;
  el.style.display = 'block';
  clearTimeout(window._toastTimer);
  window._toastTimer = setTimeout(() => { el.style.display = 'none'; }, 3600);
}

function setViewMode(mode) {
  const main = document.getElementById('mainGrid');
  main.className = mode === 'office' ? 'mode-office' : (mode === 'console' ? 'mode-console' : '');
  document.getElementById('vmSplit').classList.toggle('active', mode === 'split');
  document.getElementById('vmOffice').classList.toggle('active', mode === 'office');
  document.getElementById('vmConsole').classList.toggle('active', mode === 'console');
}

function switchTab(name) {
  for (const id of ['fleet', 'launch', 'sched', 'term']) {
    document.getElementById('tab-' + id).classList.toggle('active', id === name);
  }
  document.getElementById('tabBtnFleet').classList.toggle('active', name === 'fleet');
  document.getElementById('tabBtnLaunch').classList.toggle('active', name === 'launch');
  document.getElementById('tabBtnSched').classList.toggle('active', name === 'sched');
  document.getElementById('tabBtnTerm').classList.toggle('active', name === 'term');
  if (name === 'term') refreshTerminalPane();
  if (name === 'sched') loadSchedulesAndRuns();
}

async function api(path, opts) {
  const res = await fetch(path, Object.assign({ headers: { 'Content-Type': 'application/json' } }, opts || {}));
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || ('HTTP ' + res.status));
  return data;
}

async function loadPixelOffice() {
  try {
    const info = await api('/api/pixel-office');
    if (info.available && info.pixel_port) {
      const host = window.location.hostname || 'localhost';
      const tok = info.token ? ('?token=' + encodeURIComponent(info.token)) : '';
      pixelOfficeUrl = 'http://' + host + ':' + info.pixel_port + '/' + tok;
      document.getElementById('officeFrame').src = pixelOfficeUrl;
      document.getElementById('officeFrame').style.display = 'block';
      document.getElementById('officeFallback').style.display = 'none';
      document.getElementById('openOfficeExt').href = pixelOfficeUrl;
    } else {
      document.getElementById('officeFrame').style.display = 'none';
      document.getElementById('officeFallback').style.display = 'flex';
    }
  } catch (_) {
    document.getElementById('officeFrame').style.display = 'none';
    document.getElementById('officeFallback').style.display = 'flex';
  }
}

function reloadOfficeFrame() { loadPixelOffice(); }

async function loadHealth() {
  try {
    const h = await api('/api/health');
    const pills = [];
    pills.push(`<span class="pill ${h.on_ac_power ? 'ok' : 'err'}">${h.on_ac_power ? '⚡ AC Power' : '🔋 On Battery'}</span>`);
    pills.push(`<span class="pill ${h.battery_ceiling_percent <= 80 ? 'ok' : 'warn'}">🛡 Cap ${h.battery_ceiling_percent}%</span>`);
    pills.push(`<span class="pill ok">🌡 ${h.thermal_level}</span>`);
    pills.push(`<span class="pill">🧠 ${h.available_memory_mb} MB Free</span>`);
    pills.push(`<span class="pill">${agents.length} Agents</span>`);
    document.getElementById('hudPills').innerHTML = pills.join('');
  } catch (e) {
    document.getElementById('hudPills').innerHTML = `<span class="pill err">API Error</span>`;
  }
}

async function loadHarnessesAndProjects() {
  try {
    const hData = await api('/api/harnesses');
    harnesses = hData.harnesses || [];
    onHarnessChanged();
    populateAskModels();
  } catch (_) {}

  try {
    const pData = await api('/api/projects');
    const dl = document.getElementById('projectList');
    dl.innerHTML = '';
    for (const p of (pData.projects || [])) {
      const opt = document.createElement('option');
      opt.value = p;
      dl.appendChild(opt);
    }
    if (pData.projects && pData.projects.length > 0 && !document.getElementById('lnProject').value) {
      document.getElementById('lnProject').value = pData.projects[0];
    }
  } catch (_) {}
}

function onHarnessChanged() {
  const id = document.getElementById('lnHarness').value;
  const entry = harnesses.find(h => h.id === id);
  const dl = document.getElementById('lnModelList');
  dl.innerHTML = '';
  const note = document.getElementById('harnessNote');
  if (entry) {
    for (const m of (entry.models || [])) {
      const opt = document.createElement('option');
      opt.value = m.id;
      opt.label = m.label;
      dl.appendChild(opt);
    }
    if (entry.models && entry.models.length > 0) {
      document.getElementById('lnModel').placeholder = entry.models[0].id;
    }
    note.textContent = entry.status_note || (entry.installed ? `Ready (${entry.version || entry.id})` : 'Not installed on M1');
  } else {
    note.textContent = '';
  }
}

function populateAskModels() {
  const overrideH = document.getElementById('askHarness').value;
  const currentAgent = agents.find(a => (a.project_name + '/' + a.agent_id) === selectedKey);
  const targetId = overrideH || (currentAgent ? currentAgent.harness_slug : 'claude');
  const entry = harnesses.find(h => h.id === targetId);
  const dl = document.getElementById('askModelList');
  dl.innerHTML = '';
  if (entry) {
    for (const m of (entry.models || [])) {
      const opt = document.createElement('option');
      opt.value = m.id;
      opt.label = m.label;
      dl.appendChild(opt);
    }
  }
}

async function loadAgents() {
  try {
    const data = await api('/api/agents');
    agents = data.agents || [];
    const container = document.getElementById('agentList');
    container.innerHTML = '';
    const termSel = document.getElementById('termSession');
    const prevTerm = termSel.value || 'manual';
    termSel.innerHTML = '<option value="manual">manual (Interactive M1 Shell)</option>';

    if (agents.length === 0) {
      container.innerHTML = '<div class="muted">No worktree agents launched yet. Use "Launch &amp; Templates" to start one!</div>';
    }
    for (const a of agents) {
      const key = a.project_name + '/' + a.agent_id;
      const row = document.createElement('div');
      row.className = 'agent-row' + (selectedKey === key ? ' selected' : '');
      row.onclick = () => selectAgent(a.project_name, a.agent_id);

      const statusClass = a.status === 'active' ? 'warn' : (a.status === 'failed' ? 'err' : 'ok');
      const modelBadge = [a.harness_slug, a.model, a.effort].filter(Boolean).join(' · ');
      row.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;">
          <strong>${escapeHtml(a.project_name)} / ${escapeHtml(a.agent_id)}</strong>
          <div style="display:flex;gap:6px;align-items:center;">
            <span class="pill purple">${escapeHtml(modelBadge)}</span>
            <span class="pill ${statusClass}">${escapeHtml(a.status)}</span>
          </div>
        </div>
        <div class="muted">${escapeHtml(a.current_activity || a.branch_name)} · turns: ${(a.turns || []).length}</div>
      `;
      container.appendChild(row);

      const tOpt = document.createElement('option');
      tOpt.value = a.tmux_session_name;
      tOpt.textContent = `${a.tmux_session_name} (${modelBadge})`;
      termSel.appendChild(tOpt);
    }
    if ([...termSel.options].some(o => o.value === prevTerm)) termSel.value = prevTerm;
    if (selectedKey) {
      const [p, id] = selectedKey.split('/');
      await refreshSelectedInspector(p, id);
    }
  } catch (_) {}
}

async function selectAgent(project, agentId) {
  selectedKey = project + '/' + agentId;
  populateAskModels();
  await loadAgents();
}

async function refreshSelectedInspector(project, agentId) {
  try {
    const detail = await api(`/api/agents/${encodeURIComponent(project)}/${encodeURIComponent(agentId)}`);
    const card = document.getElementById('agentInspectorCard');
    card.style.display = 'flex';
    const rec = detail.record;
    const badge = [detail.harness_slug, rec.model, rec.effort].filter(Boolean).join(' · ');
    document.getElementById('inspTitle').textContent = `${rec.project_name} / ${rec.agent_id} (${badge})`;
    document.getElementById('inspMeta').textContent = `Branch: ${rec.branch_name}  |  Session: ${rec.provider_session_id || 'auto'}  |  Worktree: ${rec.remote_worktree_dir}`;
    document.getElementById('inspResponse').textContent = detail.latest_response || rec.last_response || detail.log_tail || '(no output yet)';
    document.getElementById('inspGit').textContent = detail.git_status || '(clean)';
  } catch (_) {
    document.getElementById('agentInspectorCard').style.display = 'none';
  }
}

async function sendAgentQuery(runAsync) {
  if (!selectedKey) return;
  const [project, agentId] = selectedKey.split('/');
  const prompt = document.getElementById('askPrompt').value.trim();
  if (!prompt) return showToast('Enter a question or follow-up instruction first.');
  const btn = document.getElementById('askBtn');
  btn.disabled = true;
  btn.textContent = runAsync ? 'Dispatching...' : 'Querying Agent...';
  try {
    const body = {
      prompt,
      harness: document.getElementById('askHarness').value || null,
      model: document.getElementById('askModel').value.trim() || null,
      effort: document.getElementById('askEffort').value || null,
      run_async: runAsync
    };
    const res = await api(`/api/agents/${encodeURIComponent(project)}/${encodeURIComponent(agentId)}/ask`, {
      method: 'POST',
      body: JSON.stringify(body)
    });
    document.getElementById('askPrompt').value = '';
    showToast(runAsync ? `Dispatched async turn to ${agentId}` : `Received reply from ${agentId}`);
    if (res.response) document.getElementById('inspResponse').textContent = res.response;
    await loadAgents();
  } catch (e) {
    showToast('Query error: ' + e.message);
  } finally {
    btn.disabled = false;
    btn.textContent = '💬 Ask Synchronously (Get Reply)';
  }
}

async function launchAgentNow() {
  const project = document.getElementById('lnProject').value.trim() || 'cheapotto';
  const prompt = document.getElementById('lnPrompt').value.trim();
  if (!prompt) return showToast('Enter a task prompt first.');
  try {
    const res = await api('/api/agents', {
      method: 'POST',
      body: JSON.stringify({
        project_name: project,
        task_id: document.getElementById('lnId').value.trim() || null,
        harness: document.getElementById('lnHarness').value,
        model: document.getElementById('lnModel').value.trim() || null,
        effort: document.getElementById('lnEffort').value || null,
        prompt,
        verify_command: document.getElementById('lnVerify').value.trim() || 'cargo test'
      })
    });
    showToast(`Launched ${res.agent_id} (${res.harness_slug})`);
    selectedKey = project + '/' + res.agent_id;
    switchTab('fleet');
    await loadAgents();
  } catch (e) {
    showToast('Launch failed: ' + e.message);
  }
}

async function saveAsTemplate() {
  const prompt = document.getElementById('lnPrompt').value.trim();
  const id = document.getElementById('lnId').value.trim();
  if (!id || !prompt) return showToast('Provide a Task/Template ID and Prompt to save a template.');
  try {
    await api('/api/templates', {
      method: 'POST',
      body: JSON.stringify({
        template_id: id,
        title: id,
        project_name: document.getElementById('lnProject').value.trim() || null,
        harness: document.getElementById('lnHarness').value,
        model: document.getElementById('lnModel').value.trim() || null,
        effort: document.getElementById('lnEffort').value || null,
        prompt,
        verify_command: document.getElementById('lnVerify').value.trim() || 'cargo test',
        create_draft_pr: false
      })
    });
    showToast(`Saved repeatable template '${id}'`);
    await loadTemplates();
  } catch (e) {
    showToast('Save failed: ' + e.message);
  }
}

async function loadTemplates() {
  try {
    const data = await api('/api/templates');
    const list = document.getElementById('templateList');
    list.innerHTML = '';
    const tpls = data.templates || [];
    if (tpls.length === 0) {
      list.innerHTML = '<div class="muted">No templates saved yet.</div>';
      return;
    }
    for (const t of tpls) {
      const badge = [t.harness_slug, t.model, t.effort].filter(Boolean).join(' · ');
      const el = document.createElement('div');
      el.className = 'agent-row';
      el.style.cursor = 'default';
      el.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:center;">
          <strong>${escapeHtml(t.template_id)}</strong>
          <span class="pill purple">${escapeHtml(badge)}</span>
        </div>
        <div class="muted">${escapeHtml(t.prompt)}</div>
        <div class="btn-row" style="margin-top:4px;">
          <button class="btn primary" style="padding:4px 9px;font-size:11px;" onclick="runTemplate('${escapeAttr(t.template_id)}')">▶ Run Template</button>
          <button class="btn danger" style="padding:4px 9px;font-size:11px;" onclick="deleteTemplate('${escapeAttr(t.template_id)}')">Remove</button>
        </div>
      `;
      list.appendChild(el);
    }
  } catch (_) {}
}

async function runTemplate(id) {
  const project = document.getElementById('lnProject').value.trim() || 'cheapotto';
  try {
    const res = await api(`/api/templates/${encodeURIComponent(id)}/run`, {
      method: 'POST',
      body: JSON.stringify({ project_name: project })
    });
    showToast(`Started '${res.agent_id}' from template '${id}'`);
    selectedKey = res.project_name + '/' + res.agent_id;
    switchTab('fleet');
    await loadAgents();
  } catch (e) {
    showToast('Template run failed: ' + e.message);
  }
}

async function deleteTemplate(id) {
  await api(`/api/templates/${encodeURIComponent(id)}`, { method: 'DELETE' });
  await loadTemplates();
}

async function scheduleFromComposer() {
  const project = document.getElementById('lnProject').value.trim() || 'cheapotto';
  const id = document.getElementById('lnId').value.trim();
  const cadenceRaw = document.getElementById('lnCadence').value.trim();
  const prompt = document.getElementById('lnPrompt').value.trim();
  if (!id || !cadenceRaw || !prompt) {
    return showToast('Provide Task ID, Prompt, and Schedule Cadence (e.g. every:15m or daily:02:00).');
  }
  try {
    await api('/api/schedules', {
      method: 'POST',
      body: JSON.stringify({
        project_name: project,
        job_id: id,
        cadence: cadenceRaw,
        harness: document.getElementById('lnHarness').value,
        model: document.getElementById('lnModel').value.trim() || null,
        effort: document.getElementById('lnEffort').value || null,
        prompt,
        verify_command: document.getElementById('lnVerify').value.trim() || 'cargo test'
      })
    });
    showToast(`Registered M1 launchd schedule '${id}'`);
    switchTab('sched');
  } catch (e) {
    showToast('Schedule failed: ' + e.message);
  }
}

async function loadSchedulesAndRuns() {
  try {
    const sData = await api('/api/schedules');
    const list = document.getElementById('scheduleList');
    list.innerHTML = '';
    const jobs = sData.jobs || [];
    if (jobs.length === 0) {
      list.innerHTML = '<div class="muted">No launchd schedules configured yet.</div>';
    }
    for (const j of jobs) {
      const el = document.createElement('div');
      el.className = 'agent-row';
      el.style.cursor = 'default';
      el.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:center;">
          <strong>${escapeHtml(j.project_name)} / ${escapeHtml(j.job_id)}</strong>
          <span class="pill ok">${escapeHtml(j.cadence_summary)}</span>
        </div>
        <div class="muted">${escapeHtml(j.launchd_label)}</div>
        <div class="btn-row" style="margin-top:4px;">
          <button class="btn primary" style="padding:4px 9px;font-size:11px;" onclick="triggerScheduleNow('${escapeAttr(j.project_name)}','${escapeAttr(j.job_id)}')">⚡ Run Now</button>
          <button class="btn danger" style="padding:4px 9px;font-size:11px;" onclick="removeSchedule('${escapeAttr(j.project_name)}','${escapeAttr(j.job_id)}')">Unload &amp; Remove</button>
        </div>
      `;
      list.appendChild(el);
    }

    const rData = await api('/api/runs');
    const records = rData.records || [];
    document.getElementById('runsBox').textContent = records.length === 0
      ? 'No scheduled runs recorded yet.'
      : records.map(r => `[${r.status.toUpperCase()}] ${r.project}/${r.job_id} (${r.kind}) sha=${r.commit_sha} dur=${r.duration_secs}s`).join('\n');
  } catch (_) {}
}

async function triggerScheduleNow(project, id) {
  await api(`/api/schedules/${encodeURIComponent(project)}/${encodeURIComponent(id)}/run-now`, { method: 'POST' });
  showToast(`Triggered '${id}' via launchctl kickstart`);
  setTimeout(loadSchedulesAndRuns, 2000);
}

async function removeSchedule(project, id) {
  await api(`/api/schedules/${encodeURIComponent(project)}/${encodeURIComponent(id)}`, { method: 'DELETE' });
  showToast(`Unloaded schedule '${id}'`);
  await loadSchedulesAndRuns();
}

function openAgentInTerminal() {
  if (!selectedKey) return;
  const [, id] = selectedKey.split('/');
  document.getElementById('termSession').value = 'otto-' + id;
  switchTab('term');
}

async function dismissSelectedAgent() {
  if (!selectedKey) return;
  const [project, id] = selectedKey.split('/');
  await api(`/api/agents/${encodeURIComponent(project)}/${encodeURIComponent(id)}`, { method: 'DELETE' });
  selectedKey = null;
  document.getElementById('agentInspectorCard').style.display = 'none';
  showToast(`Dismissed agent '${id}'`);
  await loadAgents();
}

async function refreshTerminalPane() {
  const sess = document.getElementById('termSession').value || 'manual';
  try {
    const data = await api(`/api/terminal/${encodeURIComponent(sess)}`);
    const box = document.getElementById('termOutput');
    box.textContent = data.output || '(empty pane)';
    box.scrollTop = box.scrollHeight;
  } catch (e) {
    document.getElementById('termOutput').textContent = 'Session unavailable: ' + e.message;
  }
}

async function sendTerminalCommand() {
  const sess = document.getElementById('termSession').value || 'manual';
  const input = document.getElementById('termInput');
  const cmd = input.value;
  input.value = '';
  await api(`/api/terminal/${encodeURIComponent(sess)}`, {
    method: 'POST',
    body: JSON.stringify({ keys: cmd, enter: true })
  });
  setTimeout(refreshTerminalPane, 350);
}

async function sendTerminalCtrlC() {
  const sess = document.getElementById('termSession').value || 'manual';
  await api(`/api/terminal/${encodeURIComponent(sess)}`, {
    method: 'POST',
    body: JSON.stringify({ keys: 'C-c', enter: false })
  });
  setTimeout(refreshTerminalPane, 300);
}

function escapeHtml(s) {
  return String(s || '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
}
function escapeAttr(s) { return escapeHtml(s); }

async function refreshAll() {
  await Promise.all([loadAgents(), loadHealth(), loadTemplates()]);
  if (document.getElementById('tab-term').classList.contains('active')) {
    await refreshTerminalPane();
  }
}

(async function boot() {
  await loadPixelOffice();
  await loadHarnessesAndProjects();
  await refreshAll();
  setInterval(refreshAll, 3000);
})();
</script>
</body>
</html>
"####;
