//! Embedded single-binary dark-mode Web Dashboard HTML/CSS/JS for `code-reviewer` + `Code Tender`.

pub const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Internal CI — Multi-Lens Code Reviewer & Code Tender (M1)</title>
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
      --tender: #10b981;
      --tender-hover: #34d399;
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
      background: rgba(9, 11, 16, 0.95);
      backdrop-filter: blur(10px);
      border-bottom: 1px solid var(--border);
      padding: 9px 20px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      flex-wrap: wrap;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
      flex-wrap: wrap;
    }
    .brand-logo {
      width: 32px;
      height: 32px;
      border-radius: 8px;
      background: linear-gradient(135deg, #6366f1, #06b6d4);
      display: grid;
      place-items: center;
      font-weight: 800;
      color: #fff;
      font-size: 13.5px;
      flex-shrink: 0;
    }
    .brand-logo.tender-mode {
      background: linear-gradient(135deg, #10b981, #06b6d4);
    }
    .brand h1 {
      margin: 0;
      font-size: 15px;
      font-weight: 700;
      letter-spacing: -0.01em;
    }
    .brand .sub {
      display: none;
    }
    .app-switcher {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 4px;
      border-radius: 10px;
      background: var(--panel);
      border: 1px solid var(--border);
      margin-left: 6px;
    }
    .app-switcher button {
      background: transparent;
      border: 1px solid transparent;
      color: var(--muted);
      padding: 6px 12px;
      border-radius: 7px;
      font-size: 12.5px;
      font-weight: 700;
      cursor: pointer;
    }
    .app-switcher button:hover {
      color: var(--text);
      background: var(--panel-2);
    }
    .app-switcher button.active-reviewer {
      background: rgba(99, 102, 241, 0.22);
      border-color: rgba(99, 102, 241, 0.55);
      color: #fff;
    }
    .app-switcher button.active-tender {
      background: rgba(16, 185, 129, 0.2);
      border-color: rgba(16, 185, 129, 0.55);
      color: #d1fae5;
    }
    .app-switcher button.active-pixel {
      background: rgba(168, 85, 247, 0.22);
      border-color: rgba(168, 85, 247, 0.55);
      color: #f3e8ff;
    }
    .brand-logo.pixel-mode {
      background: linear-gradient(135deg, #a855f7, #06b6d4);
    }
    button.pixel-active {
      background: rgba(168, 85, 247, 0.22);
      border-color: rgba(168, 85, 247, 0.6);
      color: #f3e8ff;
    }
    #workspace-shell {
      display: block;
      min-height: calc(100vh - 64px);
    }
    #workspace-shell.mode-split {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(390px, 44%);
      align-items: start;
    }
    #workspace-shell.mode-pixel {
      display: block;
    }
    .workspace-left { min-width: 0; }
    .pixel-pane {
      background: #070a10;
      border-left: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      height: calc(100vh - 64px);
      position: sticky;
      top: 64px;
    }
    #workspace-shell.mode-pixel .pixel-pane {
      border-left: none;
      position: static;
      height: calc(100vh - 66px);
    }
    .pixel-bar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 8px 14px;
      background: var(--panel);
      border-bottom: 1px solid var(--border);
      font-size: 12px;
      color: var(--muted);
      flex-wrap: wrap;
    }
    .pixel-frame {
      flex: 1;
      width: 100%;
      border: none;
      background: #070a10;
      min-height: 460px;
    }
    .pixel-fallback {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 20px;
      gap: 12px;
      text-align: center;
      background: radial-gradient(circle at top, #131b2e, #070a10);
      overflow-y: auto;
    }
    #pixelFallbackCanvas {
      border: 1px solid #2b3856;
      border-radius: 10px;
      background: #0b101b;
      image-rendering: pixelated;
      max-width: 100%;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
    }
    @media (max-width: 1100px) {
      #workspace-shell.mode-split { grid-template-columns: 1fr; }
      .pixel-pane { position: static; height: 580px; border-left: none; border-top: 1px solid var(--border); }
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
    button.tender-btn, .btn.tender-btn { background: #059669; }
    button.tender-btn:hover, .btn.tender-btn:hover { background: #10b981; }
    button.secondary, .btn.secondary {
      background: var(--panel-2);
      border-color: var(--border);
      color: var(--text);
    }
    button.secondary:hover, .btn.secondary:hover { border-color: #4b5563; background: #1e2536; }
    button.danger { background: rgba(239,68,68,0.16); border-color: rgba(239,68,68,0.45); color: #fca5a5; }
    button.danger:hover { background: rgba(239,68,68,0.28); }
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
    nav.tabs.tender-tabs button.active {
      border-bottom-color: var(--tender);
    }
    main { max-width: 1480px; margin: 0 auto; padding: 20px 22px 48px; }
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
      min-width: 0;
    }
    .card.highlight-tender {
      border-color: rgba(16, 185, 129, 0.45);
      background: linear-gradient(180deg, rgba(16,185,129,0.07), var(--panel));
    }
    .kpi-label { font-size: 12px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
    .kpi-val { font-size: 26px; font-weight: 800; margin-top: 4px; }
    .kpi-sub { font-size: 12px; color: var(--muted); margin-top: 2px; }
    .grid-2 { display: grid; grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr); gap: 16px; }
    .split-pr { display: grid; grid-template-columns: 350px minmax(0, 1fr); gap: 16px; align-items: start; }
    @media (max-width: 980px) { .grid-2, .split-pr { grid-template-columns: 1fr; } }
    h2 { font-size: 15px; margin: 0 0 12px; display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; }
    h3 { font-size: 14px; margin: 0 0 8px; }
    .badge {
      display: inline-block;
      padding: 2px 8px;
      border-radius: 6px;
      font-size: 11px;
      font-weight: 700;
      letter-spacing: 0.02em;
    }
    .b-pass, .b-done, .b-low, .b-fixed { background: rgba(16,185,129,0.16); color: #6ee7b7; border: 1px solid rgba(16,185,129,0.35); }
    .b-watching { background: rgba(6,182,212,0.14); color: #67e8f9; border: 1px solid rgba(6,182,212,0.35); }
    .b-changes, .b-critical, .b-high, .b-failed { background: rgba(239,68,68,0.16); color: #fca5a5; border: 1px solid rgba(239,68,68,0.4); }
    .b-medium, .b-running, .b-staged { background: rgba(245,158,11,0.16); color: #fcd34d; border: 1px solid rgba(245,158,11,0.4); }
    .b-claude { background: rgba(249,115,22,0.16); color: #fdba74; border: 1px solid rgba(249,115,22,0.4); }
    .b-agy { background: rgba(6,182,212,0.16); color: #67e8f9; border: 1px solid rgba(6,182,212,0.4); }
    .b-explained { background: rgba(168,85,247,0.16); color: #d8b4fe; border: 1px solid rgba(168,85,247,0.4); }
    .b-muted { background: #1a2130; color: var(--muted); border: 1px solid var(--border); }
    .worker-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: 12px; }
    .worker-card {
      background: var(--panel-2);
      border: 1px solid #2e3a56;
      border-radius: 10px;
      padding: 14px;
      min-width: 0;
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
      padding: 11px 12px;
      border-radius: 9px;
      border: 1px solid var(--border);
      background: var(--panel-2);
      margin-bottom: 8px;
      cursor: pointer;
    }
    .pr-item:hover, .pr-item.selected { border-color: var(--accent); background: #1a2236; }
    .pr-item.selected-tender { border-color: var(--tender); background: #122624; }
    .obs-card {
      background: var(--panel-2);
      border: 1px solid var(--border);
      border-radius: 9px;
      padding: 13px 14px;
      margin-bottom: 10px;
      min-width: 0;
    }
    pre {
      background: #07090e;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 12px;
      overflow-x: auto;
      white-space: pre-wrap;
      word-break: break-word;
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
    textarea.code-editor {
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 12px;
      min-height: 190px;
      line-height: 1.45;
      resize: vertical;
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
      max-width: 440px;
    }
    .hidden { display: none !important; }
    .filter-bar { display: flex; gap: 6px; flex-wrap: wrap; margin: 10px 0 12px; }
    .watch-bar {
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;
      padding: 12px 14px;
      border-radius: 11px;
      background: linear-gradient(180deg, rgba(16,185,129,0.09), var(--panel));
      border: 1px solid rgba(16,185,129,0.38);
      margin-bottom: 14px;
    }
    .stepper {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 8px;
      margin: 12px 0;
    }
    @media (max-width: 720px) { .stepper { grid-template-columns: 1fr; } }
    .step-node {
      padding: 9px 11px;
      border-radius: 8px;
      background: var(--bg);
      border: 1px solid var(--border);
      font-size: 12px;
    }
    .step-node .step-title { font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 6px; }
    .step-node .step-sub { color: var(--muted); font-size: 11.5px; margin-top: 2px; }
    .step-node.done { border-color: rgba(16,185,129,0.45); background: rgba(16,185,129,0.07); }
    .step-node.active { border-color: rgba(6,182,212,0.55); background: rgba(6,182,212,0.09); }
    .step-node.warn { border-color: rgba(239,68,68,0.5); background: rgba(239,68,68,0.08); }
    .status-banner {
      padding: 13px 15px;
      border-radius: 10px;
      border: 1px solid var(--border);
      background: var(--panel-2);
      margin: 12px 0;
    }
    .status-banner.watching { border-color: rgba(6,182,212,0.38); background: rgba(6,182,212,0.06); }
    .status-banner.pushed { border-color: rgba(16,185,129,0.45); background: rgba(16,185,129,0.08); }
    .status-banner.staged { border-color: rgba(245,158,11,0.48); background: rgba(245,158,11,0.08); }
    .status-banner.failed { border-color: rgba(239,68,68,0.48); background: rgba(239,68,68,0.08); }
    details.clean-acc {
      border: 1px solid var(--border);
      border-radius: 10px;
      background: var(--panel);
      margin-top: 12px;
    }
    details.clean-acc > summary {
      cursor: pointer;
      padding: 11px 15px;
      font-weight: 700;
      font-size: 13.5px;
      list-style: none;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      user-select: none;
    }
    details.clean-acc > summary::-webkit-details-marker { display: none; }
    details.clean-acc > summary::after { content: '▾'; color: var(--muted); font-size: 12px; }
    details.clean-acc[open] > summary { border-bottom: 1px solid var(--border); }
    details.clean-acc[open] > summary::after { content: '▴'; }
    details.clean-acc .acc-body { padding: 14px 15px; }
    .repo-card {
      padding: 12px 14px;
      border-radius: 9px;
      border: 1px solid var(--border);
      background: var(--panel-2);
      margin-bottom: 9px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
    }
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="brand-logo" id="brand-logo">CR</div>
      <div>
        <h1 id="brand-title">Code Reviewer</h1>
        <div class="sub" id="brand-sub"></div>
      </div>
      <div class="app-switcher" role="navigation" aria-label="Subsystem Switcher">
        <button id="nav-app-reviewer" class="active-reviewer" onclick="switchApp('reviewer')">
          🔍 Reviewer <span class="badge b-muted" id="nav-reviewer-badge">0</span>
        </button>
        <button id="nav-app-tender" onclick="switchApp('tender')">
          🛠️ Code Tender <span class="badge b-muted" id="nav-tender-badge">0</span>
        </button>
        <button id="nav-app-pixel" onclick="switchApp('pixel')">
          👾 Pixel Office <span class="badge b-muted" id="nav-pixel-badge">...</span>
        </button>
      </div>
    </div>
    <div class="header-right">
      <span class="pill" id="pill-poll-mode"><span class="dot ok"></span>2m Auto-Sync</span>
      <span class="pill" id="pill-system" title="AI Runners &amp; Host Power"><span class="dot ok"></span>claude · agy · M1</span>
      <button class="secondary" id="btn-split-pixel" onclick="toggleSplitPixel()" title="Dock Pixel Agents Office side-by-side">👾 Split Office</button>
      <button id="primary-poll-btn" onclick="triggerPrimaryAction()">⚡ Sync &amp; Review</button>
      <button class="secondary" onclick="refreshAll()" title="Refresh Dashboard">↻</button>
    </div>
  </header>

  <div id="workspace-shell">
    <div class="workspace-left" id="workspace-left">
  <!-- ========================================================= -->
  <!-- APP 1: MULTI-LENS CODE REVIEWER                           -->
  <!-- ========================================================= -->
  <div id="app-reviewer">
    <nav class="tabs" id="reviewer-tabs">
      <button class="active" data-tab="live" onclick="switchReviewerTab('live')">1. Live Parallel Sessions</button>
      <button data-tab="prs" onclick="switchReviewerTab('prs')">2. Pull Requests &amp; Observations</button>
      <button data-tab="repos" onclick="switchReviewerTab('repos')">3. Watched Repos</button>
      <button data-tab="brains" onclick="switchReviewerTab('brains')">4. Brains &amp; Knowledge Bases (8)</button>
      <button data-tab="docs" onclick="switchReviewerTab('docs')">5. Living Documentation</button>
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
            <div style="color:var(--muted);">Select a Pull Request on the left to inspect open/resolved observations, traceability matrix, Google Drive link, launch an async re-review, or open it in Code Tender.</div>
          </div>
        </div>
      </section>

      <!-- TAB 3: WATCHED REPOS -->
      <section id="tab-repos" class="hidden">
        <div class="grid-2">
          <div class="card">
            <h2>Watched Repositories (`config/repos.toml`)</h2>
            <table>
              <thead><tr><th>Repository</th><th>Branch</th><th>Identity Policy</th><th>Review Event</th><th>Reviewer</th><th>Tender</th><th>Local Checkout / Check</th><th>Actions</th></tr></thead>
              <tbody id="repos-tbody"></tbody>
            </table>
          </div>
          <div class="card">
            <h2>Add / Update Watched Repository</h2>
            <div class="form-row">
              <div><label>Owner / Repo (or GitHub URL)</label><input id="r-name" placeholder="acme-org/web-app or https://github.com/acme-org/web-app" onblur="normalizeRepoInputAndDetect('r')" /></div>
              <div><label>Default Branch</label><input id="r-branch" value="main" /></div>
            </div>
            <div class="form-row">
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--text);">
                <input type="checkbox" id="r-enabled" checked style="width:auto;" />
                <span>Run Multi-Lens Code Reviewer</span>
              </label>
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--text);">
                <input type="checkbox" id="r-tender-enabled" checked style="width:auto;" />
                <span>Run Code Tender (Auto-Fix PR Reviews)</span>
              </label>
            </div>
            <div class="form-row">
              <div style="grid-column: span 2;">
                <label>Local App Checkout Path on M1 (Code Tender spawns grounded in this app's deps &amp; docs)</label>
                <div style="display:flex;gap:8px;flex-wrap:wrap;">
                  <input id="r-local-path" style="flex:1;min-width:200px;" placeholder="~/apps/web-app" />
                  <button type="button" class="secondary" style="white-space:nowrap;" onclick="openM1Browser('r')">📂 Browse M1 Folders</button>
                  <button type="button" class="secondary" style="white-space:nowrap;" onclick="loadGitInfoFromPath('r')">📥 Load Git Info</button>
                  <button type="button" class="secondary" style="white-space:nowrap;" onclick="detectLocalRepo('r')">🔎 Auto-Detect</button>
                </div>
              </div>
            </div>
            <div class="form-row">
              <div style="grid-column: span 2;">
                <label>Local Verification Check Command (required before push; auto-inferred from package.json/Cargo.toml if blank)</label>
                <input id="r-check" placeholder="npm run typecheck && npm test" />
              </div>
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
            <button onclick="submitWatchedRepoForm('r')">Save Watched Repository</button>
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
  </div>

  <!-- ========================================================= -->
  <!-- APP 2: CODE TENDER (WATCH PULLS -> TEND REVIEW COMMENTS)  -->
  <!-- ========================================================= -->
  <div id="app-tender" class="hidden">
    <nav class="tabs tender-tabs" id="tender-tabs">
      <button class="active" data-tab="tender-prs" onclick="switchTenderTab('tender-prs')">Watched Pulls</button>
      <button data-tab="tender-repos" onclick="switchTenderTab('tender-repos')">Repos &amp; Settings</button>
    </nav>

    <main>
      <!-- TENDER TAB 1 (DEFAULT): WATCHED PULLS -->
      <section id="tab-tender-prs">
        <div id="tender-live-banner" class="hidden" style="margin-bottom:14px;"></div>

        <div class="watch-bar">
          <div style="font-weight:700;font-size:13px;white-space:nowrap;">👀 Watch a Pull:</div>
          <input
            id="watch-pr-input"
            style="flex:1;min-width:240px;"
            placeholder="Paste GitHub PR URL or owner/repo#123..."
            onkeydown="if(event.key==='Enter') watchPrFromInput()"
          />
          <button class="tender-btn" onclick="watchPrFromInput()">+ Watch PR</button>
          <button class="secondary" id="btn-open-pr-picker" onclick="toggleOpenPrPicker()">📋 Pick Open PR</button>
          <span
            id="tender-mode-summary"
            class="pill"
            style="cursor:pointer;margin-left:auto;"
            onclick="switchTenderTab('tender-repos')"
            title="Click to adjust Code Tender settings"
          >Auto-Tend: ON</span>
        </div>

        <div id="open-pr-picker-box" class="card hidden" style="margin-bottom:14px;">
          <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:10px;">
            <div>
              <strong>Open Pull Requests on GitHub</strong>
              <span style="font-size:12px;color:var(--muted);margin-left:8px;">Click Watch to track any open PR for review comments.</span>
            </div>
            <div style="display:flex;gap:8px;align-items:center;">
              <button class="secondary" style="padding:5px 10px;font-size:12px;" onclick="loadOpenPrPicker()">↻ Refresh Open PRs</button>
              <button class="secondary" style="padding:5px 10px;font-size:12px;" onclick="toggleOpenPrPicker(false)">✕ Close</button>
            </div>
          </div>
          <div id="open-pr-picker-list" style="max-height:280px;overflow-y:auto;"></div>
        </div>

        <div class="filter-bar" id="tender-pr-filters" style="margin-top:0;"></div>

        <div class="split-pr">
          <div class="card">
            <h2>
              <span>Watched Pulls</span>
              <span id="tender-pr-count-badge" class="badge b-muted">0</span>
            </h2>
            <div id="tender-pr-list-box"></div>
          </div>
          <div class="card" id="tender-pr-detail-box">
            <div style="color:var(--muted);padding:12px 0;">
              Pick an open PR above or paste a PR URL to start watching it for review comments.
            </div>
          </div>
        </div>
      </section>

      <!-- TENDER TAB 2: REPOS & SETTINGS -->
      <section id="tab-tender-repos" class="hidden">
        <div class="grid-2" style="margin-bottom:16px;">
          <div class="card">
            <h2>
              <span>Watched Repositories</span>
              <span style="font-size:12px;color:var(--muted);font-weight:500;">Open PRs on Auto-Watch repos are watched automatically</span>
            </h2>
            <div id="tender-repos-list"></div>
          </div>

          <div class="card">
            <h2 id="tr-form-heading">Add / Update Repository</h2>
            <div class="form-row">
              <div>
                <label>Owner / Repo (or GitHub URL)</label>
                <input id="tr-name" placeholder="acme-org/web-app" onblur="normalizeRepoInputAndDetect('tr')" />
              </div>
              <div>
                <label>Default Branch</label>
                <input id="tr-branch" value="main" />
              </div>
            </div>
            <div class="form-row" style="margin-bottom:12px;">
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--text);">
                <input type="checkbox" id="tr-tender-enabled" checked style="width:auto;" />
                <span>Auto-Watch &amp; Tend Open PRs</span>
              </label>
              <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--text);">
                <input type="checkbox" id="tr-enabled" style="width:auto;" />
                <span>Also Run Multi-Lens Reviewer</span>
              </label>
            </div>
            <div style="margin-bottom:10px;">
              <label>Local App Checkout on M1 (for full repo context &amp; local checks)</label>
              <div style="display:flex;gap:6px;flex-wrap:wrap;">
                <input id="tr-local-path" style="flex:1;min-width:190px;" placeholder="~/apps/web-app" />
                <button type="button" class="secondary" onclick="openM1Browser('tr')">📂 Browse M1</button>
                <button type="button" class="secondary" onclick="loadGitInfoFromPath('tr')">📥 Load Git</button>
                <button type="button" class="secondary" onclick="detectLocalRepo('tr')">🔎 Detect</button>
              </div>
            </div>
            <div style="margin-bottom:10px;">
              <label>Local Check Command (runs before push; auto-inferred if blank)</label>
              <input id="tr-check" placeholder="pnpm --filter @app/web typecheck" />
            </div>
            <details class="clean-acc" style="margin:10px 0 12px;background:var(--panel-2);">
              <summary style="padding:8px 12px;font-size:12.5px;">Advanced Repository Options (Account &amp; Drive)</summary>
              <div class="acc-body" style="padding:10px 12px;">
                <div class="form-row" style="margin-bottom:8px;">
                  <div>
                    <label>Account Policy</label>
                    <select id="tr-acct-pol">
                      <option value="swap">swap (primary ⇄ secondary)</option>
                      <option value="forced">forced (single account)</option>
                    </select>
                  </div>
                  <div><label>Forced Account</label><input id="tr-forced" placeholder="optional" /></div>
                </div>
                <div class="form-row" style="margin-bottom:0;">
                  <div>
                    <label>Reviewer Event Policy</label>
                    <select id="tr-ev-pol">
                      <option value="formal_verdict">formal_verdict</option>
                      <option value="comment_only">comment_only</option>
                    </select>
                  </div>
                  <div><label>Google Drive Folder ID</label><input id="tr-drive" placeholder="optional" /></div>
                </div>
              </div>
            </details>
            <button class="tender-btn" onclick="submitWatchedRepoForm('tr')">Save Repository</button>
          </div>
        </div>

        <div class="card" style="margin-bottom:14px;">
          <h2><span>Core Tender Behavior</span></h2>
          <div class="form-row">
            <div>
              <label>When Review Comments Land on a Watched PR</label>
              <select id="tcfg-enabled">
                <option value="true">Automatically Tend Comments (Every 2m)</option>
                <option value="false">Manual Only (Click "Tend Now" on PR)</option>
              </select>
            </div>
            <div>
              <label>When Local Checks Pass</label>
              <select id="tcfg-delivery">
                <option value="auto_push">Commit &amp; Push Verified Fix Automatically</option>
                <option value="stage_patch_only">Stage Patch Only (Wait for My Approval)</option>
              </select>
            </div>
            <div>
              <label>After Pushing a Verified Fix</label>
              <select id="tcfg-resolve">
                <option value="true">Reply &amp; Resolve Review Threads</option>
                <option value="false">Reply Only (Leave Threads Open)</option>
              </select>
            </div>
            <div style="display:flex;align-items:flex-end;">
              <button class="tender-btn" style="width:100%;justify-content:center;" onclick="saveTenderSettings()">Save Settings</button>
            </div>
          </div>
        </div>

        <details class="clean-acc">
          <summary><span>⚙️ Advanced AI Concurrency &amp; Safety Limits</span></summary>
          <div class="acc-body">
            <div class="form-row">
              <div>
                <label>2m Poller Mode</label>
                <select id="tcfg-poll-only">
                  <option value="false">Event-Only AI (0 AI on idle polls)</option>
                  <option value="true">Strict Poll-Only (Pause Auto-AI)</option>
                </select>
              </div>
              <div>
                <label>Provider Load-Splitting (`claude` ⇄ `agy`)</label>
                <select id="tcfg-intercalate">
                  <option value="true">Enabled (50/50 + Failover)</option>
                  <option value="false">Fixed Primary / Verifier</option>
                </select>
              </div>
              <div>
                <label>Ignore Clean PASS / APPROVED Reviews</label>
                <select id="tcfg-ignore-pass">
                  <option value="true">Yes (Skip non-actionable approvals)</option>
                  <option value="false">No (Inspect all reviews)</option>
                </select>
              </div>
              <div>
                <label>Max Parallel PRs</label>
                <input type="number" id="tcfg-max-prs" min="1" max="12" value="4" />
              </div>
              <div>
                <label>Max Parallel Fixers / PR</label>
                <input type="number" id="tcfg-max-fixers" min="1" max="8" value="4" />
              </div>
              <div>
                <label>Max Autonomous Cycles / PR</label>
                <input type="number" id="tcfg-max-cycles" min="1" max="20" value="3" />
              </div>
            </div>
            <div style="margin-bottom:12px;">
              <label>Allowed PR Authors (comma-separated safety boundary for auto-watch)</label>
              <input id="tcfg-allowed-authors" placeholder="primary_account, secondary_account" />
            </div>
            <button class="tender-btn" onclick="saveTenderSettings()">Save Advanced Settings</button>
          </div>
        </details>

        <details class="clean-acc" ontoggle="if(this.open){loadTenderKb();loadTenderAgentDetail(SELECTED_TENDER_AGENT);}">
          <summary><span>🧠 Continuous-Learning Knowledge Base &amp; Agent Prompts (01..03)</span></summary>
          <div class="acc-body">
            <div class="card" style="background:var(--panel-2);margin-bottom:14px;">
              <h3>+ Append Learned Rule</h3>
              <div class="form-row">
                <div>
                  <label>Target KB File</label>
                  <select id="tkb-target">
                    <option value="patterns-fixed">Patterns Fixed (`patterns-fixed.md`)</option>
                    <option value="check-recipes">Local Check Recipes (`check-recipes.md`)</option>
                    <option value="do-not-repeat">Do Not Repeat Guardrails (`do-not-repeat.md`)</option>
                    <option value="agent-lesson">Specific Agent Lessons (`kb/lessons.md`)</option>
                  </select>
                </div>
                <div>
                  <label>Target Agent (if Agent Lessons)</label>
                  <select id="tkb-agent">
                    <option value="01-tender-triage">01-tender-triage</option>
                    <option value="02-tender-fixer">02-tender-fixer</option>
                    <option value="03-tender-verifier">03-tender-verifier</option>
                  </select>
                </div>
                <div style="grid-column: span 2;">
                  <label>Imperative Rule (include `[Owner/repo]` or `[*]`)</label>
                  <input id="tkb-rule" placeholder="[acme-org/web-app] Always run typecheck before marking fixes done." />
                </div>
              </div>
              <button class="tender-btn" onclick="submitTenderKbRule()">+ Append Rule</button>
            </div>

            <div class="grid-2">
              <div>
                <label>Patterns Fixed (`tender-agents/kb/patterns-fixed.md`)</label>
                <textarea id="tkb-patterns" class="code-editor" style="min-height:150px;"></textarea>
                <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('patterns-fixed', 'tkb-patterns')">💾 Save `patterns-fixed.md`</button>
              </div>
              <div>
                <label>Local Check Recipes (`tender-agents/kb/check-recipes.md`)</label>
                <textarea id="tkb-recipes" class="code-editor" style="min-height:150px;"></textarea>
                <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('check-recipes', 'tkb-recipes')">💾 Save `check-recipes.md`</button>
              </div>
            </div>
            <div style="margin:12px 0 18px;">
              <label>Do Not Repeat Guardrails (`tender-agents/kb/do-not-repeat.md`)</label>
              <textarea id="tkb-dnr" class="code-editor" style="min-height:130px;"></textarea>
              <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('do-not-repeat', 'tkb-dnr')">💾 Save `do-not-repeat.md`</button>
            </div>

            <div class="split-pr">
              <div class="card" style="background:var(--panel-2);">
                <h3>Tender Agents (01..03)</h3>
                <div id="tender-agents-list-box"></div>
              </div>
              <div class="card" style="background:var(--panel-2);">
                <h3 id="tender-agent-detail-title">Tender Agent Prompt &amp; Guardrails</h3>
                <div style="margin-bottom:10px;">
                  <label>System Instructions (`PROMPT.md`)</label>
                  <textarea id="ta-prompt" class="code-editor"></textarea>
                </div>
                <div style="margin-bottom:10px;">
                  <label>Operator Notes (`notes.md`)</label>
                  <textarea id="ta-notes" class="code-editor" style="min-height:100px;"></textarea>
                </div>
                <div style="margin-bottom:10px;">
                  <label>Learned Agent Lessons (`kb/lessons.md`)</label>
                  <textarea id="ta-lessons" class="code-editor" style="min-height:110px;"></textarea>
                </div>
                <button class="tender-btn" onclick="saveTenderAgentPrompt()">💾 Save Agent Prompt &amp; Notes</button>
              </div>
            </div>
          </div>
        </details>

        <details class="clean-acc">
          <summary><span>📜 Live Poller Logs &amp; Recent 2m Poll History</span></summary>
          <div class="acc-body">
            <pre id="tender-logs-box">Loading tender logs...</pre>
            <h3 style="margin-top:14px;">Recent 2m GraphQL Poll Ticks <span class="badge b-pass">0 AI Tokens / Poll</span></h3>
            <div style="max-height:200px;overflow-y:auto;">
              <table>
                <thead><tr><th>Tick</th><th>Engine</th><th>Duration</th><th>Started (UTC)</th></tr></thead>
                <tbody id="tender-history-tbody"></tbody>
              </table>
            </div>
          </div>
        </details>
      </section>
    </main>
  </div>
    </div>

    <!-- ========================================================= -->
    <!-- APP 3 / SPLIT DOCK: PIXEL AGENTS VISUAL OFFICE            -->
    <!-- ========================================================= -->
    <aside id="pixel-office-pane" class="pixel-pane hidden">
      <div class="pixel-bar">
        <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
          <strong>👾 Pixel Agents Office</strong>
          <span class="badge b-muted" id="pixel-conn-badge">Checking :7421...</span>
          <span id="pixel-workers-pill" style="font-size:11.5px;color:var(--muted);"></span>
        </div>
        <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;">
          <button class="secondary" style="padding:4px 9px;font-size:11.5px;" onclick="startPixelAgentsServer()">▶ Start :7421</button>
          <button class="secondary" style="padding:4px 9px;font-size:11.5px;" onclick="loadPixelOffice(true)">↻ Reload Office</button>
          <a id="pixel-open-ext" class="btn secondary" style="padding:4px 9px;font-size:11.5px;" href="#" target="_blank" rel="noopener">↗ Open Tab</a>
        </div>
      </div>
      <iframe id="pixelOfficeFrame" class="pixel-frame hidden" title="Pixel Agents Visual Office"></iframe>
      <div id="pixelFallbackPane" class="pixel-fallback">
        <canvas id="pixelFallbackCanvas" width="760" height="320"></canvas>
        <div style="max-width:640px;">
          <div style="font-weight:700;font-size:15px;margin-bottom:4px;" id="pixel-fallback-title">Live M1 Agent Floorplan Preview</div>
          <div style="font-size:12.5px;color:var(--muted);margin-bottom:10px;" id="pixel-fallback-desc">
            Connecting to standalone <code>pixel-agents v1.6.0</code> on port <code>7421</code>...
          </div>
          <div style="display:flex;gap:8px;justify-content:center;flex-wrap:wrap;">
            <button onclick="startPixelAgentsServer()">▶ Start Pixel Agents Server (:7421)</button>
            <button class="secondary" onclick="loadPixelOffice(true)">↻ Retry Connection</button>
          </div>
        </div>
      </div>
    </aside>
  </div>

  <div id="m1-browser-modal" class="hidden" style="position:fixed;inset:0;background:rgba(2,6,23,0.78);z-index:100;display:none;align-items:center;justify-content:center;padding:20px;">
    <div class="card" style="width:min(860px,100%);max-height:86vh;display:flex;flex-direction:column;gap:12px;overflow:hidden;">
      <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;">
        <h2 style="margin:0;">📂 Select Repository Folder on M1</h2>
        <button class="secondary" style="padding:5px 10px;" onclick="closeM1Browser()">✕ Close</button>
      </div>
      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
        <button id="m1-up-btn" class="secondary" style="padding:6px 10px;" onclick="navigateM1Up()">⬆ Up</button>
        <input id="m1-path-input" style="flex:1;min-width:240px;" placeholder="~/" onkeydown="if(event.key==='Enter') navigateM1Browser(this.value)" />
        <button class="secondary" style="padding:6px 12px;" onclick="navigateM1Browser(document.getElementById('m1-path-input').value)">Open Path</button>
        <button class="tender-btn" style="padding:6px 12px;" onclick="pickM1Folder(document.getElementById('m1-path-input').value)">✓ Use Current Folder</button>
      </div>
      <div id="m1-browser-status" style="font-size:12px;color:var(--muted);"></div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;overflow:hidden;min-height:280px;">
        <div style="border:1px solid var(--border);border-radius:8px;padding:10px;overflow-y:auto;">
          <h3 style="margin-bottom:8px;">⚡ Discovered Local Git Repos on M1</h3>
          <div id="m1-discovered-list" style="display:flex;flex-direction:column;gap:6px;"></div>
        </div>
        <div style="border:1px solid var(--border);border-radius:8px;padding:10px;overflow-y:auto;">
          <h3 style="margin-bottom:8px;">📁 Folders in Current Directory</h3>
          <div id="m1-dir-list" style="display:flex;flex-direction:column;gap:6px;"></div>
        </div>
      </div>
    </div>
  </div>

  <div class="toast" id="toast"></div>

  <script>
    let STATE = null;
    let ACTIVE_APP = 'reviewer';
    let ACTIVE_REVIEWER_TAB = 'live';
    let ACTIVE_TENDER_TAB = 'tender-prs';
    let SELECTED_PR_KEY = null;
    let SELECTED_TENDER_PR_KEY = null;
    let OBS_FILTER = 'all';
    let TENDER_PR_FILTER = 'all';
    let TENDER_ITEM_FILTER = 'all';
    let OPEN_PR_PICKER_VISIBLE = false;
    let OPEN_PR_PICKER_ITEMS = [];
    let OPEN_PR_PICKER_LOADING = false;
    let SELECTED_BRAIN = '01-intent-traceability';
    let SELECTED_TENDER_AGENT = '01-tender-triage';
    let SETTINGS_INITIALIZED = false;
    let TENDER_SETTINGS_INITIALIZED = false;
    let SPLIT_PIXEL = localStorage.getItem('cr_split_pixel') === '1';
    let PIXEL_OFFICE_URL = null;
    let PIXEL_ONLINE = false;
    let PIXEL_LOADED_ONCE = false;

    function detectInitialApp() {
      const path = (window.location.pathname || '').toLowerCase();
      const params = new URLSearchParams(window.location.search || '');
      if (path.startsWith('/pixel') || params.get('app') === 'pixel' || window.location.hash === '#pixel') {
        return 'pixel';
      }
      if (path.startsWith('/tender') || params.get('app') === 'tender' || window.location.hash === '#tender') {
        return 'tender';
      }
      return 'reviewer';
    }

    function showToast(msg) {
      const el = document.getElementById('toast');
      el.textContent = msg;
      el.style.display = 'block';
      clearTimeout(window.__toastTimer);
      window.__toastTimer = setTimeout(() => { el.style.display = 'none'; }, 3800);
    }

    function esc(s) {
      return String(s ?? '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
    }

    const TENDER_REPLY_DRAFTS = {};

    function escJsAttr(s) {
      return String(s ?? '')
        .replace(/\\/g, '\\\\')
        .replace(/'/g, "\\'")
        .replace(/&/g, '&amp;')
        .replace(/"/g, '&quot;');
    }

    function cleanReviewBody(raw) {
      let s = String(raw ?? '');
      s = s.replace(/\.\.\.\[truncated for prompt budget[^\]]*\]\.\.\./gi, '…');
      s = s.replace(/<!--[\s\S]*?(-->|$)/g, '');
      let guard = 0;
      while (/<details\b/i.test(s) && guard++ < 8) {
        s = s.replace(/<details\b(?:(?!<details\b)[\s\S])*?(<\/details>|$)/gi, '');
      }
      s = s.replace(/<\/details>/gi, '');
      s = s.replace(/<picture[\s\S]*?(<\/picture>|$)/gi, '');
      s = s.replace(/<img[^>]*>/gi, '');
      s = s.replace(/<br\s*\/?>/gi, '\n');
      s = s.replace(/\n{3,}/g, '\n\n').trim();
      if (s === '…') s = '';
      return s;
    }

    function applyWorkspaceLayout() {
      const isPixel = ACTIVE_APP === 'pixel';
      const isTender = ACTIVE_APP === 'tender';
      const showPixelPane = isPixel || SPLIT_PIXEL;

      document.getElementById('app-reviewer').classList.toggle('hidden', isPixel || isTender);
      document.getElementById('app-tender').classList.toggle('hidden', isPixel || !isTender);
      document.getElementById('workspace-left').classList.toggle('hidden', isPixel);

      const shell = document.getElementById('workspace-shell');
      shell.classList.toggle('mode-pixel', isPixel);
      shell.classList.toggle('mode-split', !isPixel && SPLIT_PIXEL);

      const pixelPane = document.getElementById('pixel-office-pane');
      pixelPane.classList.toggle('hidden', !showPixelPane);

      const splitBtn = document.getElementById('btn-split-pixel');
      splitBtn.classList.toggle('pixel-active', !isPixel && SPLIT_PIXEL);
      splitBtn.classList.toggle('hidden', isPixel);
      splitBtn.textContent = SPLIT_PIXEL ? '✕ Hide Office' : '👾 Split Office';

      if (showPixelPane && !PIXEL_LOADED_ONCE) {
        loadPixelOffice(false);
      }
    }

    function toggleSplitPixel() {
      SPLIT_PIXEL = !SPLIT_PIXEL;
      localStorage.setItem('cr_split_pixel', SPLIT_PIXEL ? '1' : '0');
      applyWorkspaceLayout();
    }

    function switchApp(app, pushUrl = true) {
      ACTIVE_APP = (app === 'tender' || app === 'pixel') ? app : 'reviewer';
      const isTender = ACTIVE_APP === 'tender';
      const isPixel = ACTIVE_APP === 'pixel';

      document.getElementById('nav-app-reviewer').classList.toggle('active-reviewer', ACTIVE_APP === 'reviewer');
      document.getElementById('nav-app-tender').classList.toggle('active-tender', isTender);
      document.getElementById('nav-app-pixel').classList.toggle('active-pixel', isPixel);

      const logo = document.getElementById('brand-logo');
      logo.textContent = isPixel ? 'PX' : (isTender ? 'CT' : 'CR');
      logo.classList.toggle('tender-mode', isTender);
      logo.classList.toggle('pixel-mode', isPixel);

      document.getElementById('brand-title').textContent = isPixel
        ? 'Pixel Agents Office'
        : (isTender ? 'Code Tender' : 'Multi-Lens Reviewer');

      const primaryBtn = document.getElementById('primary-poll-btn');
      primaryBtn.className = isTender ? 'tender-btn' : '';
      primaryBtn.textContent = isPixel
        ? '↻ Reload Office'
        : (isTender ? '↻ Sync Pulls' : '⚡ Sync & Review');

      applyWorkspaceLayout();

      if (pushUrl && window.location.protocol.startsWith('http')) {
        const targetPath = isPixel ? '/pixel' : (isTender ? '/tender' : '/reviewer');
        if (window.location.pathname !== targetPath) {
          window.history.pushState({ app: ACTIVE_APP }, '', targetPath);
        }
      }

      renderTopStatusBar();
    }

    window.addEventListener('popstate', () => {
      switchApp(detectInitialApp(), false);
    });

    function switchReviewerTab(tab) {
      ACTIVE_REVIEWER_TAB = tab;
      document.querySelectorAll('#reviewer-tabs button').forEach(b => {
        b.classList.toggle('active', b.dataset.tab === tab);
      });
      ['live', 'prs', 'repos', 'brains', 'docs'].forEach(t => {
        document.getElementById('tab-' + t).classList.toggle('hidden', t !== tab);
      });
      if (tab === 'brains') loadBrainDetail(SELECTED_BRAIN);
      if (tab === 'docs') loadLivingDocs();
    }

    function switchTenderTab(tab) {
      const normalized = (tab === 'tender-repos') ? 'tender-repos' : 'tender-prs';
      ACTIVE_TENDER_TAB = normalized;
      document.querySelectorAll('#tender-tabs button').forEach(b => {
        b.classList.toggle('active', b.dataset.tab === normalized);
      });
      ['tender-prs', 'tender-repos'].forEach(t => {
        const el = document.getElementById('tab-' + t);
        if (el) el.classList.toggle('hidden', t !== normalized);
      });
    }

    async function openPrInTender(repo, prNumber) {
      switchApp('tender');
      switchTenderTab('tender-prs');
      const key = `${repo}#${prNumber}`;
      const exists = (STATE?.tender_pull_requests || []).some(p => p.key === key);
      if (exists) {
        SELECTED_TENDER_PR_KEY = key;
        renderTenderPrList();
      } else {
        await setPrWatched(repo, prNumber, true);
      }
    }

    function openPrInReviewer(repo, prNumber) {
      switchApp('reviewer');
      const key = `${repo}#${prNumber}`;
      const exists = (STATE?.pull_requests || []).some(p => p.key === key);
      if (exists) {
        SELECTED_PR_KEY = key;
        renderPrList();
        switchReviewerTab('prs');
      } else {
        switchReviewerTab('live');
      }
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
        renderTopStatusBar();
        renderHeaderAndLive();
        renderPrList();
        renderRepos();
        renderBrainsSidebar();
        populateDocsRepoSelect();
        renderTenderLive();
        renderTenderPrList();
        renderTenderReposAndAgents();
      } catch (e) {
        console.error(e);
      }
    }

    function renderTopStatusBar() {
      if (!STATE) return;
      const revActive = (STATE.active_workers || []).length;
      const tendActive = (STATE.tender_active_workers || []).length;
      const watchedOpenCount = (STATE.tender_pull_requests || []).filter(p => p.is_open !== false && p.watched !== false).length;
      const revBadge = document.getElementById('nav-reviewer-badge');
      const tendBadge = document.getElementById('nav-tender-badge');
      revBadge.textContent = revActive ? `${revActive} live` : String((STATE.pull_requests || []).length);
      revBadge.className = `badge ${revActive ? 'b-running' : 'b-muted'}`;
      tendBadge.textContent = tendActive ? `${tendActive} live` : String(watchedOpenCount);
      tendBadge.className = `badge ${tendActive ? 'b-running' : 'b-muted'}`;

      const pmPill = document.getElementById('pill-poll-mode');
      const isTender = ACTIVE_APP === 'tender';
      const pollPaused = isTender ? Boolean(STATE.tender?.poll_only_mode) : Boolean(STATE.agents?.poll_only_mode);
      if (pollPaused) {
        pmPill.innerHTML = `<span class="dot warn"></span>Auto-AI Paused`;
      } else if (isTender && STATE.tender && !STATE.tender.enabled) {
        pmPill.innerHTML = `<span class="dot warn"></span>Manual Mode`;
      } else {
        pmPill.innerHTML = `<span class="dot ok"></span>2m Auto-Watch`;
      }

      const sysPill = document.getElementById('pill-system');
      if (sysPill) {
        const cOk = Boolean(STATE.runners?.claude_ready);
        const aOk = Boolean(STATE.runners?.agy_ready);
        const pOk = Boolean(STATE.host?.ac_power);
        const allOk = cOk && aOk && pOk;
        const pwrShort = pOk ? 'AC' : 'Battery';
        sysPill.innerHTML = `<span class="dot ${allOk ? 'ok' : 'warn'}"></span>claude ${cOk ? '✓' : '!'} · agy ${aOk ? '✓' : '!'} · ${pwrShort}`;
        sysPill.title = `${STATE.host?.summary || 'M1 Host'} | claude: ${cOk ? 'ready' : 'sign-in'} | agy: ${aOk ? 'ready' : 'sign-in'}`;
      }
      updatePixelWorkersSummary();
    }

    function renderHeaderAndLive() {
      if (!STATE) return;
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
        wb.innerHTML = `<div style="color:var(--muted);padding:8px 0;">Idle — zero in-flight PR review sessions right now. Next 2m poll or push will launch parallel async sessions automatically.</div>`;
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
      const prs = STATE?.pull_requests || [];
      document.getElementById('pr-count-badge').textContent = prs.length;
      if (!SELECTED_PR_KEY && prs.length) SELECTED_PR_KEY = prs[0].key;

      document.getElementById('pr-list-box').innerHTML = prs.map(p => {
        const vClass = p.verdict === 'PASS' ? 'b-pass' : (p.verdict === 'CHANGES_NEEDED' ? 'b-changes' : 'b-muted');
        const openCnt = (p.observations || []).filter(o => o.status === 'open').length;
        return `<div class="pr-item ${p.key === SELECTED_PR_KEY ? 'selected' : ''}" onclick="selectPr('${esc(p.key)}')">
          <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
            <strong>${esc(p.repo)} #${p.pr_number}</strong>
            <span class="badge ${vClass}">${esc(p.verdict)}</span>
          </div>
          <div style="font-size:12.5px;margin-top:4px;">${esc(p.title)}</div>
          <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">
            Runs: ${p.runs_count} • Open obs: ${openCnt} • Last reviewer: <code>${esc(p.last_reviewer_account || '-')}</code>
          </div>
        </div>`;
      }).join('') || '<div style="color:var(--muted);">No tracked PRs yet.</div>';

      const selected = prs.find(p => p.key === SELECTED_PR_KEY);
      const box = document.getElementById('pr-detail-box');
      if (!selected) return;

      const p = selected;
      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const obsAll = p.observations || [];
      const obsFiltered = obsAll.filter(o => {
        if (OBS_FILTER === 'open') return o.status === 'open';
        if (OBS_FILTER === 'blocking') return o.status === 'open' && (o.severity === 'CRITICAL' || o.severity === 'HIGH');
        if (OBS_FILTER === 'resolved') return o.status !== 'open';
        return true;
      });

      const obsHtml = obsFiltered.map(o => {
        const sevClass = o.severity === 'CRITICAL' ? 'b-critical' : (o.severity === 'HIGH' ? 'b-high' : (o.severity === 'MEDIUM' ? 'b-medium' : 'b-low'));
        const stClass = o.status === 'open' ? 'b-changes' : 'b-pass';
        const loc = o.line ? `${o.path}:${o.line}` : o.path;
        return `<div class="obs-card">
          <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
            <div>
              <span class="badge ${sevClass}">${esc(o.severity)}</span>
              <span class="badge ${stClass}">${esc(o.status.toUpperCase())}</span>
              <span class="badge b-muted">${esc(o.brain_id)}</span>
              <strong>${esc(o.title)}</strong>
            </div>
            <span style="font-size:11.5px;color:var(--muted);">opened run #${o.opened_in_run} • ${esc(o.origin_account)}</span>
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
            <button class="secondary" onclick="openPrInTender('${esc(p.repo)}', ${p.pr_number})">🛠️ Open in Code Tender</button>
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
          <td><span class="badge ${r.tender_enabled ? 'b-pass' : 'b-muted'}">${r.tender_enabled ? 'TENDER ON' : 'OFF'}</span></td>
          <td style="font-size:11.5px;">
            <div><code>${esc(r.local_repo_path || '(auto-detect)')}</code></div>
            <div style="color:var(--muted);"><code>${esc(r.local_check_cmd || '(auto-infer)')}</code></div>
          </td>
          <td style="display:flex;gap:6px;flex-wrap:wrap;">
            <button class="secondary" style="padding:4px 8px;font-size:11.5px;" onclick="loadRepoIntoForm('r', '${esc(r.full_name)}')">Edit</button>
            <button class="secondary" style="padding:4px 8px;font-size:11.5px;" onclick="toggleRepo('${esc(r.full_name)}')">${r.enabled ? 'Pause Rev' : 'Enable Rev'}</button>
            <button class="secondary" style="padding:4px 8px;font-size:11.5px;" onclick="toggleRepoTender('${esc(r.full_name)}')">${r.tender_enabled ? 'Pause Tend' : 'Enable Tend'}</button>
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

    function selectPr(key) { SELECTED_PR_KEY = key; renderPrList(); }
    function setObsFilter(f) { OBS_FILTER = f; renderPrList(); }

    function triggerPrimaryAction() {
      if (ACTIVE_APP === 'pixel') {
        loadPixelOffice(true);
      } else if (ACTIVE_APP === 'tender') {
        triggerTenderPollNow();
      } else {
        triggerPollNow();
      }
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
      switchReviewerTab('live');
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
      showToast('Saved reviewer settings!');
      refreshAll();
    }

    let M1_BROWSER_PREFIX = 'r';
    let M1_BROWSER_PARENT = null;

    function parseRepoSlugJs(raw) {
      let s = (raw || '').trim().split('?')[0].split('#')[0].replace(/\/+$/, '');
      if (!s) return '';
      if (s.includes('://')) {
        const idx = s.indexOf('://');
        const rest = s.slice(idx + 3);
        const slash = rest.indexOf('/');
        if (slash >= 0) s = rest.slice(slash + 1);
      } else if (s.includes(':') && !s.startsWith('/')) {
        s = s.slice(s.indexOf(':') + 1);
      } else if (s.startsWith('github.com/')) {
        s = s.slice('github.com/'.length);
      } else if (s.startsWith('www.github.com/')) {
        s = s.slice('www.github.com/'.length);
      }
      const parts = s.split('/').filter(Boolean);
      if (parts.length >= 2) {
        return parts[0] + '/' + parts[1].replace(/\.git$/i, '');
      }
      return s.replace(/\.git$/i, '');
    }

    async function normalizeRepoInputAndDetect(prefix) {
      const el = document.getElementById(prefix + '-name');
      const cleaned = parseRepoSlugJs(el.value);
      if (cleaned && cleaned !== el.value.trim()) {
        el.value = cleaned;
      }
      if (cleaned && cleaned.includes('/') && !document.getElementById(prefix + '-local-path').value.trim()) {
        await detectLocalRepo(prefix, true);
      }
    }

    function applyLocalGitInfo(prefix, info) {
      if (!info) return;
      if (info.path) {
        document.getElementById(prefix + '-local-path').value = info.path;
      }
      if (info.full_name) {
        document.getElementById(prefix + '-name').value = info.full_name;
      }
      if (info.default_branch) {
        document.getElementById(prefix + '-branch').value = info.default_branch;
      }
      if (info.inferred_check_cmd && !document.getElementById(prefix + '-check').value.trim()) {
        document.getElementById(prefix + '-check').value = info.inferred_check_cmd;
      }
    }

    async function loadGitInfoFromPath(prefix) {
      const path = document.getElementById(prefix + '-local-path').value.trim();
      if (!path) {
        showToast('Enter or browse an M1 folder path first');
        return;
      }
      try {
        const info = await api('/api/m1/git-info?path=' + encodeURIComponent(path));
        applyLocalGitInfo(prefix, info);
        showToast(info.full_name
          ? `Loaded ${info.full_name} (${info.default_branch}) from M1 path`
          : `Folder loaded (no GitHub origin remote found in .git/config)`);
      } catch (e) {
        showToast('Load failed: ' + e.message);
      }
    }

    async function detectLocalRepo(prefix, silentMiss = false) {
      const nameEl = document.getElementById(prefix + '-name');
      const repo = parseRepoSlugJs(nameEl.value);
      if (repo) nameEl.value = repo;
      if (!repo) {
        if (!silentMiss) showToast('Enter Owner / Repo or GitHub URL first');
        return;
      }
      try {
        const res = await api('/api/repos/detect-local?repo=' + encodeURIComponent(repo));
        if (res.local_repo_path) {
          document.getElementById(prefix + '-local-path').value = res.local_repo_path;
        }
        if (res.default_branch) {
          document.getElementById(prefix + '-branch').value = res.default_branch;
        }
        if (res.inferred_check_cmd && !document.getElementById(prefix + '-check').value.trim()) {
          document.getElementById(prefix + '-check').value = res.inferred_check_cmd;
        }
        if (!silentMiss || res.local_repo_path) {
          showToast(res.local_repo_path
            ? `Detected local clone: ${res.local_repo_path}`
            : `No local clone found on M1 for ${repo} (will use PR worktree fallback)`);
        }
      } catch (e) {
        if (!silentMiss) showToast('Detect failed: ' + e.message);
      }
    }

    async function openM1Browser(prefix) {
      M1_BROWSER_PREFIX = prefix;
      const modal = document.getElementById('m1-browser-modal');
      modal.classList.remove('hidden');
      modal.style.display = 'flex';
      const initial = document.getElementById(prefix + '-local-path').value.trim();
      await navigateM1Browser(initial);
    }

    function closeM1Browser() {
      const modal = document.getElementById('m1-browser-modal');
      modal.classList.add('hidden');
      modal.style.display = 'none';
    }

    function navigateM1Up() {
      if (M1_BROWSER_PARENT) navigateM1Browser(M1_BROWSER_PARENT);
    }

    async function navigateM1Browser(path) {
      const status = document.getElementById('m1-browser-status');
      status.textContent = 'Scanning M1 folder...';
      try {
        const q = path ? ('?path=' + encodeURIComponent(path)) : '';
        const d = await api('/api/m1/browse' + q);
        document.getElementById('m1-path-input').value = d.current_path || '';
        M1_BROWSER_PARENT = d.parent_path || null;
        document.getElementById('m1-up-btn').disabled = !M1_BROWSER_PARENT;

        const cur = d.current || {};
        status.innerHTML = cur.is_git_repo
          ? `<span class="badge b-pass">GIT REPO</span> <code>${esc(cur.full_name || cur.origin_url || 'no origin')}</code> • branch <code>${esc(cur.default_branch || 'main')}</code>`
          : `Browsing <code>${esc(d.current_path || '')}</code>`;

        document.getElementById('m1-discovered-list').innerHTML = (d.discovered_repos || []).map(r => `
          <div class="pr-item" style="margin:0;padding:8px 10px;" onclick="pickM1Folder('${esc(r.path.replace(/'/g, "\\'"))}')">
            <div style="display:flex;justify-content:space-between;gap:6px;align-items:center;">
              <strong>${esc(r.full_name || r.name)}</strong>
              <span class="badge b-pass">${esc(r.default_branch || 'main')}</span>
            </div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:2px;"><code>${esc(r.path)}</code></div>
          </div>
        `).join('') || '<div style="color:var(--muted);font-size:12px;">No git clones auto-discovered in standard roots; browse on the right.</div>';

        document.getElementById('m1-dir-list').innerHTML = (d.directories || []).map(dir => `
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;padding:6px 8px;border:1px solid var(--border);border-radius:7px;background:var(--panel-2);">
            <div style="cursor:pointer;flex:1;min-width:0;" onclick="navigateM1Browser('${esc(dir.path.replace(/'/g, "\\'"))}')">
              <div style="font-size:13px;font-weight:600;">
                ${dir.is_git_repo ? '📦' : '📁'} ${esc(dir.name)}
                ${dir.full_name ? `<span class="badge b-pass" style="margin-left:4px;">${esc(dir.full_name)}</span>` : ''}
              </div>
            </div>
            <button type="button" class="secondary" style="padding:4px 8px;font-size:11.5px;" onclick="pickM1Folder('${esc(dir.path.replace(/'/g, "\\'"))}')">✓ Select</button>
          </div>
        `).join('') || '<div style="color:var(--muted);font-size:12px;">Empty folder.</div>';
      } catch (e) {
        status.textContent = 'Error: ' + e.message;
      }
    }

    async function pickM1Folder(path) {
      try {
        const info = await api('/api/m1/git-info?path=' + encodeURIComponent(path));
        applyLocalGitInfo(M1_BROWSER_PREFIX, info);
        closeM1Browser();
        showToast(info.full_name
          ? `Loaded ${info.full_name} (${info.default_branch}) from M1!`
          : `Selected ${info.path} (enter Owner / Repo if no GitHub origin)`);
      } catch (e) {
        showToast('Could not read M1 folder: ' + e.message);
      }
    }

    function loadRepoIntoForm(prefix, fullName) {
      const r = (STATE.repos || []).find(x => x.full_name === fullName);
      if (!r) return;
      document.getElementById(prefix + '-name').value = r.full_name || '';
      document.getElementById(prefix + '-branch').value = r.default_branch || 'main';
      document.getElementById(prefix + '-enabled').checked = Boolean(r.enabled);
      document.getElementById(prefix + '-tender-enabled').checked = Boolean(r.tender_enabled);
      document.getElementById(prefix + '-local-path').value = r.local_repo_path || '';
      document.getElementById(prefix + '-check').value = r.local_check_cmd || '';
      document.getElementById(prefix + '-acct-pol').value = r.account_policy || 'swap';
      document.getElementById(prefix + '-forced').value = r.forced_account || '';
      document.getElementById(prefix + '-ev-pol').value = r.review_event_policy || 'formal_verdict';
      document.getElementById(prefix + '-drive').value = r.gdrive_folder_id || '';
      showToast(`Loaded ${r.full_name} into editor`);
    }

    async function submitWatchedRepoForm(prefix) {
      const nameEl = document.getElementById(prefix + '-name');
      const fullName = parseRepoSlugJs(nameEl.value);
      nameEl.value = fullName;
      if (!fullName || !fullName.includes('/')) {
        showToast('Enter Owner / Repo, a GitHub URL, or browse an M1 Git folder first');
        return;
      }
      const payload = {
        full_name: fullName,
        default_branch: document.getElementById(prefix + '-branch').value.trim() || 'main',
        enabled: document.getElementById(prefix + '-enabled').checked,
        tender_enabled: document.getElementById(prefix + '-tender-enabled').checked,
        local_repo_path: document.getElementById(prefix + '-local-path').value.trim() || null,
        local_check_cmd: document.getElementById(prefix + '-check').value.trim() || null,
        account_policy: document.getElementById(prefix + '-acct-pol').value,
        forced_account: document.getElementById(prefix + '-forced').value.trim() || null,
        review_event_policy: document.getElementById(prefix + '-ev-pol').value,
        gdrive_folder_id: document.getElementById(prefix + '-drive').value.trim() || null
      };
      try {
        await api('/api/repos/add', { method: 'POST', body: JSON.stringify(payload) });
        showToast(`Saved watched repo ${payload.full_name}`);
        refreshAll();
      } catch (e) {
        showToast('Save failed: ' + e.message);
      }
    }

    async function toggleRepo(full_name) {
      await api('/api/repos/toggle', { method: 'POST', body: JSON.stringify({ full_name }) });
      refreshAll();
    }

    async function toggleRepoTender(full_name) {
      await api('/api/repos/tender-toggle', { method: 'POST', body: JSON.stringify({ full_name }) });
      showToast(`Toggled Code Tender for ${full_name}`);
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

    // =========================================================
    // CODE TENDER RENDERERS & ACTIONS (SIMPLIFIED WATCH -> TEND)
    // =========================================================

    function isPrActivelyTending(repo, prNumber) {
      return (STATE?.tender_active_workers || []).some(
        w => String(w.repo || '').toLowerCase() === String(repo || '').toLowerCase() && w.pr_number === prNumber
      );
    }

    function classifyTenderPr(p) {
      const running = isPrActivelyTending(p.repo, p.pr_number);
      const isOpen = p.is_open !== false;
      const isWatched = p.watched !== false;
      const openFb = Math.max(Number(p.open_feedback_count || 0), (p.pending_items || []).length);

      if (running) {
        return { category: 'tending', badgeCls: 'b-running', badgeText: '⚡ Tending', sub: 'Fixing & running local checks...' };
      }
      if (!isOpen) {
        return { category: 'inactive', badgeCls: 'b-muted', badgeText: 'Closed', sub: 'PR closed or merged' };
      }
      if (!isWatched) {
        return { category: 'inactive', badgeCls: 'b-muted', badgeText: '⏸ Paused', sub: 'Watching paused for this PR' };
      }
      if (openFb > 0) {
        return {
          category: 'tending',
          badgeCls: 'b-running',
          badgeText: `💬 ${openFb} new`,
          sub: `${openFb} review comment${openFb === 1 ? '' : 's'} ready to tend`
        };
      }
      if (p.status_badge === 'CHECK_FAILED') {
        return { category: 'attention', badgeCls: 'b-failed', badgeText: '⚠️ Check Failed', sub: 'Local check failed — push blocked' };
      }
      if (p.status_badge === 'NEEDS_HUMAN') {
        return { category: 'attention', badgeCls: 'b-changes', badgeText: '👤 Needs Input', sub: 'Comment needs human decision' };
      }
      if (p.status_badge === 'STAGED_PATCH' || p.staged_patch) {
        return { category: 'attention', badgeCls: 'b-staged', badgeText: '📦 Ready to Push', sub: 'Verified fix waiting for approval' };
      }
      if (p.status_badge === 'FIXED_PUSHED') {
        const sha = p.last_commit_pushed ? p.last_commit_pushed.slice(0, 7) : '';
        return {
          category: 'handled',
          badgeCls: 'b-pass',
          badgeText: '✅ Fixed & Pushed',
          sub: sha ? `Pushed ${sha} · Watching for new reviews` : 'Fix pushed · Watching for new reviews'
        };
      }
      if (p.status_badge === 'REPLIED_ONLY') {
        return { category: 'handled', badgeCls: 'b-explained', badgeText: '💬 Replied', sub: 'Replied · Watching for new reviews' };
      }
      return { category: 'watching', badgeCls: 'b-watching', badgeText: '👀 Watching', sub: 'Listening for review comments' };
    }

    function renderTenderLive() {
      if (!STATE || !STATE.tender) return;
      const tc = STATE.tender;
      const active = STATE.tender_active_workers || [];

      if (!TENDER_SETTINGS_INITIALIZED) {
        document.getElementById('tcfg-enabled').value = String(Boolean(tc.enabled));
        document.getElementById('tcfg-delivery').value = tc.delivery_mode || 'auto_push';
        document.getElementById('tcfg-poll-only').value = String(Boolean(tc.poll_only_mode));
        document.getElementById('tcfg-intercalate').value = String(Boolean(tc.intercalate_providers));
        document.getElementById('tcfg-max-prs').value = tc.max_parallel_prs || 4;
        document.getElementById('tcfg-max-fixers').value = tc.max_parallel_fixers_per_pr || 4;
        document.getElementById('tcfg-max-cycles').value = tc.max_autonomous_cycles_per_pr || 3;
        document.getElementById('tcfg-resolve').value = String(Boolean(tc.resolve_threads_on_fix));
        document.getElementById('tcfg-ignore-pass').value = String(Boolean(tc.ignore_clean_pass_reviews));
        const defaultAuthors = [STATE.accounts?.primary, STATE.accounts?.secondary].filter(Boolean);
        const authors = (tc.allowed_authors && tc.allowed_authors.length) ? tc.allowed_authors : defaultAuthors;
        document.getElementById('tcfg-allowed-authors').value = authors.join(', ');
        TENDER_SETTINGS_INITIALIZED = true;
      }

      const modeEl = document.getElementById('tender-mode-summary');
      if (modeEl) {
        if (!tc.enabled || tc.poll_only_mode) {
          modeEl.innerHTML = `<span class="dot warn"></span>Manual Mode`;
        } else {
          const deliv = tc.delivery_mode === 'stage_patch_only' ? 'Stage Only' : 'Auto-Push';
          modeEl.innerHTML = `<span class="dot ok"></span>Auto-Tend · ${deliv}`;
        }
      }

      const banner = document.getElementById('tender-live-banner');
      if (banner) {
        if (!active.length) {
          banner.classList.add('hidden');
          banner.innerHTML = '';
        } else {
          banner.classList.remove('hidden');
          banner.innerHTML = `<div class="worker-grid">` + active.map(w => {
            const chips = Object.entries(w.workers || {}).map(([name, wState]) => {
              const rBadge = wState.runner === 'claude' ? 'b-claude' : (wState.runner === 'agy' ? 'b-agy' : 'b-muted');
              return `<div class="brain-chip ${esc(wState.status)}">
                <span>${esc(name)}</span>
                <span><span class="badge ${rBadge}">${esc(wState.runner)}</span> <span class="badge ${wState.status === 'running' ? 'b-running' : (wState.status === 'done' ? 'b-done' : 'b-muted')}">${esc(wState.status)}</span></span>
              </div>`;
            }).join('');
            return `<div class="worker-card" style="border-color:var(--tender);">
              <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
                <strong><span class="dot run"></span> Tending ${esc(w.repo)} #${w.pr_number}</strong>
                <span class="badge b-running">${esc(w.phase)} (${w.packets_done}/${w.packets_total})</span>
              </div>
              <div style="font-size:12.5px;margin-top:4px;">${esc(w.title)}</div>
              <div class="brain-chips">${chips}</div>
            </div>`;
          }).join('') + `</div>`;
        }
      }

      const logEl = document.getElementById('tender-logs-box');
      if (logEl) logEl.textContent = STATE.tender_logs || '(no Code Tender logs yet)';
      const histEl = document.getElementById('tender-history-tbody');
      if (histEl) {
        histEl.innerHTML = (STATE.tender_history || []).map(h => `
          <tr>
            <td><span class="badge ${h.status === 'pass' ? 'b-pass' : 'b-changes'}">${esc(h.status)}</span></td>
            <td><span class="badge b-muted">GraphQL (0 AI)</span></td>
            <td>${h.duration_secs}s</td>
            <td style="color:var(--muted);">${new Date(h.started_epoch_secs * 1000).toISOString().replace('T', ' ').slice(0, 19)}</td>
          </tr>
        `).join('');
      }
    }

    async function watchPrFromInput() {
      const inputEl = document.getElementById('watch-pr-input');
      const raw = (inputEl?.value || '').trim();
      if (!raw) {
        showToast('Paste a GitHub PR URL or owner/repo#123 first');
        inputEl?.focus();
        return;
      }
      try {
        const res = await api('/api/tender/watch-pr', {
          method: 'POST',
          body: JSON.stringify({ input: raw, watched: true })
        });
        inputEl.value = '';
        if (res.key) SELECTED_TENDER_PR_KEY = res.key;
        TENDER_PR_FILTER = 'all';
        showToast(res.message || `Watching ${res.key}`);
        await refreshAll();
      } catch (e) {
        showToast('Could not watch PR: ' + e.message);
      }
    }

    async function setPrWatched(repo, pr, watched) {
      try {
        const res = await api('/api/tender/watch-pr', {
          method: 'POST',
          body: JSON.stringify({ repo, pr, watched: Boolean(watched) })
        });
        if (watched && res.key) SELECTED_TENDER_PR_KEY = res.key;
        showToast(res.message || (watched ? `Watching ${repo}#${pr}` : `Paused ${repo}#${pr}`));
        if (OPEN_PR_PICKER_VISIBLE) {
          OPEN_PR_PICKER_ITEMS = OPEN_PR_PICKER_ITEMS.map(item =>
            (item.repo === repo && item.pr_number === pr) ? { ...item, watched: Boolean(watched) } : item
          );
          renderOpenPrPicker();
        }
        await refreshAll();
      } catch (e) {
        showToast('Watch update failed: ' + e.message);
      }
    }

    async function removeWatchedPr(repo, pr) {
      try {
        const res = await api('/api/tender/remove-pr', {
          method: 'POST',
          body: JSON.stringify({ repo, pr })
        });
        const key = `${repo}#${pr}`;
        if (SELECTED_TENDER_PR_KEY === key && !res.paused) {
          SELECTED_TENDER_PR_KEY = null;
        }
        showToast(res.message || `Removed ${key}`);
        await refreshAll();
      } catch (e) {
        showToast('Remove failed: ' + e.message);
      }
    }

    function toggleOpenPrPicker(forceState) {
      OPEN_PR_PICKER_VISIBLE = typeof forceState === 'boolean' ? forceState : !OPEN_PR_PICKER_VISIBLE;
      const box = document.getElementById('open-pr-picker-box');
      if (box) box.classList.toggle('hidden', !OPEN_PR_PICKER_VISIBLE);
      if (OPEN_PR_PICKER_VISIBLE) {
        loadOpenPrPicker();
      }
    }

    async function loadOpenPrPicker() {
      const listEl = document.getElementById('open-pr-picker-list');
      if (!listEl) return;
      OPEN_PR_PICKER_LOADING = true;
      listEl.innerHTML = `<div style="color:var(--muted);padding:8px 0;">Fetching live open Pull Requests from GitHub...</div>`;
      try {
        const res = await api('/api/tender/open-prs');
        OPEN_PR_PICKER_ITEMS = res.prs || [];
        OPEN_PR_PICKER_LOADING = false;
        renderOpenPrPicker();
        await refreshAll();
      } catch (e) {
        OPEN_PR_PICKER_LOADING = false;
        listEl.innerHTML = `<div style="color:var(--red);padding:8px 0;">Could not load open PRs: ${esc(e.message)}</div>`;
      }
    }

    function renderOpenPrPicker() {
      const listEl = document.getElementById('open-pr-picker-list');
      if (!listEl || OPEN_PR_PICKER_LOADING) return;
      if (!OPEN_PR_PICKER_ITEMS.length) {
        listEl.innerHTML = `<div style="color:var(--muted);padding:8px 0;">No open PRs found on your configured repositories. Paste any GitHub PR URL above or add a repository in Repos &amp; Settings.</div>`;
        return;
      }
      listEl.innerHTML = OPEN_PR_PICKER_ITEMS.map(item => {
        const watched = Boolean(item.watched);
        const fb = Number(item.open_feedback_count || 0);
        return `<div class="repo-card" style="margin-bottom:6px;padding:9px 12px;">
          <div style="min-width:0;flex:1;">
            <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
              <strong>${esc(item.repo)} #${item.pr_number}</strong>
              ${item.is_draft ? '<span class="badge b-muted">Draft</span>' : ''}
              ${fb > 0 ? `<span class="badge b-running">💬 ${fb} review comment${fb === 1 ? '' : 's'}</span>` : '<span class="badge b-muted">0 new comments</span>'}
            </div>
            <div style="font-size:13px;margin-top:2px;">${esc(item.title)}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:2px;">
              <code>${esc(item.head_ref)}</code> → <code>${esc(item.base_ref)}</code> · @${esc(item.author)}
            </div>
          </div>
          <div>
            ${watched
              ? `<button class="secondary" onclick="setPrWatched('${esc(item.repo)}', ${item.pr_number}, false)">✓ Watching</button>`
              : `<button class="tender-btn" onclick="setPrWatched('${esc(item.repo)}', ${item.pr_number}, true)">+ Watch PR</button>`
            }
          </div>
        </div>`;
      }).join('');
    }

    function renderTenderPrList() {
      const allPrs = STATE?.tender_pull_requests || [];
      const counts = { all: 0, watching: 0, tending: 0, attention: 0, handled: 0, inactive: 0 };
      const decorated = allPrs.map(p => {
        const meta = classifyTenderPr(p);
        counts[meta.category] = (counts[meta.category] || 0) + 1;
        if (meta.category !== 'inactive') counts.all++;
        return { p, meta };
      });

      const filterBar = document.getElementById('tender-pr-filters');
      if (filterBar) {
        const pill = (id, label, cnt) => {
          const active = TENDER_PR_FILTER === id;
          return `<button class="${active ? 'tender-btn' : 'secondary'}" style="padding:5px 11px;font-size:12px;" onclick="setTenderPrFilter('${id}')">${label} <span class="badge ${active ? 'b-pass' : 'b-muted'}" style="margin-left:3px;">${cnt}</span></button>`;
        };
        filterBar.innerHTML = [
          pill('all', 'Active Pulls', counts.all),
          pill('watching', '👀 Watching', counts.watching),
          pill('tending', '⚡ Tending', counts.tending),
          pill('attention', '⚠️ Needs Attention', counts.attention),
          pill('handled', '✅ Fixed & Pushed', counts.handled),
          counts.inactive > 0 ? pill('inactive', 'Paused / Closed', counts.inactive) : ''
        ].join('');
      }

      const filtered = decorated.filter(({ meta }) => {
        if (TENDER_PR_FILTER === 'all') return meta.category !== 'inactive';
        return meta.category === TENDER_PR_FILTER;
      });
      const displayList = filtered;
      const autoWatchedRepos = (STATE?.repos || []).filter(r => r.tender_enabled).map(r => r.full_name);

      document.getElementById('tender-pr-count-badge').textContent = displayList.length;
      if ((!SELECTED_TENDER_PR_KEY || !displayList.some(x => x.p.key === SELECTED_TENDER_PR_KEY)) && displayList.length) {
        SELECTED_TENDER_PR_KEY = displayList[0].p.key;
      }

      const listBox = document.getElementById('tender-pr-list-box');
      listBox.innerHTML = displayList.map(({ p, meta }) => `
        <div class="pr-item ${p.key === SELECTED_TENDER_PR_KEY ? 'selected-tender' : ''}" onclick="selectTenderPr('${esc(p.key)}')">
          <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
            <strong style="font-size:13px;">${esc(p.repo)} #${p.pr_number}</strong>
            <span class="badge ${meta.badgeCls}">${meta.badgeText}</span>
          </div>
          <div style="font-size:12.5px;margin-top:4px;line-height:1.35;">${esc(p.title)}</div>
          <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">${esc(meta.sub)}</div>
        </div>
      `).join('') || `<div style="color:var(--muted);padding:10px 0;font-size:13px;">${autoWatchedRepos.length ? `Watching <b>${esc(autoWatchedRepos.join(', '))}</b> — 0 open PRs right now.` : `No active pulls in this filter.`} ${counts.inactive > 0 ? `<div style="margin-top:6px;"><button class="secondary" style="padding:4px 9px;font-size:11.5px;" onclick="setTenderPrFilter('inactive')">View ${counts.inactive} Merged / Closed PR${counts.inactive === 1 ? '' : 's'}</button></div>` : ''}</div>`;

      const selectedEntry = displayList.find(d => d.p.key === SELECTED_TENDER_PR_KEY);
      const box = document.getElementById('tender-pr-detail-box');
      if (document.activeElement && document.activeElement.classList && document.activeElement.classList.contains('tender-inline-reply') && box.dataset.prKey === SELECTED_TENDER_PR_KEY) {
        return;
      }
      box.dataset.prKey = SELECTED_TENDER_PR_KEY || '';
      if (!selectedEntry) {
        box.innerHTML = autoWatchedRepos.length ? `
          <div class="status-banner watching" style="margin-top:0;">
            <strong style="color:#6ee7b7;">👀 Watching ${esc(autoWatchedRepos.join(', '))} for Pull Requests</strong>
            <div style="font-size:13px;color:var(--muted);margin-top:6px;line-height:1.5;">
              Polled every 2 minutes on your headless M1 (0 AI tokens while idle). Currently <b>0 open pull requests</b> on GitHub — as soon as a new PR opens and receives review comments, Code Tender will automatically watch it, fix comments in your local repo context, run checks, and push.
            </div>
            ${counts.inactive > 0 ? `<div style="margin-top:10px;"><button class="secondary" onclick="setTenderPrFilter('inactive')">Inspect ${counts.inactive} Previously Merged / Closed PRs</button></div>` : ''}
          </div>` : `<div style="color:var(--muted);padding:16px 0;">Select a Pull Request on the left, or click <b>Pick Open PR</b> above to watch an open pull request.</div>`;
        return;
      }

      const { p, meta } = selectedEntry;
      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const isWatched = p.watched !== false;
      const isOpen = p.is_open !== false;
      const running = isPrActivelyTending(p.repo, p.pr_number);
      const pending = p.pending_items || [];
      const handledAll = p.handled_items || [];
      const openFb = Math.max(Number(p.open_feedback_count || 0), pending.length);

      // 3-Step Lifecycle Bar: 1. Watch Pull -> 2. Review Comments -> 3. Fix & Push
      const step1Cls = !isOpen ? '' : (isWatched ? 'done' : 'warn');
      const step1Sub = !isOpen ? 'PR closed/merged' : (isWatched ? 'Polling every 2m (0 AI tokens)' : 'Paused by user');

      let step2Cls = '';
      let step2Sub = 'Waiting for review comments';
      if (running || openFb > 0) {
        step2Cls = 'active';
        step2Sub = running ? 'Triaging & fixing comments now' : `${openFb} new comment${openFb === 1 ? '' : 's'} detected`;
      } else if (handledAll.length > 0) {
        step2Cls = 'done';
        step2Sub = `${handledAll.length} comment${handledAll.length === 1 ? '' : 's'} processed`;
      }

      let step3Cls = '';
      let step3Sub = 'Standby until comments arrive';
      if (running) {
        step3Cls = 'active';
        step3Sub = 'Running local check gate...';
      } else if (p.status_badge === 'CHECK_FAILED') {
        step3Cls = 'warn';
        step3Sub = 'Local check failed — push blocked';
      } else if (p.status_badge === 'NEEDS_HUMAN') {
        step3Cls = 'warn';
        step3Sub = 'Waiting for human decision';
      } else if (p.status_badge === 'STAGED_PATCH') {
        step3Cls = 'active';
        step3Sub = 'Verified patch ready to push';
      } else if (p.status_badge === 'FIXED_PUSHED') {
        step3Cls = 'done';
        step3Sub = p.last_commit_pushed ? `Pushed commit ${p.last_commit_pushed.slice(0, 7)}` : 'Verified fix pushed';
      } else if (p.status_badge === 'REPLIED_ONLY') {
        step3Cls = 'done';
        step3Sub = 'Replied on PR threads';
      }

      // Single Clear Contextual Status / Action Banner
      let statusBannerHtml = '';
      if (!isOpen) {
        statusBannerHtml = `
          <div class="status-banner">
            <strong>✓ Pull Request Merged / Closed</strong>
            <div style="font-size:12.5px;color:var(--muted);margin-top:4px;">This pull request is closed on GitHub. Historical tended comments and replies are preserved below.</div>
          </div>`;
      } else if (running) {
        statusBannerHtml = `
          <div class="status-banner watching">
            <strong>⚡ Actively Tending This Pull Request</strong>
            <div style="font-size:12.5px;color:var(--muted);margin-top:4px;">Agents are applying fixes in your local repo context and running verification checks before pushing.</div>
          </div>`;
      } else if (p.status_badge === 'CHECK_FAILED') {
        statusBannerHtml = `
          <div class="status-banner failed">
            <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap;">
              <div style="flex:1;min-width:220px;">
                <strong style="color:#fca5a5;">⚠️ Fix Drafted, But Local Verification Check Failed</strong>
                <div style="font-size:12.5px;margin-top:4px;">${esc(p.last_summary || 'Auto-push was blocked because the local check command exited non-zero.')}</div>
                ${p.last_check_cmd_used ? `<div style="font-size:11.5px;color:var(--muted);margin-top:4px;">Check command: <code>${esc(p.last_check_cmd_used)}</code></div>` : ''}
              </div>
              <div style="display:flex;gap:8px;flex-wrap:wrap;">
                <button class="tender-btn" onclick="triggerPrTend('${esc(p.repo)}', ${p.pr_number}, false)">🔄 Retry Tend</button>
                ${p.staged_patch ? `<button class="secondary" onclick="approveAndPushPatch('${esc(p.repo)}', ${p.pr_number})">Push Draft Anyway</button>` : ''}
                ${p.staged_patch ? `<button class="danger" onclick="discardStagedPatch('${esc(p.repo)}', ${p.pr_number})">Discard Draft</button>` : ''}
              </div>
            </div>
            ${p.staged_patch ? `<details class="clean-acc" style="margin-top:10px;background:var(--bg);"><summary style="padding:8px 12px;font-size:12px;">Inspect Drafted Diff (<code>${esc(p.staged_commit_message || 'fix: address review feedback')}</code>)</summary><div class="acc-body" style="padding:10px;"><pre style="margin:0;">${esc(p.staged_patch)}</pre></div></details>` : ''}
          </div>`;
      } else if (p.staged_patch) {
        statusBannerHtml = `
          <div class="status-banner staged">
            <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
              <div>
                <strong style="color:#fcd34d;">📦 Verified Fix Ready to Push</strong>
                <div style="font-size:12.5px;color:var(--muted);margin-top:3px;">Commit: <code>${esc(p.staged_commit_message || 'fix: address code review feedback')}</code></div>
              </div>
              <div style="display:flex;gap:8px;flex-wrap:wrap;">
                <button class="tender-btn" onclick="approveAndPushPatch('${esc(p.repo)}', ${p.pr_number})">✅ Approve &amp; Push</button>
                <button class="danger" onclick="discardStagedPatch('${esc(p.repo)}', ${p.pr_number})">Discard</button>
              </div>
            </div>
            <details class="clean-acc" style="margin-top:10px;background:var(--bg);"><summary style="padding:8px 12px;font-size:12px;">View Verified Patch Diff</summary><div class="acc-body" style="padding:10px;"><pre style="margin:0;">${esc(p.staged_patch)}</pre></div></details>
          </div>`;
      } else if (openFb > 0) {
        statusBannerHtml = `
          <div class="status-banner watching">
            <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
              <div>
                <strong>💬 ${openFb} New Review Comment${openFb === 1 ? '' : 's'} Ready to Tend</strong>
                <div style="font-size:12.5px;color:var(--muted);margin-top:3px;">Will auto-tend on the next 2m cycle, or click Tend Now to fix immediately.</div>
              </div>
              <button class="tender-btn" onclick="triggerPrTend('${esc(p.repo)}', ${p.pr_number}, false)">🛠️ Tend Comments Now</button>
            </div>
          </div>`;
      } else if (p.status_badge === 'FIXED_PUSHED') {
        statusBannerHtml = `
          <div class="status-banner pushed">
            <strong style="color:#6ee7b7;">✅ Verified Fix Pushed${p.last_commit_pushed ? ` (<code>${esc(p.last_commit_pushed.slice(0, 7))}</code>)` : ''}</strong>
            <div style="font-size:12.5px;margin-top:4px;">${esc(p.last_summary || 'Review comments were fixed, verified locally, and pushed to the PR branch.')}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">Still watching this PR — if new review comments land, Code Tender will tend them automatically.</div>
          </div>`;
      } else if (p.status_badge === 'NEEDS_HUMAN') {
        statusBannerHtml = `
          <div class="status-banner staged">
            <strong style="color:#fcd34d;">👤 Comment Needs Your Decision</strong>
            <div style="font-size:12.5px;margin-top:4px;">${esc(p.last_summary || 'One or more review comments require a product or architecture decision before editing code.')}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">Reply or create a follow-up issue directly inside the comment card below — Code Tender will post it on the GitHub thread, resolve it, and clear Needs Input.</div>
          </div>`;
      } else if (!isWatched) {
        statusBannerHtml = `
          <div class="status-banner">
            <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
              <div>
                <strong>⏸ Watching Paused</strong>
                <div style="font-size:12.5px;color:var(--muted);margin-top:3px;">New review comments on this PR are ignored until you resume watching.</div>
              </div>
              <button class="tender-btn" onclick="setPrWatched('${esc(p.repo)}', ${p.pr_number}, true)">▶ Resume Watching</button>
            </div>
          </div>`;
      } else {
        statusBannerHtml = `
          <div class="status-banner watching">
            <strong>👀 Watching Pull Request for Review Comments</strong>
            <div style="font-size:12.5px;color:var(--muted);margin-top:4px;">
              Checked every 2 minutes via lightweight GraphQL (0 AI tokens). As soon as review comments land, Code Tender will triage, fix in your local repo context, verify checks, and push.
            </div>
            ${p.last_summary ? `<div style="font-size:12px;margin-top:6px;color:#cbd5e1;">Last note: ${esc(p.last_summary)}</div>` : ''}
          </div>`;
      }

      const pendingHtml = pending.length ? `
        <h3 style="margin-top:16px;">Pending Review Comments (${pending.length})</h3>
        ${pending.map(item => {
          const loc = item.path ? (item.line ? `${item.path}:${item.line}` : item.path) : item.kind;
          const cleaned = cleanReviewBody(item.body);
          return `<div class="obs-card">
            <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
              <div><span class="badge b-running">Pending</span> <strong>@${esc(item.author)}</strong> · <code>${esc(loc)}</code></div>
              <span style="font-size:11.5px;color:var(--muted);">${esc((item.created_at || '').replace('T', ' ').slice(0, 16))}</span>
            </div>
            <div style="margin-top:6px;white-space:pre-wrap;font-size:13px;">${esc(cleaned)}</div>
          </div>`;
        }).join('')}` : '';

      const handledFiltered = handledAll.filter(h => {
        if (TENDER_ITEM_FILTER === 'fixed') return h.action === 'fixed' || h.action === 'already_addressed';
        if (TENDER_ITEM_FILTER === 'explained') return h.action === 'explained_rationale' || h.action === 'created_issue';
        if (TENDER_ITEM_FILTER === 'human') return h.action === 'needs_human';
        return true;
      });

      const handledSectionHtml = handledAll.length ? `
        <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap;margin-top:16px;margin-bottom:8px;">
          <h3 style="margin:0;">Tended Comments &amp; Replies (${handledAll.length})</h3>
          ${handledAll.length > 1 ? `
            <div style="display:flex;gap:5px;flex-wrap:wrap;">
              <button class="${TENDER_ITEM_FILTER === 'all' ? 'tender-btn' : 'secondary'}" style="padding:3px 9px;font-size:11.5px;" onclick="setTenderItemFilter('all')">All (${handledAll.length})</button>
              <button class="${TENDER_ITEM_FILTER === 'fixed' ? 'tender-btn' : 'secondary'}" style="padding:3px 9px;font-size:11.5px;" onclick="setTenderItemFilter('fixed')">Fixed (${handledAll.filter(h => h.action === 'fixed' || h.action === 'already_addressed').length})</button>
              <button class="${TENDER_ITEM_FILTER === 'explained' ? 'tender-btn' : 'secondary'}" style="padding:3px 9px;font-size:11.5px;" onclick="setTenderItemFilter('explained')">Addressed / Issue (${handledAll.filter(h => h.action === 'explained_rationale' || h.action === 'created_issue').length})</button>
              <button class="${TENDER_ITEM_FILTER === 'human' ? 'tender-btn' : 'secondary'}" style="padding:3px 9px;font-size:11.5px;" onclick="setTenderItemFilter('human')">Needs Input (${handledAll.filter(h => h.action === 'needs_human').length})</button>
            </div>` : ''}
        </div>
        ${handledFiltered.map((h, idx) => {
          const isFixed = h.action === 'fixed' || h.action === 'already_addressed';
          const isIssue = h.action === 'created_issue';
          const isExplained = h.action === 'explained_rationale' || isIssue;
          const aClass = isFixed ? 'b-fixed' : (isExplained ? 'b-explained' : 'b-changes');
          const aLabel = h.action === 'fixed'
            ? 'Fixed'
            : (h.action === 'already_addressed'
              ? 'Already Addressed'
              : (isIssue
                ? (h.issue_number ? `📌 Issue #${h.issue_number}` : '📌 Issue Created')
                : (h.action === 'explained_rationale' ? 'Explained' : 'Needs Input')));
          const loc = h.path ? (h.line ? `${h.path}:${h.line}` : h.path) : 'PR thread';
          const draftKey = `${p.key}::${h.feedback_id}`;
          const draftVal = TENDER_REPLY_DRAFTS[draftKey] ?? '';
          const inputId = `tender-reply-${idx}`;
          const issuePill = (h.issue_number && h.issue_url)
            ? `<a href="${esc(h.issue_url)}" target="_blank" rel="noopener" class="badge b-explained" style="text-decoration:none;">↗ #${h.issue_number}</a>`
            : '';
          const respondBar = h.action === 'needs_human' ? `
            <div style="margin-top:10px;padding-top:9px;border-top:1px solid var(--border);display:flex;gap:6px;flex-wrap:wrap;align-items:center;">
              <input id="${inputId}" class="tender-inline-reply" data-draft-key="${esc(draftKey)}" value="${esc(draftVal)}"
                oninput="TENDER_REPLY_DRAFTS[this.dataset.draftKey] = this.value"
                onkeydown="if(event.key==='Enter' && !event.shiftKey){event.preventDefault();respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', false);}"
                placeholder="Write reply addressing concern (or note for follow-up issue)..."
                style="flex:1;min-width:220px;padding:7px 10px;font-size:12.5px;" />
              <button class="tender-btn" style="padding:6px 11px;font-size:12px;" onclick="respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', false)">💬 Reply &amp; Resolve</button>
              <button class="secondary" style="padding:6px 11px;font-size:12px;" onclick="respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', true)">📌 Create Issue &amp; Resolve</button>
            </div>` : '';
          return `<div class="obs-card">
            <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
              <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;">
                <span class="badge ${aClass}">${aLabel}</span>
                ${issuePill}
                <strong>@${esc(h.source_author)}</strong>
                <code style="font-size:11.5px;color:#93c5fd;">${esc(loc)}</code>
                ${h.resolved_thread ? '<span class="badge b-pass">✓ Resolved</span>' : ''}
              </div>
              ${h.commit_sha ? `<code style="font-size:11.5px;color:var(--muted);">${esc(h.commit_sha.slice(0, 7))}</code>` : ''}
            </div>
            ${(() => { const cs = cleanReviewBody(h.summary); return cs ? `<div style="margin-top:6px;font-size:13px;">${esc(cs)}</div>` : ''; })()}
            ${(() => { const cr = cleanReviewBody(h.reply_text); return cr ? `<div style="margin-top:6px;font-size:12.5px;color:#a7f3d0;">↳ ${esc(cr)}</div>` : ''; })()}
            ${respondBar}
          </div>`;
        }).join('')}` : '';

      const learningList = p.learned_rules || [];
      const metaDetailsHtml = (learningList.length || p.local_context_used || p.last_check_cmd_used) ? `
        <details class="clean-acc" style="background:var(--panel-2);">
          <summary style="padding:9px 12px;font-size:12.5px;">
            <span>Context, Local Check Gate${learningList.length ? ` &amp; ${learningList.length} Learned Rule${learningList.length === 1 ? '' : 's'}` : ''}</span>
          </summary>
          <div class="acc-body" style="padding:10px 12px;font-size:12px;">
            <div style="color:var(--muted);">
              Local Context: <code>${esc(p.local_context_used || 'auto-detected on next run')}</code> ·
              Check Gate: <code>${esc(p.last_check_cmd_used || 'auto-inferred on next run')}</code> ·
              Cycles Run: <code>${p.runs_count || 0}</code>
            </div>
            ${learningList.map(lr => `
              <div style="border-top:1px solid var(--border);margin-top:8px;padding-top:8px;">
                <span class="badge b-pass">${esc(lr.scope)}</span> <code>${esc(lr.target)}</code> — ${esc(lr.rule)}
              </div>
            `).join('')}
          </div>
        </details>` : '';

      box.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap;">
          <div style="min-width:0;flex:1;">
            <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
              <h2 style="margin:0;">${esc(p.repo)} #${p.pr_number} — ${esc(p.title)}</h2>
              <span class="badge ${meta.badgeCls}">${meta.badgeText}</span>
            </div>
            <div style="font-size:12px;color:var(--muted);margin-top:4px;">
              ${p.head_ref ? `<code>${esc(p.head_ref)}</code> → <code>${esc(p.base_ref)}</code> · ` : ''}
              ${p.author ? `@${esc(p.author)} · ` : ''}
              Head: <code>${esc(((p.head_sha || p.last_tended_sha || '-') + '').slice(0, 7))}</code>
            </div>
          </div>
          <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center;">
            <a class="btn secondary" href="${esc(ghUrl)}" target="_blank" rel="noopener">↗ GitHub</a>
            ${isWatched
              ? `<button class="secondary" onclick="setPrWatched('${esc(p.repo)}', ${p.pr_number}, false)">⏸ Pause</button>`
              : `<button class="tender-btn" onclick="setPrWatched('${esc(p.repo)}', ${p.pr_number}, true)">▶ Watch</button>`
            }
            <button class="tender-btn" onclick="triggerPrTend('${esc(p.repo)}', ${p.pr_number}, false)">🛠️ Tend Now</button>
            <button class="secondary" title="Remove from list" onclick="removeWatchedPr('${esc(p.repo)}', ${p.pr_number})">✕</button>
          </div>
        </div>

        <div class="stepper">
          <div class="step-node ${step1Cls}">
            <div class="step-title">1. Watch Pull</div>
            <div class="step-sub">${esc(step1Sub)}</div>
          </div>
          <div class="step-node ${step2Cls}">
            <div class="step-title">2. Review Comments</div>
            <div class="step-sub">${esc(step2Sub)}</div>
          </div>
          <div class="step-node ${step3Cls}">
            <div class="step-title">3. Tend &amp; Push</div>
            <div class="step-sub">${esc(step3Sub)}</div>
          </div>
        </div>

        ${statusBannerHtml}
        ${pendingHtml}
        ${handledSectionHtml}
        ${metaDetailsHtml}
      `;
    }

    function renderTenderReposAndAgents() {
      if (!STATE) return;
      const reposList = document.getElementById('tender-repos-list');
      if (reposList) {
        reposList.innerHTML = (STATE.repos || []).map(r => `
          <div class="repo-card">
            <div style="min-width:0;flex:1;">
              <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
                <strong>${esc(r.full_name)}</strong>
                <code style="font-size:11.5px;color:var(--muted);">${esc(r.default_branch)}</code>
              </div>
              <div style="font-size:12px;color:var(--muted);margin-top:3px;">
                Local M1: <code>${esc(r.local_repo_path || 'auto-detect')}</code> · Check: <code>${esc(r.local_check_cmd || 'auto-infer')}</code>
              </div>
            </div>
            <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center;">
              <button class="${r.tender_enabled ? 'tender-btn' : 'secondary'}" style="padding:5px 10px;font-size:11.5px;" onclick="toggleRepoTender('${esc(r.full_name)}')">
                ${r.tender_enabled ? '✓ Auto-Watch PRs' : 'Auto-Watch Off'}
              </button>
              <button class="secondary" style="padding:5px 9px;font-size:11.5px;" onclick="toggleRepo('${esc(r.full_name)}')">
                ${r.enabled ? '✓ Reviewer On' : 'Reviewer Off'}
              </button>
              <button class="secondary" style="padding:5px 9px;font-size:11.5px;" onclick="loadRepoIntoForm('tr', '${esc(r.full_name)}')">Edit</button>
              <button class="danger" style="padding:5px 9px;font-size:11.5px;" onclick="removeRepo('${esc(r.full_name)}')">✕</button>
            </div>
          </div>
        `).join('') || `<div style="color:var(--muted);font-size:13px;">No repositories configured yet. Add one on the right or paste a PR URL on Watched Pulls.</div>`;
      }

      const agentsBox = document.getElementById('tender-agents-list-box');
      if (agentsBox) {
        const agents = STATE.tender_agents || ['01-tender-triage', '02-tender-fixer', '03-tender-verifier'];
        agentsBox.innerHTML = agents.map(a => `
          <div class="pr-item ${a === SELECTED_TENDER_AGENT ? 'selected-tender' : ''}" onclick="loadTenderAgentDetail('${esc(a)}')">
            <strong>${esc(a)}</strong>
          </div>
        `).join('');
      }
    }

    async function loadTenderAgentDetail(agentId) {
      SELECTED_TENDER_AGENT = agentId;
      renderTenderReposAndAgents();
      document.getElementById('tender-agent-detail-title').textContent = `Tender Agent: ${agentId}`;
      const d = await api('/api/tender-agent?id=' + encodeURIComponent(agentId));
      document.getElementById('ta-prompt').value = d.prompt || '';
      document.getElementById('ta-notes').value = d.notes || '';
      document.getElementById('ta-lessons').value = d.lessons || '';
    }

    async function saveTenderAgentPrompt() {
      const prompt = document.getElementById('ta-prompt').value;
      const notes = document.getElementById('ta-notes').value;
      const lessons = document.getElementById('ta-lessons').value;
      await api('/api/tender-agent/save', {
        method: 'POST',
        body: JSON.stringify({
          agent: SELECTED_TENDER_AGENT,
          prompt,
          notes,
          lessons
        })
      });
      showToast(`Saved prompt, notes & lessons for ${SELECTED_TENDER_AGENT}`);
    }

    async function loadTenderKb() {
      const kb = await api('/api/tender-kb');
      document.getElementById('tkb-patterns').value = kb.patterns_fixed || '';
      document.getElementById('tkb-recipes').value = kb.check_recipes || '';
      document.getElementById('tkb-dnr').value = kb.do_not_repeat || '';
    }

    async function saveTenderKbFile(fileKey, elId) {
      const content = document.getElementById(elId).value;
      await api('/api/tender-kb/save', {
        method: 'POST',
        body: JSON.stringify({ file_key: fileKey, content })
      });
      showToast(`Saved ${fileKey}.md`);
    }

    async function submitTenderKbRule() {
      const target = document.getElementById('tkb-target').value;
      const target_agent = document.getElementById('tkb-agent').value;
      const rule = document.getElementById('tkb-rule').value.trim();
      if (!rule) return;
      const res = await api('/api/tender-kb/add', {
        method: 'POST',
        body: JSON.stringify({ target, target_agent, rule })
      });
      document.getElementById('tkb-rule').value = '';
      showToast(res.added ? `Appended rule to Code Tender KB (${target})` : 'Rule already present (deduplicated)');
      loadTenderKb();
      if (target === 'agent-lesson') {
        loadTenderAgentDetail(SELECTED_TENDER_AGENT);
      }
    }

    function selectTenderPr(key) { SELECTED_TENDER_PR_KEY = key; renderTenderPrList(); }
    function setTenderPrFilter(f) { TENDER_PR_FILTER = f; renderTenderPrList(); }
    function setTenderItemFilter(f) { TENDER_ITEM_FILTER = f; renderTenderPrList(); }

    async function triggerTenderPollNow() {
      const res = await api('/api/actions/tend-poll', { method: 'POST', body: '{}' });
      showToast(res.message || 'Syncing open pulls & review comments...');
      setTimeout(refreshAll, 1100);
    }

    async function triggerPrTend(repo, pr, stageOnly) {
      const res = await api('/api/actions/tend', {
        method: 'POST',
        body: JSON.stringify({ repo, pr, stage_only: Boolean(stageOnly) })
      });
      SELECTED_TENDER_PR_KEY = `${repo}#${pr}`;
      switchTenderTab('tender-prs');
      showToast(res.message || `Started tending ${repo}#${pr}`);
      setTimeout(refreshAll, 900);
    }

    async function approveAndPushPatch(repo, pr) {
      const res = await api('/api/actions/tend/approve-push', {
        method: 'POST',
        body: JSON.stringify({ repo, pr })
      });
      showToast(res.message || `Pushed fix for ${repo}#${pr}!`);
      refreshAll();
    }

    async function discardStagedPatch(repo, pr) {
      const res = await api('/api/actions/tend/discard-patch', {
        method: 'POST',
        body: JSON.stringify({ repo, pr })
      });
      showToast(res.message || `Discarded draft patch for ${repo}#${pr}`);
      refreshAll();
    }

    async function respondTenderItem(repo, pr, feedbackId, inputId, createIssue) {
      const inputEl = document.getElementById(inputId);
      const reply_text = (inputEl?.value || '').trim();
      if (!reply_text && !createIssue) {
        showToast('Write a reply first, or click Create Issue & Resolve');
        inputEl?.focus();
        return;
      }
      if (inputEl) inputEl.blur();
      try {
        const res = await api('/api/tender/respond-item', {
          method: 'POST',
          body: JSON.stringify({
            repo,
            pr,
            feedback_id: feedbackId,
            reply_text,
            create_issue: Boolean(createIssue)
          })
        });
        delete TENDER_REPLY_DRAFTS[`${repo}#${pr}::${feedbackId}`];
        showToast(res.message || 'Posted reply & resolved thread!');
        await refreshAll();
      } catch (e) {
        showToast('Could not respond on thread: ' + e.message);
      }
    }

    async function saveTenderSettings() {
      const rawAuthors = document.getElementById('tcfg-allowed-authors').value || '';
      const allowed_authors = rawAuthors.split(',').map(s => s.trim()).filter(Boolean);
      await api('/api/settings/tender', {
        method: 'POST',
        body: JSON.stringify({
          enabled: document.getElementById('tcfg-enabled').value === 'true',
          delivery_mode: document.getElementById('tcfg-delivery').value,
          poll_only_mode: document.getElementById('tcfg-poll-only').value === 'true',
          intercalate_providers: document.getElementById('tcfg-intercalate').value === 'true',
          max_parallel_prs: parseInt(document.getElementById('tcfg-max-prs').value || '4', 10),
          max_parallel_fixers_per_pr: parseInt(document.getElementById('tcfg-max-fixers').value || '4', 10),
          max_autonomous_cycles_per_pr: parseInt(document.getElementById('tcfg-max-cycles').value || '3', 10),
          resolve_threads_on_fix: document.getElementById('tcfg-resolve').value === 'true',
          ignore_clean_pass_reviews: document.getElementById('tcfg-ignore-pass').value === 'true',
          allowed_authors
        })
      });
      TENDER_SETTINGS_INITIALIZED = false;
      showToast('Saved Code Tender settings!');
      refreshAll();
    }

    // =========================================================
    // PIXEL AGENTS VISUAL OFFICE & LIVE FALLBACK CANVAS
    // =========================================================

    async function loadPixelOffice(forceReload) {
      PIXEL_LOADED_ONCE = true;
      const badge = document.getElementById('pixel-conn-badge');
      const navBadge = document.getElementById('nav-pixel-badge');
      const frame = document.getElementById('pixelOfficeFrame');
      const fallback = document.getElementById('pixelFallbackPane');
      const ext = document.getElementById('pixel-open-ext');

      try {
        const info = await api('/api/pixel-office');
        const port = info.pixel_port || 7421;
        const host = window.location.hostname || '127.0.0.1';
        const tok = info.token ? ('?token=' + encodeURIComponent(info.token)) : '';
        const url = `http://${host}:${port}/${tok}`;
        ext.href = url;

        let browserReachable = false;
        if (info.available) {
          const ctrl = new AbortController();
          const timer = setTimeout(() => ctrl.abort(), 1400);
          try {
            await fetch(`http://${host}:${port}/`, { mode: 'no-cors', signal: ctrl.signal });
            browserReachable = true;
          } catch (_) {
            browserReachable = false;
          } finally {
            clearTimeout(timer);
          }
        }

        if (info.available && browserReachable) {
          PIXEL_ONLINE = true;
          badge.textContent = `LIVE :${port}`;
          badge.className = 'badge b-pass';
          navBadge.textContent = 'LIVE';
          navBadge.className = 'badge b-pass';
          if (forceReload || PIXEL_OFFICE_URL !== url || frame.classList.contains('hidden')) {
            PIXEL_OFFICE_URL = url;
            frame.src = url;
          }
          frame.classList.remove('hidden');
          fallback.classList.add('hidden');
          return;
        }

        PIXEL_ONLINE = false;
        frame.classList.add('hidden');
        fallback.classList.remove('hidden');
        if (info.available && !browserReachable) {
          badge.textContent = `Remote :${port} (Needs Tunnel)`;
          badge.className = 'badge b-running';
          navBadge.textContent = '7421';
          navBadge.className = 'badge b-running';
          document.getElementById('pixel-fallback-title').textContent = 'Pixel Agents Running on M1 — Forward Port 7421 to View Full Office';
          document.getElementById('pixel-fallback-desc').innerHTML =
            `<code>pixel-agents v1.6.0</code> is live on M1 at <code>127.0.0.1:${port}</code>. Run <code>code-reviewer ui</code> or forward <code>-L 127.0.0.1:${port}:127.0.0.1:${port}</code> on your SSH tunnel, then click Retry. Showing live M1 floorplan preview meanwhile:`;
        } else {
          badge.textContent = 'Offline';
          badge.className = 'badge b-muted';
          navBadge.textContent = 'OFF';
          navBadge.className = 'badge b-muted';
          document.getElementById('pixel-fallback-title').textContent = 'Pixel Agents Standalone Server Not Running';
          document.getElementById('pixel-fallback-desc').textContent =
            'Click "Start Pixel Agents Server (:7421)" to launch the live pixel-agents office on your M1 host.';
        }
      } catch (e) {
        PIXEL_ONLINE = false;
        badge.textContent = 'Offline';
        badge.className = 'badge b-muted';
      }
    }

    async function startPixelAgentsServer() {
      try {
        const res = await api('/api/pixel-office/start', { method: 'POST', body: '{}' });
        showToast(res.message || 'Started pixel-agents server!');
        setTimeout(() => loadPixelOffice(true), 800);
      } catch (e) {
        showToast('Could not start pixel-agents: ' + e.message);
      }
    }

    function updatePixelWorkersSummary() {
      if (!STATE) return;
      const rev = (STATE.active_workers || []).length;
      const tend = (STATE.tender_active_workers || []).length;
      const el = document.getElementById('pixel-workers-pill');
      if (el) {
        el.textContent = `${rev} Reviewer • ${tend} Code Tender active`;
      }
      if (!PIXEL_ONLINE) {
        const navBadge = document.getElementById('nav-pixel-badge');
        if (navBadge && navBadge.textContent === '...') {
          navBadge.textContent = 'v1.6';
        }
      }
    }

    // Crisp retro pixel-art floorplan fallback renderer synced to live Reviewer & Tender workers
    let PIXEL_TICK = 0;
    function drawFallbackPixelOffice() {
      const canvas = document.getElementById('pixelFallbackCanvas');
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      PIXEL_TICK++;

      const W = canvas.width, H = canvas.height;
      ctx.fillStyle = '#0c1220';
      ctx.fillRect(0, 0, W, H);

      // Floor tiles
      for (let x = 0; x < W; x += 24) {
        for (let y = 52; y < H; y += 24) {
          ctx.fillStyle = ((x / 24 + y / 24) % 2 === 0) ? '#10182a' : '#0d1424';
          ctx.fillRect(x, y, 24, 24);
        }
      }

      // Top office wall & M1 server rack
      ctx.fillStyle = '#182238';
      ctx.fillRect(0, 0, W, 52);
      ctx.fillStyle = '#263554';
      ctx.fillRect(0, 50, W, 3);
      ctx.fillStyle = '#93c5fd';
      ctx.font = 'bold 12px monospace';
      ctx.fillText('M1 INTERNAL CI OFFICE  •  CLAUDE ⇄ AGY CLUSTER', 16, 30);

      const stations = [];
      (STATE?.active_workers || []).forEach(w => {
        stations.push({ label: `CR #${w.pr_number}`, runner: w.lead_runner, active: true, kind: 'reviewer' });
      });
      (STATE?.tender_active_workers || []).forEach(w => {
        stations.push({ label: `CT #${w.pr_number}`, runner: w.lead_runner, active: true, kind: 'tender' });
      });
      while (stations.length < 6) {
        const idx = stations.length + 1;
        stations.push({
          label: idx <= 3 ? `Brain-0${idx}` : `Tender-0${idx - 3}`,
          runner: idx % 2 === 0 ? 'agy' : 'claude',
          active: false,
          kind: idx <= 3 ? 'reviewer' : 'tender'
        });
      }

      stations.slice(0, 6).forEach((st, i) => {
        const col = i % 3, row = Math.floor(i / 3);
        const dx = 65 + col * 235;
        const dy = 96 + row * 112;

        // Desk
        ctx.fillStyle = '#3b2b20';
        ctx.fillRect(dx, dy, 148, 28);
        ctx.fillStyle = '#523c2d';
        ctx.fillRect(dx + 2, dy + 2, 144, 6);

        // Monitor glow
        const screenColor = st.active
          ? (PIXEL_TICK % 6 < 3 ? '#22d3ee' : '#34d399')
          : '#1e293b';
        ctx.fillStyle = '#090d16';
        ctx.fillRect(dx + 52, dy - 22, 44, 24);
        ctx.fillStyle = screenColor;
        ctx.fillRect(dx + 56, dy - 18, 36, 16);

        // Pixel developer sprite
        const bob = st.active ? (PIXEL_TICK % 4 < 2 ? -2 : 0) : 0;
        const shirt = st.runner === 'claude' ? '#f97316' : '#06b6d4';
        ctx.fillStyle = '#fde68a'; // head
        ctx.fillRect(dx + 64, dy + 26 + bob, 20, 16);
        ctx.fillStyle = '#1e293b'; // hair
        ctx.fillRect(dx + 64, dy + 24 + bob, 20, 6);
        ctx.fillStyle = shirt; // torso
        ctx.fillRect(dx + 60, dy + 42 + bob, 28, 18);

        // Label badge
        ctx.fillStyle = st.active ? '#10b981' : '#64748b';
        ctx.font = 'bold 11px monospace';
        ctx.fillText(`${st.label} [${st.runner}]`, dx + 8, dy + 76);
      });
    }

    switchApp(detectInitialApp(), false);
    loadPixelOffice(false);
    refreshAll();
    setInterval(refreshAll, 2500);
    setInterval(drawFallbackPixelOffice, 350);
  </script>
</body>
</html>
"##;
