//! Embedded single-binary dark-mode Web Dashboard HTML/CSS/JS for `code-reviewer`.

pub const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Internal CI — Multi-Lens Code Reviewer (otto-mac M1)</title>
  <style>
    :root {
      --bg: #090b10;
      --panel: #11151f;
      --panel-2: #161b28;
      --border: #232b3e;
      --text: #e6edf7;
      --muted: #8c99b3;
      --accent: #6366f1;
      --accent-hover: #818cf8;
      --green: #10b981;
      --amber: #f59e0b;
      --red: #ef4444;
      --cyan: #06b6d4;
      --purple: #a855f7;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Inter, Roboto, sans-serif;
      background: var(--bg);
      color: var(--text);
      font-size: 14px;
      line-height: 1.5;
    }
    a { color: #93c5fd; text-decoration: none; }
    a:hover { text-decoration: underline; }
    header {
      position: sticky;
      top: 0;
      z-index: 30;
      background: rgba(9, 11, 16, 0.92);
      backdrop-filter: blur(10px);
      border-bottom: 1px solid var(--border);
      padding: 12px 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 16px;
      flex-wrap: wrap;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .brand-logo {
      width: 34px;
      height: 34px;
      border-radius: 9px;
      background: linear-gradient(135deg, #6366f1, #06b6d4);
      display: grid;
      place-items: center;
      font-weight: 800;
      color: #fff;
      font-size: 15px;
    }
    .brand h1 {
      margin: 0;
      font-size: 16px;
      font-weight: 700;
      letter-spacing: -0.01em;
    }
    .brand .sub {
      font-size: 12px;
      color: var(--muted);
    }
    .header-right {
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;
    }
    .pill {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 4px 10px;
      border-radius: 999px;
      font-size: 12px;
      font-weight: 600;
      background: var(--panel-2);
      border: 1px solid var(--border);
      color: var(--text);
    }
    .dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
    .dot.ok { background: var(--green); box-shadow: 0 0 8px var(--green); }
    .dot.warn { background: var(--amber); }
    .dot.run { background: var(--cyan); animation: pulse 1.3s infinite; }
    @keyframes pulse {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.45; transform: scale(1.25); }
    }
    button, .btn {
      background: var(--accent);
      color: #fff;
      border: 1px solid transparent;
      border-radius: 7px;
      padding: 7px 13px;
      font-size: 12.5px;
      font-weight: 600;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 6px;
    }
    button:hover, .btn:hover { background: var(--accent-hover); text-decoration: none; }
    button.secondary, .btn.secondary {
      background: var(--panel-2);
      border-color: var(--border);
      color: var(--text);
    }
    button.secondary:hover, .btn.secondary:hover { border-color: #4b5563; background: #1e2536; }
    button.danger { background: rgba(239,68,68,0.16); border-color: rgba(239,68,68,0.45); color: #fca5a5; }
    nav.tabs {
      display: flex;
      gap: 4px;
      padding: 10px 22px 0;
      border-bottom: 1px solid var(--border);
      background: var(--panel);
      overflow-x: auto;
    }
    nav.tabs button {
      background: transparent;
      border: none;
      border-bottom: 2px solid transparent;
      border-radius: 0;
      padding: 10px 14px;
      color: var(--muted);
      font-weight: 600;
    }
    nav.tabs button.active {
      color: #fff;
      border-bottom-color: var(--accent);
    }
    main { max-width: 1460px; margin: 0 auto; padding: 20px 22px 48px; }
    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(185px, 1fr));
      gap: 12px;
      margin-bottom: 18px;
    }
    .card {
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 11px;
      padding: 16px;
    }
    .kpi-label { font-size: 12px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
    .kpi-val { font-size: 26px; font-weight: 800; margin-top: 4px; }
    .kpi-sub { font-size: 12px; color: var(--muted); margin-top: 2px; }
    .grid-2 { display: grid; grid-template-columns: 1.35fr 1fr; gap: 16px; }
    .split-pr { display: grid; grid-template-columns: 360px 1fr; gap: 16px; align-items: start; }
    @media (max-width: 980px) { .grid-2, .split-pr { grid-template-columns: 1fr; } }
    h2 { font-size: 15px; margin: 0 0 12px; display: flex; align-items: center; justify-content: space-between; }
    h3 { font-size: 14px; margin: 0 0 8px; }
    .badge {
      display: inline-block;
      padding: 2px 8px;
      border-radius: 6px;
      font-size: 11px;
      font-weight: 700;
      letter-spacing: 0.02em;
    }
    .b-pass, .b-done, .b-low { background: rgba(16,185,129,0.16); color: #6ee7b7; border: 1px solid rgba(16,185,129,0.35); }
    .b-changes, .b-critical, .b-high { background: rgba(239,68,68,0.16); color: #fca5a5; border: 1px solid rgba(239,68,68,0.4); }
    .b-medium, .b-running { background: rgba(245,158,11,0.16); color: #fcd34d; border: 1px solid rgba(245,158,11,0.4); }
    .b-claude { background: rgba(249,115,22,0.16); color: #fdba74; border: 1px solid rgba(249,115,22,0.4); }
    .b-agy { background: rgba(6,182,212,0.16); color: #67e8f9; border: 1px solid rgba(6,182,212,0.4); }
    .b-muted { background: #1a2130; color: var(--muted); border: 1px solid var(--border); }
    .worker-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(390px, 1fr)); gap: 12px; }
    .worker-card {
      background: var(--panel-2);
      border: 1px solid #2e3a56;
      border-radius: 10px;
      padding: 14px;
    }
    .brain-chips {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 6px;
      margin-top: 10px;
    }
    .brain-chip {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 6px;
      padding: 5px 8px;
      border-radius: 6px;
      background: var(--bg);
      border: 1px solid var(--border);
      font-size: 11.5px;
    }
    .brain-chip.running { border-color: var(--cyan); box-shadow: inset 0 0 0 1px rgba(6,182,212,0.35); }
    .brain-chip.done { border-color: rgba(16,185,129,0.45); }
    .pr-item {
      padding: 12px;
      border-radius: 9px;
      border: 1px solid var(--border);
      background: var(--panel-2);
      margin-bottom: 8px;
      cursor: pointer;
    }
    .pr-item:hover, .pr-item.selected { border-color: var(--accent); background: #1a2236; }
    .obs-card {
      background: var(--panel-2);
      border: 1px solid var(--border);
      border-radius: 9px;
      padding: 14px;
      margin-bottom: 10px;
    }
    pre {
      background: #07090e;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 12px;
      overflow-x: auto;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 12px;
      color: #dbeafe;
      margin: 8px 0 0;
      max-height: 360px;
    }
    input, select, textarea {
      background: #0b0e16;
      border: 1px solid var(--border);
      color: var(--text);
      border-radius: 7px;
      padding: 7px 10px;
      font-size: 13px;
      width: 100%;
    }
    label { display: block; font-size: 12px; color: var(--muted); margin-bottom: 4px; font-weight: 600; }
    .form-row { display: grid; grid-template-columns: repeat(auto-fit, minmax(170px, 1fr)); gap: 10px; margin-bottom: 10px; }
    table { width: 100%; border-collapse: collapse; font-size: 12.5px; }
    th, td { text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--border); vertical-align: top; }
    th { color: var(--muted); font-weight: 600; }
    .toast {
      position: fixed;
      bottom: 20px;
      right: 20px;
      background: #1e293b;
      border: 1px solid var(--accent);
      color: #fff;
      padding: 10px 16px;
      border-radius: 9px;
      font-size: 13px;
      z-index: 99;
      display: none;
    }
    .hidden { display: none !important; }
    .filter-bar { display: flex; gap: 6px; flex-wrap: wrap; margin: 10px 0 12px; }
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="brand-logo">CR</div>
      <div>
        <h1>Internal CI — Multi-Lens Code Reviewer</h1>
        <div class="sub">Parallel Async Sessions • Claude ⇄ Antigravity Load Intercalation • Living Docs</div>
      </div>
    </div>
    <div class="header-right">
      <span class="pill" id="pill-poll-mode"><span class="dot ok"></span>Poller: Pure GraphQL (0 AI)</span>
      <span class="pill" id="pill-claude"><span class="dot"></span>claude</span>
      <span class="pill" id="pill-agy"><span class="dot"></span>agy</span>
      <span class="pill" id="pill-intercalate">⇄ Intercalate</span>
      <span class="pill" id="pill-power">M1 Host</span>
      <button onclick="triggerPollNow()">⚡ Poll &amp; Dispatch Parallel Batch</button>
      <button class="secondary" onclick="refreshAll()">↻ Refresh</button>
    </div>
  </header>

  <nav class="tabs">
    <button class="active" data-tab="live" onclick="switchTab('live')">1. Live Parallel Sessions</button>
    <button data-tab="prs" onclick="switchTab('prs')">2. Pull Requests &amp; Observations</button>
    <button data-tab="repos" onclick="switchTab('repos')">3. Watched Repos</button>
    <button data-tab="brains" onclick="switchTab('brains')">4. Brains &amp; Knowledge Bases (8)</button>
    <button data-tab="docs" onclick="switchTab('docs')">5. Living Documentation</button>
  </nav>

  <main>
    <!-- TAB 1: LIVE -->
    <section id="tab-live">
      <div class="kpis" id="kpi-strip"></div>

      <div class="card" style="margin-bottom:16px;">
        <h2>
          <span>Active Parallel Async PR Sessions</span>
          <span class="sub" id="parallel-cap-label" style="font-size:12px;color:var(--muted);"></span>
        </h2>
        <div id="active-workers-box"></div>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2><span>Live Poller &amp; Worker Stream</span><span class="badge b-muted">auto-updating 2.5s</span></h2>
          <pre id="live-logs-box">Loading logs...</pre>
        </div>

        <div class="card">
          <h2><span>Fast Models, Load-Splitting &amp; Full Parallelism</span></h2>
          <div class="form-row">
            <div>
              <label>Claude Fast Model</label>
              <select id="cfg-claude-model">
                <option value="sonnet">sonnet (Fast — Recommended)</option>
                <option value="haiku">haiku (Ultra-Fast)</option>
                <option value="opus">opus (Deep / Slower)</option>
              </select>
            </div>
            <div>
              <label>Reasoning Effort (`claude` + `agy`)</label>
              <select id="cfg-effort">
                <option value="low">low (Fastest)</option>
                <option value="medium">medium (Balanced Speed/Depth)</option>
                <option value="high">high</option>
              </select>
            </div>
            <div>
              <label>Intercalate (`claude` ⇄ `agy`)</label>
              <select id="cfg-intercalate">
                <option value="true">Enabled (50/50 + failover)</option>
                <option value="false">Fixed Primary / Verifier</option>
              </select>
            </div>
            <div>
              <label>Max Parallel PR Sessions (`async`)</label>
              <input type="number" id="cfg-max-prs" min="1" max="12" value="8" />
            </div>
            <div>
              <label>Parallel Brains per PR (`8` = All at Once)</label>
              <input type="number" id="cfg-max-brains" min="1" max="8" value="8" />
            </div>
            <div>
              <label>2m Poller Token Guard</label>
              <select id="cfg-poll-only">
                <option value="false">Event-Only AI (0 AI on idle GraphQL polls)</option>
                <option value="true">Strict Poll-Only (Pause all Auto-AI)</option>
              </select>
            </div>
          </div>
          <button onclick="saveAgentSettings()">Save Fast Model &amp; Parallel Settings</button>

          <h3 style="margin-top:18px;">Recent 2m GraphQL Polls <span class="badge b-pass">0 AI Tokens / Poll</span></h3>
          <div style="max-height:210px;overflow-y:auto;">
            <table>
              <thead><tr><th>Tick</th><th>Engine</th><th>Duration</th><th>Started (UTC)</th></tr></thead>
              <tbody id="history-tbody"></tbody>
            </table>
          </div>
        </div>
      </div>
    </section>

    <!-- TAB 2: PULL REQUESTS -->
    <section id="tab-prs" class="hidden">
      <div class="split-pr">
        <div class="card">
          <h2><span>Tracked Pull Requests</span><span id="pr-count-badge" class="badge b-muted">0</span></h2>
          <div id="pr-list-box"></div>
        </div>
        <div class="card" id="pr-detail-box">
          <div style="color:var(--muted);">Select a Pull Request on the left to inspect open/resolved observations, traceability matrix, Google Drive link, or launch an async re-review.</div>
        </div>
      </div>
    </section>

    <!-- TAB 3: WATCHED REPOS -->
    <section id="tab-repos" class="hidden">
      <div class="grid-2">
        <div class="card">
          <h2>Watched Repositories (`config/repos.toml`)</h2>
          <table>
            <thead><tr><th>Repository</th><th>Branch</th><th>Identity Policy</th><th>Review Event</th><th>Status</th><th>Actions</th></tr></thead>
            <tbody id="repos-tbody"></tbody>
          </table>
        </div>
        <div class="card">
          <h2>Add / Update Watched Repository</h2>
          <div class="form-row">
            <div><label>Owner / Repo</label><input id="r-name" placeholder="acme-org/web-app" /></div>
            <div><label>Default Branch</label><input id="r-branch" value="main" /></div>
          </div>
          <div class="form-row">
            <div>
              <label>Account Policy</label>
              <select id="r-acct-pol">
                <option value="swap">swap (primary ⇄ secondary)</option>
                <option value="forced">forced (single account)</option>
              </select>
            </div>
            <div><label>Forced Account (if forced)</label><input id="r-forced" placeholder="alice-dev" /></div>
          </div>
          <div class="form-row">
            <div>
              <label>GitHub Event Policy</label>
              <select id="r-ev-pol">
                <option value="formal_verdict">formal_verdict (APPROVE / REQUEST_CHANGES)</option>
                <option value="comment_only">comment_only (COMMENT + inline)</option>
              </select>
            </div>
            <div><label>Google Drive Folder ID (optional)</label><input id="r-drive" placeholder="1AbCdEfGhIj..." /></div>
          </div>
          <div class="form-row">
            <div><label>Local Check Command (optional)</label><input id="r-check" placeholder="npm run check" /></div>
          </div>
          <button onclick="addWatchedRepo()">Save Watched Repository</button>
        </div>
      </div>
    </section>

    <!-- TAB 4: BRAINS & KBS -->
    <section id="tab-brains" class="hidden">
      <div class="split-pr">
        <div class="card">
          <h2>Review Brains (01..08)</h2>
          <div id="brains-list-box"></div>
        </div>
        <div class="card">
          <h2 id="brain-detail-title">Brain Details &amp; Isolated Knowledge Base</h2>
          <div class="card" style="background:var(--panel-2);margin-bottom:14px;">
            <h3>Append Rule to This Brain's Knowledge Base</h3>
            <div class="form-row">
              <div>
                <label>Entry Kind</label>
                <select id="kb-kind">
                  <option value="lesson">Active Lesson (`lessons.md`)</option>
                  <option value="false-positive">False-Positive Guardrail (`false-positives.md`)</option>
                  <option value="exemplar">Repo Exemplar (`exemplars.md`)</option>
                </select>
              </div>
              <div style="grid-column: span 2;">
                <label>Imperative Rule (include `[Owner/repo]` or `[*]`)</label>
                <input id="kb-rule" placeholder="[acme-org/web-app] Verify BFF routes preserve HttpOnly + SameSite cookie flags." />
              </div>
            </div>
            <button onclick="submitKbRule()">+ Append to Brain KB</button>
          </div>
          <div id="brain-files-box"></div>
        </div>
      </div>
    </section>

    <!-- TAB 5: LIVING DOCS -->
    <section id="tab-docs" class="hidden">
      <div class="card">
        <h2>
          <span>Living Repository Context (`living-docs/`)</span>
          <div style="display:flex;gap:8px;align-items:center;">
            <select id="docs-repo-select" onchange="loadLivingDocs()" style="width:260px;"></select>
            <button class="secondary" onclick="syncLivingDocsDrive()">☁ Sync Living Docs to Google Drive</button>
          </div>
        </h2>
        <div id="living-docs-box"></div>
      </div>
    </section>
  </main>

  <div class="toast" id="toast"></div>

  <script>
    let STATE = null;
    let SELECTED_PR_KEY = null;
    let OBS_FILTER = 'all';
    let SELECTED_BRAIN = '01-intent-traceability';
    let SETTINGS_INITIALIZED = false;

    function showToast(msg) {
      const el = document.getElementById('toast');
      el.textContent = msg;
      el.style.display = 'block';
      clearTimeout(window.__toastTimer);
      window.__toastTimer = setTimeout(() => { el.style.display = 'none'; }, 3600);
    }

    function esc(s) {
      return String(s ?? '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }

    function switchTab(tab) {
      document.querySelectorAll('nav.tabs button').forEach(b => {
        b.classList.toggle('active', b.dataset.tab === tab);
      });
      ['live', 'prs', 'repos', 'brains', 'docs'].forEach(t => {
        document.getElementById('tab-' + t).classList.toggle('hidden', t !== tab);
      });
      if (tab === 'brains') loadBrainDetail(SELECTED_BRAIN);
      if (tab === 'docs') loadLivingDocs();
    }

    async function api(path, opts = {}) {
      const res = await fetch(path, {
        headers: { 'Content-Type': 'application/json' },
        ...opts
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || ('HTTP ' + res.status));
      return data;
    }

    async function refreshAll() {
      try {
        STATE = await api('/api/overview');
        renderHeaderAndLive();
        renderPrList();
        renderRepos();
        renderBrainsSidebar();
        populateDocsRepoSelect();
      } catch (e) {
        console.error(e);
      }
    }

    function renderHeaderAndLive() {
      if (!STATE) return;
      const pmPill = document.getElementById('pill-poll-mode');
      if (STATE.agents.poll_only_mode) {
        pmPill.innerHTML = `<span class="dot warn"></span>Poll-Only (Auto-AI Paused • 0 Tokens)`;
      } else {
        pmPill.innerHTML = `<span class="dot ok"></span>2m Poll: Pure GraphQL (0 AI Tokens)`;
      }

      const cPill = document.getElementById('pill-claude');
      cPill.innerHTML = `<span class="dot ${STATE.runners.claude_ready ? 'ok' : 'warn'}"></span>claude (${esc(STATE.agents.claude_model || 'sonnet')}): ${STATE.runners.claude_ready ? 'ready' : 'sign-in'}`;
      const aPill = document.getElementById('pill-agy');
      aPill.innerHTML = `<span class="dot ${STATE.runners.agy_ready ? 'ok' : 'warn'}"></span>agy (${esc(STATE.agents.reasoning_effort || 'medium')}): ${STATE.runners.agy_ready ? 'ready' : 'sign-in'}`;

      const iPill = document.getElementById('pill-intercalate');
      iPill.textContent = STATE.agents.intercalate_providers
        ? `⇄ All-Parallel (${STATE.agents.max_parallel_prs} PRs × ${STATE.agents.max_parallel_brains_per_pr} Brains)`
        : `Single Primary (${STATE.agents.primary_runner})`;

      const pPill = document.getElementById('pill-power');
      pPill.innerHTML = `<span class="dot ${STATE.host.ac_power ? 'ok' : 'warn'}"></span>${esc(STATE.host.summary)}`;

      if (!SETTINGS_INITIALIZED) {
        document.getElementById('cfg-claude-model').value = STATE.agents.claude_model || 'sonnet';
        document.getElementById('cfg-effort').value = STATE.agents.reasoning_effort || 'medium';
        document.getElementById('cfg-intercalate').value = String(STATE.agents.intercalate_providers);
        document.getElementById('cfg-max-prs').value = STATE.agents.max_parallel_prs;
        document.getElementById('cfg-max-brains').value = STATE.agents.max_parallel_brains_per_pr;
        document.getElementById('cfg-poll-only').value = String(Boolean(STATE.agents.poll_only_mode));
        SETTINGS_INITIALIZED = true;
      }

      const prs = STATE.pull_requests || [];
      const passCount = prs.filter(p => p.verdict === 'PASS').length;
      const chgCount = prs.filter(p => p.verdict === 'CHANGES_NEEDED').length;
      let openObs = 0, critHigh = 0;
      prs.forEach(p => (p.observations || []).forEach(o => {
        if (o.status === 'open') {
          openObs++;
          if (o.severity === 'CRITICAL' || o.severity === 'HIGH') critHigh++;
        }
      }));

      document.getElementById('kpi-strip').innerHTML = `
        <div class="card"><div class="kpi-label">Active Async Sessions</div><div class="kpi-val" style="color:var(--cyan);">${STATE.active_workers.length} / ${STATE.agents.max_parallel_prs}</div><div class="kpi-sub">Next lead slot: <b>${esc(STATE.next_lead_runner)}</b></div></div>
        <div class="card"><div class="kpi-label">Tracked PRs</div><div class="kpi-val">${prs.length}</div><div class="kpi-sub">${passCount} PASS • ${chgCount} CHANGES_NEEDED</div></div>
        <div class="card"><div class="kpi-label">Open Observations</div><div class="kpi-val" style="color:${critHigh > 0 ? 'var(--red)' : 'var(--amber)'};">${openObs}</div><div class="kpi-sub">${critHigh} blocking (CRITICAL/HIGH)</div></div>
        <div class="card"><div class="kpi-label">Watched Repositories</div><div class="kpi-val">${STATE.repos.filter(r => r.enabled).length}</div><div class="kpi-sub">${STATE.repos.length} configured total</div></div>
      `;

      document.getElementById('parallel-cap-label').textContent =
        `Up to ${STATE.agents.max_parallel_prs} PRs run concurrently in independent async sessions (${STATE.agents.max_parallel_brains_per_pr} parallel brains/wave)`;

      const wb = document.getElementById('active-workers-box');
      if (!STATE.active_workers.length) {
        wb.innerHTML = `<div style="color:var(--muted);padding:8px 0;">Idle — zero in-flight PR sessions right now. Next 2m poll or push will launch parallel async sessions automatically.</div>`;
      } else {
        wb.innerHTML = `<div class="worker-grid">` + STATE.active_workers.map(w => {
          const chips = Object.entries(w.brains || {}).map(([bName, bState]) => {
            const rBadge = bState.runner === 'claude' ? 'b-claude' : (bState.runner === 'agy' ? 'b-agy' : 'b-muted');
            return `<div class="brain-chip ${esc(bState.status)}">
              <span>${esc(bName.replace(/^\d+-/, ''))}</span>
              <span><span class="badge ${rBadge}">${esc(bState.runner)}</span> <span class="badge ${bState.status === 'running' ? 'b-running' : (bState.status === 'done' ? 'b-done' : 'b-muted')}">${esc(bState.status)}</span></span>
            </div>`;
          }).join('');
          return `<div class="worker-card">
            <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
              <strong><span class="dot run"></span> ${esc(w.repo)} #${w.pr_number}</strong>
              <span class="badge b-running">${esc(w.phase)}</span>
            </div>
            <div style="font-size:12.5px;margin-top:4px;">${esc(w.title)}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">
              SHA: <code>${esc((w.head_sha || '').slice(0, 7))}</code> • Reviewer: <code>${esc(w.reviewer_account)}</code> • PID: <code>${w.pid}</code><br/>
              Session Providers: <span class="badge b-claude">lead: ${esc(w.lead_runner)}</span> ⇄ <span class="badge b-agy">alt: ${esc(w.alt_runner)}</span>
            </div>
            <div class="brain-chips">${chips}</div>
          </div>`;
        }).join('') + `</div>`;
      }

      document.getElementById('live-logs-box').textContent = STATE.recent_logs || '(no logs yet)';
      document.getElementById('history-tbody').innerHTML = (STATE.run_history || []).map(h => `
        <tr>
          <td><span class="badge ${h.status === 'pass' ? 'b-pass' : 'b-changes'}">${esc(h.status)}</span></td>
          <td><span class="badge b-muted">GraphQL (0 AI)</span> <code>${esc((h.commit_sha || '').slice(0, 7))}</code></td>
          <td>${h.duration_secs}s</td>
          <td style="color:var(--muted);">${new Date(h.started_epoch_secs * 1000).toISOString().replace('T', ' ').slice(0, 19)}</td>
        </tr>
      `).join('');
    }

    function renderPrList() {
      const prs = STATE.pull_requests || [];
      document.getElementById('pr-count-badge').textContent = prs.length;
      if (!SELECTED_PR_KEY && prs.length) SELECTED_PR_KEY = prs[0].key;

      document.getElementById('pr-list-box').innerHTML = prs.map(p => {
        const openCount = (p.observations || []).filter(o => o.status === 'open').length;
        const vClass = p.verdict === 'PASS' ? 'b-pass' : 'b-changes';
        return `<div class="pr-item ${p.key === SELECTED_PR_KEY ? 'selected' : ''}" onclick="selectPr('${esc(p.key)}')">
          <div style="display:flex;justify-content:space-between;gap:6px;">
            <strong>${esc(p.repo)} #${p.pr_number}</strong>
            <span class="badge ${vClass}">${esc(p.verdict)}</span>
          </div>
          <div style="font-size:12.5px;margin-top:3px;">${esc(p.title)}</div>
          <div style="font-size:11.5px;color:var(--muted);margin-top:5px;">
            Runs: <b>${p.run_count}</b> • Open: <b>${openCount}</b>/${(p.observations || []).length} • SHA: <code>${esc((p.last_reviewed_sha || '-').slice(0, 7))}</code>
          </div>
        </div>`;
      }).join('') || '<div style="color:var(--muted);">No reviewed PRs recorded yet.</div>';

      renderSelectedPrDetail();
    }

    function selectPr(key) {
      SELECTED_PR_KEY = key;
      renderPrList();
    }

    function setObsFilter(f) {
      OBS_FILTER = f;
      renderSelectedPrDetail();
    }

    function renderSelectedPrDetail() {
      const box = document.getElementById('pr-detail-box');
      const p = (STATE.pull_requests || []).find(x => x.key === SELECTED_PR_KEY);
      if (!p) return;

      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const obsAll = p.observations || [];
      const filtered = obsAll.filter(o => {
        if (OBS_FILTER === 'open') return o.status === 'open';
        if (OBS_FILTER === 'resolved') return o.status !== 'open';
        if (OBS_FILTER === 'blocking') return o.severity === 'CRITICAL' || o.severity === 'HIGH';
        return true;
      });

      const obsHtml = filtered.map(o => {
        const sevClass = (o.severity === 'CRITICAL' || o.severity === 'HIGH') ? 'b-high' : (o.severity === 'MEDIUM' ? 'b-medium' : 'b-low');
        const stClass = o.status === 'open' ? 'b-medium' : 'b-pass';
        const loc = o.end_line ? `${o.path}:${o.start_line && o.start_line !== o.end_line ? o.start_line + '-' : ''}${o.end_line}` : o.path;
        return `<div class="obs-card">
          <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;">
            <div>
              <span class="badge ${sevClass}">${esc(o.severity)}</span>
              <span class="badge b-muted">${esc(o.confidence)}</span>
              <span class="badge b-agy">${esc(o.brain_id)}</span>
              <strong style="margin-left:6px;">${esc(o.obs_code)} — ${esc(o.title)}</strong>
            </div>
            <span class="badge ${stClass}">${esc(o.status.toUpperCase())}</span>
          </div>
          <div style="margin-top:6px;font-size:12px;color:#93c5fd;"><code>${esc(loc)}</code></div>
          <div style="margin-top:6px;">${esc(o.claim)}</div>
          <div style="margin-top:6px;color:#cbd5e1;"><strong>Fix Guidance:</strong> ${esc(o.fix_guidance)}</div>
          ${o.resolution_note ? `<div style="margin-top:6px;color:#6ee7b7;"><strong>Resolution Note:</strong> ${esc(o.resolution_note)}</div>` : ''}
          ${o.suggested_code ? `<pre>${esc(o.suggested_code)}</pre>` : ''}
          <div style="margin-top:8px;">
            <button class="secondary" style="padding:4px 9px;font-size:11.5px;" onclick="promoteObsToFp('${esc(o.brain_id)}', '${esc(p.repo)}', '${esc(o.title.replace(/'/g, ''))}')">+ Add False-Positive Guardrail to ${esc(o.brain_id)}</button>
          </div>
        </div>`;
      }).join('') || '<div style="color:var(--muted);padding:10px 0;">No observations match this filter.</div>';

      const traceHtml = (p.traceability || []).length ? `
        <h3 style="margin-top:18px;">Ticket-to-Code Traceability Matrix</h3>
        <table>
          <thead><tr><th>ID</th><th>Requirement</th><th>Code Evidence</th><th>Test Evidence</th><th>Status</th></tr></thead>
          <tbody>
            ${p.traceability.map(c => `<tr><td><code>${esc(c.id)}</code></td><td>${esc(c.requirement)}</td><td><code>${esc(c.code_evidence)}</code></td><td><code>${esc(c.test_evidence)}</code></td><td><span class="badge b-muted">${esc(c.status)}</span></td></tr>`).join('')}
          </tbody>
        </table>` : '';

      box.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap;">
          <div>
            <h2 style="margin-bottom:4px;">${esc(p.repo)} #${p.pr_number} — ${esc(p.title)}</h2>
            <div style="font-size:12px;color:var(--muted);">
              Branch: <code>${esc(p.head_ref)}</code> → <code>${esc(p.base_ref)}</code> • Author: <code>${esc(p.author)}</code> • Reviewed SHA: <code>${esc(p.last_reviewed_sha || '-')}</code>
            </div>
          </div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;">
            <a class="btn secondary" href="${esc(ghUrl)}" target="_blank" rel="noopener">↗ GitHub PR</a>
            ${p.gdrive_doc_url ? `<a class="btn secondary" href="${esc(p.gdrive_doc_url)}" target="_blank" rel="noopener">📄 Google Drive Handoff Doc</a>` : ''}
            <button onclick="triggerAsyncReview('${esc(p.repo)}', ${p.pr_number})">⚡ Start Async Re-Review</button>
          </div>
        </div>
        ${p.latest_summary ? `<div class="card" style="background:var(--panel-2);margin-top:12px;"><strong>Executive Summary:</strong> ${esc(p.latest_summary)}</div>` : ''}
        <div class="filter-bar">
          <button class="${OBS_FILTER === 'all' ? '' : 'secondary'}" onclick="setObsFilter('all')">All (${obsAll.length})</button>
          <button class="${OBS_FILTER === 'open' ? '' : 'secondary'}" onclick="setObsFilter('open')">Open (${obsAll.filter(o => o.status === 'open').length})</button>
          <button class="${OBS_FILTER === 'blocking' ? '' : 'secondary'}" onclick="setObsFilter('blocking')">Critical / High</button>
          <button class="${OBS_FILTER === 'resolved' ? '' : 'secondary'}" onclick="setObsFilter('resolved')">Resolved (${obsAll.filter(o => o.status !== 'open').length})</button>
        </div>
        ${obsHtml}
        ${traceHtml}
      `;
    }

    function renderRepos() {
      document.getElementById('repos-tbody').innerHTML = (STATE.repos || []).map(r => `
        <tr>
          <td><strong>${esc(r.full_name)}</strong></td>
          <td><code>${esc(r.default_branch)}</code></td>
          <td><code>${esc(r.account_policy)}${r.forced_account ? ' (' + esc(r.forced_account) + ')' : ''}</code></td>
          <td><code>${esc(r.review_event_policy)}</code></td>
          <td><span class="badge ${r.enabled ? 'b-pass' : 'b-muted'}">${r.enabled ? 'ENABLED' : 'PAUSED'}</span></td>
          <td style="display:flex;gap:6px;">
            <button class="secondary" style="padding:4px 8px;font-size:11.5px;" onclick="toggleRepo('${esc(r.full_name)}')">${r.enabled ? 'Pause' : 'Enable'}</button>
            <button class="danger" style="padding:4px 8px;font-size:11.5px;" onclick="removeRepo('${esc(r.full_name)}')">Remove</button>
          </td>
        </tr>
      `).join('');
    }

    function renderBrainsSidebar() {
      const brains = STATE.brains || [];
      document.getElementById('brains-list-box').innerHTML = brains.map(b => `
        <div class="pr-item ${b === SELECTED_BRAIN ? 'selected' : ''}" onclick="loadBrainDetail('${esc(b)}')">
          <strong>${esc(b)}</strong>
        </div>
      `).join('');
    }

    async function loadBrainDetail(brainId) {
      SELECTED_BRAIN = brainId;
      renderBrainsSidebar();
      document.getElementById('brain-detail-title').textContent = `Brain: ${brainId}`;
      const d = await api('/api/brain?id=' + encodeURIComponent(brainId));
      document.getElementById('brain-files-box').innerHTML = `
        <h3>Active Lessons (<code>kb/lessons.md</code>)</h3><pre>${esc(d.lessons)}</pre>
        <h3 style="margin-top:14px;">False-Positive Guardrails (<code>kb/false-positives.md</code>)</h3><pre>${esc(d.false_positives)}</pre>
        <h3 style="margin-top:14px;">Repo Exemplars (<code>kb/exemplars.md</code>)</h3><pre>${esc(d.exemplars)}</pre>
        <h3 style="margin-top:14px;">System Prompt (<code>PROMPT.md</code>)</h3><pre>${esc(d.prompt)}</pre>
      `;
    }

    function populateDocsRepoSelect() {
      const sel = document.getElementById('docs-repo-select');
      const prev = sel.value;
      sel.innerHTML = (STATE.repos || []).map(r => `<option value="${esc(r.full_name)}">${esc(r.full_name)}</option>`).join('');
      if (prev) sel.value = prev;
    }

    async function loadLivingDocs() {
      const repo = document.getElementById('docs-repo-select').value;
      if (!repo) return;
      const d = await api('/api/living-docs?repo=' + encodeURIComponent(repo));
      document.getElementById('living-docs-box').innerHTML = (d.files || []).map(f => `
        <h3 style="margin-top:14px;"><code>${esc(f.name)}</code></h3>
        <pre>${esc(f.content)}</pre>
      `).join('');
    }

    async function triggerPollNow() {
      const res = await api('/api/actions/poll', { method: 'POST', body: '{}' });
      showToast(res.message || 'Parallel poll dispatched!');
      setTimeout(refreshAll, 900);
    }

    async function triggerAsyncReview(repo, pr) {
      const res = await api('/api/actions/review', {
        method: 'POST',
        body: JSON.stringify({ repo, pr })
      });
      showToast(res.message || `Launched async session for ${repo}#${pr}`);
      switchTab('live');
      setTimeout(refreshAll, 900);
    }

    async function saveAgentSettings() {
      const claude_model = document.getElementById('cfg-claude-model').value;
      const reasoning_effort = document.getElementById('cfg-effort').value;
      const intercalate_providers = document.getElementById('cfg-intercalate').value === 'true';
      const max_parallel_prs = parseInt(document.getElementById('cfg-max-prs').value || '8', 10);
      const max_parallel_brains_per_pr = parseInt(document.getElementById('cfg-max-brains').value || '8', 10);
      const poll_only_mode = document.getElementById('cfg-poll-only').value === 'true';
      await api('/api/settings/agents', {
        method: 'POST',
        body: JSON.stringify({
          claude_model,
          reasoning_effort,
          intercalate_providers,
          max_parallel_prs,
          max_parallel_brains_per_pr,
          poll_only_mode
        })
      });
      SETTINGS_INITIALIZED = false;
      showToast('Saved settings!');
      refreshAll();
    }

    async function addWatchedRepo() {
      const payload = {
        full_name: document.getElementById('r-name').value.trim(),
        default_branch: document.getElementById('r-branch').value.trim() || 'main',
        account_policy: document.getElementById('r-acct-pol').value,
        forced_account: document.getElementById('r-forced').value.trim() || null,
        review_event_policy: document.getElementById('r-ev-pol').value,
        gdrive_folder_id: document.getElementById('r-drive').value.trim() || null,
        local_check_cmd: document.getElementById('r-check').value.trim() || null
      };
      await api('/api/repos/add', { method: 'POST', body: JSON.stringify(payload) });
      showToast(`Saved watched repo ${payload.full_name}`);
      refreshAll();
    }

    async function toggleRepo(full_name) {
      await api('/api/repos/toggle', { method: 'POST', body: JSON.stringify({ full_name }) });
      refreshAll();
    }

    async function removeRepo(full_name) {
      await api('/api/repos/remove', { method: 'POST', body: JSON.stringify({ full_name }) });
      showToast(`Removed ${full_name}`);
      refreshAll();
    }

    async function submitKbRule() {
      const kind = document.getElementById('kb-kind').value;
      const rule = document.getElementById('kb-rule').value.trim();
      if (!rule) return;
      await api('/api/kb/add', {
        method: 'POST',
        body: JSON.stringify({ brain: SELECTED_BRAIN, kind, rule })
      });
      document.getElementById('kb-rule').value = '';
      showToast(`Added ${kind} rule to ${SELECTED_BRAIN}`);
      loadBrainDetail(SELECTED_BRAIN);
    }

    async function promoteObsToFp(brain, repo, title) {
      const rule = prompt(`False-positive guardrail rule to add to ${brain}:`, `[${repo}] Do not flag: ${title}`);
      if (!rule) return;
      await api('/api/kb/add', {
        method: 'POST',
        body: JSON.stringify({ brain, kind: 'false-positive', rule })
      });
      showToast(`Added false-positive guardrail to ${brain}`);
    }

    async function syncLivingDocsDrive() {
      const repo = document.getElementById('docs-repo-select').value;
      if (!repo) return;
      const res = await api('/api/actions/sync-docs', {
        method: 'POST',
        body: JSON.stringify({ repo })
      });
      showToast(res.message || 'Living docs synced!');
    }

    refreshAll();
    setInterval(refreshAll, 2500);
  </script>
</body>
</html>
"##;
