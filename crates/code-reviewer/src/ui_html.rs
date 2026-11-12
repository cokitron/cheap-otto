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
      background: rgba(9, 11, 16, 0.94);
      backdrop-filter: blur(10px);
      border-bottom: 1px solid var(--border);
      padding: 12px 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 14px;
      flex-wrap: wrap;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
    }
    .brand-logo {
      width: 36px;
      height: 36px;
      border-radius: 9px;
      background: linear-gradient(135deg, #6366f1, #06b6d4);
      display: grid;
      place-items: center;
      font-weight: 800;
      color: #fff;
      font-size: 15px;
      flex-shrink: 0;
    }
    .brand-logo.tender-mode {
      background: linear-gradient(135deg, #10b981, #06b6d4);
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
    }
    .card.highlight-tender {
      border-color: rgba(16, 185, 129, 0.45);
      background: linear-gradient(180deg, rgba(16,185,129,0.07), var(--panel));
    }
    .kpi-label { font-size: 12px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
    .kpi-val { font-size: 26px; font-weight: 800; margin-top: 4px; }
    .kpi-sub { font-size: 12px; color: var(--muted); margin-top: 2px; }
    .grid-2 { display: grid; grid-template-columns: 1.35fr 1fr; gap: 16px; }
    .split-pr { display: grid; grid-template-columns: 360px 1fr; gap: 16px; align-items: start; }
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
    .b-changes, .b-critical, .b-high, .b-failed { background: rgba(239,68,68,0.16); color: #fca5a5; border: 1px solid rgba(239,68,68,0.4); }
    .b-medium, .b-running, .b-staged { background: rgba(245,158,11,0.16); color: #fcd34d; border: 1px solid rgba(245,158,11,0.4); }
    .b-claude { background: rgba(249,115,22,0.16); color: #fdba74; border: 1px solid rgba(249,115,22,0.4); }
    .b-agy { background: rgba(6,182,212,0.16); color: #67e8f9; border: 1px solid rgba(6,182,212,0.4); }
    .b-explained { background: rgba(168,85,247,0.16); color: #d8b4fe; border: 1px solid rgba(168,85,247,0.4); }
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
    .pr-item.selected-tender { border-color: var(--tender); background: #132524; }
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
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="brand-logo" id="brand-logo">CR</div>
      <div>
        <h1 id="brand-title">Internal CI — Multi-Lens Code Reviewer</h1>
        <div class="sub" id="brand-sub">Parallel Async Sessions • Claude ⇄ Antigravity Load Intercalation • Living Docs</div>
      </div>
      <div class="app-switcher" role="navigation" aria-label="Subsystem Switcher">
        <button id="nav-app-reviewer" class="active-reviewer" onclick="switchApp('reviewer')">
          🔍 Multi-Lens Code Reviewer <span class="badge b-muted" id="nav-reviewer-badge">0</span>
        </button>
        <button id="nav-app-tender" onclick="switchApp('tender')">
          🛠️ Code Tender <span class="badge b-muted" id="nav-tender-badge">0</span>
        </button>
      </div>
    </div>
    <div class="header-right">
      <span class="pill" id="pill-poll-mode"><span class="dot ok"></span>Poller: Pure GraphQL (0 AI)</span>
      <span class="pill" id="pill-claude"><span class="dot"></span>claude</span>
      <span class="pill" id="pill-agy"><span class="dot"></span>agy</span>
      <span class="pill" id="pill-intercalate">⇄ Intercalate</span>
      <span class="pill" id="pill-power">M1 Host</span>
      <button id="primary-poll-btn" onclick="triggerPrimaryAction()">⚡ Poll &amp; Dispatch Reviews</button>
      <button class="secondary" onclick="refreshAll()">↻ Refresh</button>
    </div>
  </header>

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
              <thead><tr><th>Repository</th><th>Branch</th><th>Identity Policy</th><th>Review Event</th><th>Reviewer</th><th>Tender</th><th>Actions</th></tr></thead>
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
  </div>

  <!-- ========================================================= -->
  <!-- APP 2: CODE TENDER (AUTONOMOUS REVIEW HANDLER)            -->
  <!-- ========================================================= -->
  <div id="app-tender" class="hidden">
    <nav class="tabs tender-tabs" id="tender-tabs">
      <button class="active" data-tab="tender-live" onclick="switchTenderTab('tender-live')">1. Tender Queue &amp; Live AI Controls</button>
      <button data-tab="tender-prs" onclick="switchTenderTab('tender-prs')">2. Handled PRs, Staged Patches &amp; Replies</button>
      <button data-tab="tender-repos" onclick="switchTenderTab('tender-repos')">3. Tender Repos &amp; Agent Prompts</button>
    </nav>

    <main>
      <!-- TENDER TAB 1: LIVE & EASY AI CONTROLS -->
      <section id="tab-tender-live">
        <div class="kpis" id="tender-kpi-strip"></div>

        <div class="card highlight-tender" style="margin-bottom:16px;">
          <h2>
            <span>Quick Launch — Tend Specific Pull Request</span>
            <span class="sub" style="font-size:12px;color:var(--muted);">Triage review comments ➔ Fix disjoint file packets in parallel ➔ Verify checks ➔ Push or Stage Patch</span>
          </h2>
          <div class="form-row">
            <div>
              <label>Watched Repository</label>
              <select id="qt-repo-select" onchange="syncQuickTendRepoInput()"></select>
            </div>
            <div>
              <label>Or Custom Owner / Repo</label>
              <input id="qt-repo-custom" placeholder="owner/repo" />
            </div>
            <div>
              <label>Pull Request #</label>
              <input type="number" id="qt-pr" min="1" placeholder="42" />
            </div>
            <div>
              <label>Delivery Mode</label>
              <select id="qt-stage-only">
                <option value="default">Follow Default Policy</option>
                <option value="false">Auto-Commit &amp; Push Verified Fix</option>
                <option value="true">Stage Patch Only (Dry-Run / Approve)</option>
              </select>
            </div>
            <div>
              <label>Lead AI Provider</label>
              <select id="qt-lead-runner">
                <option value="auto">Auto (50/50 Claude ⇄ Agy Intercalate)</option>
                <option value="claude">claude</option>
                <option value="agy">agy</option>
              </select>
            </div>
            <div style="display:flex;align-items:flex-end;">
              <button class="tender-btn" style="width:100%;justify-content:center;" onclick="triggerQuickTend()">🛠️ Tend PR Now</button>
            </div>
          </div>
        </div>

        <div class="card" style="margin-bottom:16px;">
          <h2>
            <span>Active Parallel Code Tender Workers</span>
            <span class="sub" id="tender-parallel-label" style="font-size:12px;color:var(--muted);"></span>
          </h2>
          <div id="tender-active-workers-box"></div>
        </div>

        <div class="grid-2">
          <div class="card">
            <h2><span>Live Code Tender Poller &amp; Fix Stream</span><span class="badge b-muted">auto-updating 2.5s</span></h2>
            <pre id="tender-logs-box">Loading tender logs...</pre>

            <h3 style="margin-top:16px;">Recent 2m Tender GraphQL Polls <span class="badge b-pass">0 AI Tokens / Poll</span></h3>
            <div style="max-height:200px;overflow-y:auto;">
              <table>
                <thead><tr><th>Tick</th><th>Engine</th><th>Duration</th><th>Started (UTC)</th></tr></thead>
                <tbody id="tender-history-tbody"></tbody>
              </table>
            </div>
          </div>

          <div class="card">
            <h2><span>Easy-to-Use Code Tender AI Controls</span></h2>
            <div class="form-row">
              <div>
                <label>Autonomous Tender Master Switch</label>
                <select id="tcfg-enabled">
                  <option value="true">Enabled (Auto-handle new review feedback)</option>
                  <option value="false">Paused (Manual triggers only)</option>
                </select>
              </div>
              <div>
                <label>Default Fix Delivery Mode</label>
                <select id="tcfg-delivery">
                  <option value="auto_push">Auto-Commit &amp; Push Verified Fixes</option>
                  <option value="stage_patch_only">Stage Patch Only (Human Approves Push)</option>
                </select>
              </div>
              <div>
                <label>2m Tender Poller Token Guard</label>
                <select id="tcfg-poll-only">
                  <option value="false">Event-Only AI (0 AI on idle polls)</option>
                  <option value="true">Strict Poll-Only (Pause Auto-AI)</option>
                </select>
              </div>
              <div>
                <label>Intercalate Providers (`claude` ⇄ `agy`)</label>
                <select id="tcfg-intercalate">
                  <option value="true">Enabled (50/50 + failover)</option>
                  <option value="false">Fixed Primary / Verifier</option>
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
              <div>
                <label>Auto-Resolve Fixed Review Threads</label>
                <select id="tcfg-resolve">
                  <option value="true">Yes (Resolve GraphQL threads after fix)</option>
                  <option value="false">No (Reply only; leave threads open)</option>
                </select>
              </div>
              <div>
                <label>Ignore Clean PASS / APPROVED Reviews</label>
                <select id="tcfg-ignore-pass">
                  <option value="true">Yes (Prevent unnecessary churn)</option>
                  <option value="false">No (Inspect all review summaries)</option>
                </select>
              </div>
            </div>
            <div style="margin-bottom:12px;">
              <label>Allowed PR Authors (comma-separated — safety boundary; leave default for your own PRs)</label>
              <input id="tcfg-allowed-authors" placeholder="primary_account, secondary_account" />
            </div>
            <button class="tender-btn" onclick="saveTenderSettings()">Save Code Tender AI Settings</button>
          </div>
        </div>
      </section>

      <!-- TENDER TAB 2: HANDLED PRS, STAGED PATCHES & REPLIES -->
      <section id="tab-tender-prs" class="hidden">
        <div class="split-pr">
          <div class="card">
            <h2><span>Tended Pull Requests</span><span id="tender-pr-count-badge" class="badge b-muted">0</span></h2>
            <div id="tender-pr-list-box"></div>
          </div>
          <div class="card" id="tender-pr-detail-box">
            <div style="color:var(--muted);">Select a Pull Request on the left to inspect staged patches, review replies, resolved threads, or trigger an autonomous fix cycle.</div>
          </div>
        </div>
      </section>

      <!-- TENDER TAB 3: REPOS & TENDER AGENT PROMPTS -->
      <section id="tab-tender-repos" class="hidden">
        <div class="card" style="margin-bottom:16px;">
          <h2><span>Repository Auto-Tending Toggles (`config/repos.toml`)</span></h2>
          <table>
            <thead><tr><th>Repository</th><th>Branch</th><th>Verification Check Command</th><th>Reviewer</th><th>Code Tender</th><th>Actions</th></tr></thead>
            <tbody id="tender-repos-tbody"></tbody>
          </table>
        </div>

        <div class="split-pr">
          <div class="card">
            <h2>Code Tender Agents (01..03)</h2>
            <div id="tender-agents-list-box"></div>
          </div>
          <div class="card">
            <h2 id="tender-agent-detail-title">Tender Agent Prompt &amp; Playbook Notes</h2>
            <div style="margin-bottom:12px;">
              <label>System Instructions (`PROMPT.md`)</label>
              <textarea id="ta-prompt" class="code-editor"></textarea>
            </div>
            <div style="margin-bottom:12px;">
              <label>Operator Notes / Guardrails (`notes.md`)</label>
              <textarea id="ta-notes" class="code-editor" style="min-height:130px;"></textarea>
            </div>
            <button class="tender-btn" onclick="saveTenderAgentPrompt()">💾 Save Prompt &amp; Notes</button>
          </div>
        </div>
      </section>
    </main>
  </div>

  <div class="toast" id="toast"></div>

  <script>
    let STATE = null;
    let ACTIVE_APP = 'reviewer';
    let ACTIVE_REVIEWER_TAB = 'live';
    let ACTIVE_TENDER_TAB = 'tender-live';
    let SELECTED_PR_KEY = null;
    let SELECTED_TENDER_PR_KEY = null;
    let OBS_FILTER = 'all';
    let TENDER_ITEM_FILTER = 'all';
    let SELECTED_BRAIN = '01-intent-traceability';
    let SELECTED_TENDER_AGENT = '01-tender-triage';
    let SETTINGS_INITIALIZED = false;
    let TENDER_SETTINGS_INITIALIZED = false;

    function detectInitialApp() {
      const path = (window.location.pathname || '').toLowerCase();
      const params = new URLSearchParams(window.location.search || '');
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

    function switchApp(app, pushUrl = true) {
      ACTIVE_APP = app === 'tender' ? 'tender' : 'reviewer';
      const isTender = ACTIVE_APP === 'tender';

      document.getElementById('app-reviewer').classList.toggle('hidden', isTender);
      document.getElementById('app-tender').classList.toggle('hidden', !isTender);

      document.getElementById('nav-app-reviewer').classList.toggle('active-reviewer', !isTender);
      document.getElementById('nav-app-tender').classList.toggle('active-tender', isTender);

      const logo = document.getElementById('brand-logo');
      logo.textContent = isTender ? 'CT' : 'CR';
      logo.classList.toggle('tender-mode', isTender);

      document.getElementById('brand-title').textContent = isTender
        ? 'Internal CI — Code Tender (Autonomous Review Handler)'
        : 'Internal CI — Multi-Lens Code Reviewer';
      document.getElementById('brand-sub').textContent = isTender
        ? 'Parallel Review Fixing • Claude ⇄ Antigravity Intercalation • Check Verification & Thread Resolution'
        : 'Parallel Async Sessions • Claude ⇄ Antigravity Load Intercalation • Living Docs';

      const primaryBtn = document.getElementById('primary-poll-btn');
      primaryBtn.className = isTender ? 'tender-btn' : '';
      primaryBtn.textContent = isTender ? '🛠️ Poll & Tend Reviews' : '⚡ Poll & Dispatch Reviews';

      if (pushUrl && window.location.protocol.startsWith('http')) {
        const targetPath = isTender ? '/tender' : '/reviewer';
        if (window.location.pathname !== targetPath) {
          window.history.pushState({ app: ACTIVE_APP }, '', targetPath);
        }
      }

      renderTopStatusBar();
      if (isTender && ACTIVE_TENDER_TAB === 'tender-repos') {
        loadTenderAgentDetail(SELECTED_TENDER_AGENT);
      }
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
      ACTIVE_TENDER_TAB = tab;
      document.querySelectorAll('#tender-tabs button').forEach(b => {
        b.classList.toggle('active', b.dataset.tab === tab);
      });
      ['tender-live', 'tender-prs', 'tender-repos'].forEach(t => {
        document.getElementById('tab-' + t).classList.toggle('hidden', t !== tab);
      });
      if (tab === 'tender-repos') loadTenderAgentDetail(SELECTED_TENDER_AGENT);
    }

    function openPrInTender(repo, prNumber) {
      switchApp('tender');
      const key = `${repo}#${prNumber}`;
      const exists = (STATE?.tender_pull_requests || []).some(p => p.key === key);
      const repoSel = document.getElementById('qt-repo-select');
      if (repoSel && Array.from(repoSel.options).some(o => o.value === repo)) {
        repoSel.value = repo;
      }
      document.getElementById('qt-repo-custom').value = repo;
      document.getElementById('qt-pr').value = prNumber;
      if (exists) {
        SELECTED_TENDER_PR_KEY = key;
        renderTenderPrList();
        switchTenderTab('tender-prs');
      } else {
        switchTenderTab('tender-live');
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
      const revBadge = document.getElementById('nav-reviewer-badge');
      const tendBadge = document.getElementById('nav-tender-badge');
      revBadge.textContent = revActive ? `${revActive} live` : String((STATE.pull_requests || []).length);
      revBadge.className = `badge ${revActive ? 'b-running' : 'b-muted'}`;
      tendBadge.textContent = tendActive ? `${tendActive} live` : String((STATE.tender_pull_requests || []).length);
      tendBadge.className = `badge ${tendActive ? 'b-running' : 'b-muted'}`;

      const pmPill = document.getElementById('pill-poll-mode');
      const isTender = ACTIVE_APP === 'tender';
      const pollPaused = isTender ? Boolean(STATE.tender?.poll_only_mode) : Boolean(STATE.agents?.poll_only_mode);
      if (pollPaused) {
        pmPill.innerHTML = `<span class="dot warn"></span>Poll-Only (Auto-AI Paused • 0 Tokens)`;
      } else if (isTender && STATE.tender && !STATE.tender.enabled) {
        pmPill.innerHTML = `<span class="dot warn"></span>Code Tender Paused`;
      } else {
        pmPill.innerHTML = `<span class="dot ok"></span>2m Poll: Pure GraphQL (0 AI Tokens)`;
      }

      const cPill = document.getElementById('pill-claude');
      cPill.innerHTML = `<span class="dot ${STATE.runners.claude_ready ? 'ok' : 'warn'}"></span>claude (${esc(STATE.agents.claude_model || 'sonnet')}): ${STATE.runners.claude_ready ? 'ready' : 'sign-in'}`;
      const aPill = document.getElementById('pill-agy');
      aPill.innerHTML = `<span class="dot ${STATE.runners.agy_ready ? 'ok' : 'warn'}"></span>agy (${esc(STATE.agents.reasoning_effort || 'medium')}): ${STATE.runners.agy_ready ? 'ready' : 'sign-in'}`;

      const iPill = document.getElementById('pill-intercalate');
      if (isTender && STATE.tender) {
        iPill.textContent = STATE.tender.intercalate_providers
          ? `⇄ Fix Batch (${STATE.tender.max_parallel_prs} PRs × ${STATE.tender.max_parallel_fixers_per_pr} Fixers)`
          : `Fixed Primary (${STATE.agents.primary_runner})`;
      } else {
        iPill.textContent = STATE.agents.intercalate_providers
          ? `⇄ All-Parallel (${STATE.agents.max_parallel_prs} PRs × ${STATE.agents.max_parallel_brains_per_pr} Brains)`
          : `Single Primary (${STATE.agents.primary_runner})`;
      }

      const pPill = document.getElementById('pill-power');
      pPill.innerHTML = `<span class="dot ${STATE.host.ac_power ? 'ok' : 'warn'}"></span>${esc(STATE.host.summary)}`;
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
          <td style="display:flex;gap:6px;flex-wrap:wrap;">
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
      if (ACTIVE_APP === 'tender') {
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
    // CODE TENDER RENDERERS & ACTIONS
    // =========================================================

    function syncQuickTendRepoInput() {
      const sel = document.getElementById('qt-repo-select');
      const custom = document.getElementById('qt-repo-custom');
      if (sel && sel.value) custom.value = sel.value;
    }

    function renderTenderLive() {
      if (!STATE || !STATE.tender) return;
      const tc = STATE.tender;
      const tprs = STATE.tender_pull_requests || [];
      const active = STATE.tender_active_workers || [];
      const stagedCount = tprs.filter(p => Boolean(p.staged_patch)).length;
      const tenderEnabledRepos = (STATE.repos || []).filter(r => r.tender_enabled).length;

      // Keep quick-tend repo selector populated
      const qSel = document.getElementById('qt-repo-select');
      const prevSel = qSel.value;
      qSel.innerHTML = (STATE.repos || []).map(r => `<option value="${esc(r.full_name)}">${esc(r.full_name)}${r.tender_enabled ? '' : ' (paused)'}</option>`).join('');
      if (prevSel && Array.from(qSel.options).some(o => o.value === prevSel)) {
        qSel.value = prevSel;
      } else if (qSel.options.length && !document.getElementById('qt-repo-custom').value) {
        document.getElementById('qt-repo-custom').value = qSel.value;
      }

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

      document.getElementById('tender-kpi-strip').innerHTML = `
        <div class="card"><div class="kpi-label">Active Tender Sessions</div><div class="kpi-val" style="color:var(--tender);">${active.length} / ${tc.max_parallel_prs}</div><div class="kpi-sub">Next lead AI: <b>${esc(STATE.tender_next_lead_runner || 'claude')}</b></div></div>
        <div class="card"><div class="kpi-label">Tracked Tender PRs</div><div class="kpi-val">${tprs.length}</div><div class="kpi-sub">Mode: <code>${esc(tc.delivery_mode)}</code></div></div>
        <div class="card"><div class="kpi-label">Staged Patches Waiting</div><div class="kpi-val" style="color:${stagedCount > 0 ? 'var(--amber)' : 'var(--green)'};">${stagedCount}</div><div class="kpi-sub">Ready for 1-click approve &amp; push</div></div>
        <div class="card"><div class="kpi-label">Tender-Enabled Repos</div><div class="kpi-val">${tenderEnabledRepos}</div><div class="kpi-sub">Authors: <code>${esc(((tc.allowed_authors && tc.allowed_authors.length) ? tc.allowed_authors : [STATE.accounts?.primary, STATE.accounts?.secondary].filter(Boolean)).join(', '))}</code></div></div>
      `;

      document.getElementById('tender-parallel-label').textContent =
        `Up to ${tc.max_parallel_prs} PRs tended concurrently (${tc.max_parallel_fixers_per_pr} parallel disjoint fixers/PR, cap ${tc.max_autonomous_cycles_per_pr} cycles/PR)`;

      const awb = document.getElementById('tender-active-workers-box');
      if (!active.length) {
        awb.innerHTML = `<div style="color:var(--muted);padding:8px 0;">Idle — zero in-flight Code Tender sessions right now. New PR comments/reviews will automatically wake fix agents on the next poll tick (or trigger one above).</div>`;
      } else {
        awb.innerHTML = `<div class="worker-grid">` + active.map(w => {
          const chips = Object.entries(w.workers || {}).map(([name, wState]) => {
            const rBadge = wState.runner === 'claude' ? 'b-claude' : (wState.runner === 'agy' ? 'b-agy' : 'b-muted');
            return `<div class="brain-chip ${esc(wState.status)}">
              <span>${esc(name)}</span>
              <span><span class="badge ${rBadge}">${esc(wState.runner)}</span> <span class="badge ${wState.status === 'running' ? 'b-running' : (wState.status === 'done' ? 'b-done' : 'b-muted')}">${esc(wState.status)}</span></span>
            </div>`;
          }).join('');
          return `<div class="worker-card">
            <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
              <strong><span class="dot run"></span> ${esc(w.repo)} #${w.pr_number}</strong>
              <span class="badge b-running">${esc(w.phase)} (${w.packets_done}/${w.packets_total})</span>
            </div>
            <div style="font-size:12.5px;margin-top:4px;">${esc(w.title)}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">
              SHA: <code>${esc((w.head_sha || '').slice(0, 7))}</code> • Author Identity: <code>${esc(w.tender_account)}</code> • PID: <code>${w.pid}</code><br/>
              Fixer Providers: <span class="badge b-claude">lead: ${esc(w.lead_runner)}</span> ⇄ <span class="badge b-agy">alt: ${esc(w.alt_runner)}</span>
            </div>
            <div class="brain-chips">${chips}</div>
          </div>`;
        }).join('') + `</div>`;
      }

      document.getElementById('tender-logs-box').textContent = STATE.tender_logs || '(no Code Tender logs yet)';
      document.getElementById('tender-history-tbody').innerHTML = (STATE.tender_history || []).map(h => `
        <tr>
          <td><span class="badge ${h.status === 'pass' ? 'b-pass' : 'b-changes'}">${esc(h.status)}</span></td>
          <td><span class="badge b-muted">GraphQL (0 AI)</span></td>
          <td>${h.duration_secs}s</td>
          <td style="color:var(--muted);">${new Date(h.started_epoch_secs * 1000).toISOString().replace('T', ' ').slice(0, 19)}</td>
        </tr>
      `).join('');
    }

    function tenderStatusClass(badge) {
      if (badge === 'FIXED_PUSHED') return 'b-pass';
      if (badge === 'STAGED_PATCH') return 'b-staged';
      if (badge === 'REPLIED_ONLY') return 'b-explained';
      if (badge === 'CHECK_FAILED' || badge === 'NEEDS_HUMAN') return 'b-changes';
      return 'b-muted';
    }

    function renderTenderPrList() {
      const prs = STATE?.tender_pull_requests || [];
      document.getElementById('tender-pr-count-badge').textContent = prs.length;
      if (!SELECTED_TENDER_PR_KEY && prs.length) SELECTED_TENDER_PR_KEY = prs[0].key;

      document.getElementById('tender-pr-list-box').innerHTML = prs.map(p => {
        const badgeCls = tenderStatusClass(p.status_badge);
        const pendingCnt = (p.pending_items || []).length;
        const handledCnt = (p.handled_items || []).length;
        return `<div class="pr-item ${p.key === SELECTED_TENDER_PR_KEY ? 'selected-tender' : ''}" onclick="selectTenderPr('${esc(p.key)}')">
          <div style="display:flex;justify-content:space-between;gap:8px;align-items:center;">
            <strong>${esc(p.repo)} #${p.pr_number}</strong>
            <span class="badge ${badgeCls}">${esc(p.status_badge)}</span>
          </div>
          <div style="font-size:12.5px;margin-top:4px;">${esc(p.title)}</div>
          <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">
            Cycles: ${p.runs_count} • Pending: ${pendingCnt} • Handled: ${handledCnt}${p.staged_patch ? ' • <strong style="color:#fcd34d;">Staged Patch</strong>' : ''}
          </div>
        </div>`;
      }).join('') || '<div style="color:var(--muted);">No tended PRs yet. Use Quick Launch or wait for review feedback.</div>';

      const selected = prs.find(p => p.key === SELECTED_TENDER_PR_KEY);
      const box = document.getElementById('tender-pr-detail-box');
      if (!selected) return;

      const p = selected;
      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const pending = p.pending_items || [];
      const handledAll = p.handled_items || [];
      const handledFiltered = handledAll.filter(h => {
        if (TENDER_ITEM_FILTER === 'fixed') return h.action === 'fixed' || h.action === 'already_addressed';
        if (TENDER_ITEM_FILTER === 'explained') return h.action === 'explained_rationale';
        if (TENDER_ITEM_FILTER === 'human') return h.action === 'needs_human';
        return true;
      });

      const stagedHtml = p.staged_patch ? `
        <div class="card highlight-tender" style="margin-top:14px;">
          <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap;">
            <div>
              <h3 style="margin-bottom:2px;">📦 Staged Patch Ready for Approval</h3>
              <div style="font-size:12px;color:var(--muted);">Commit message: <code>${esc(p.staged_commit_message || 'fix: address code review feedback')}</code></div>
            </div>
            <div style="display:flex;gap:8px;flex-wrap:wrap;">
              <button class="tender-btn" onclick="approveAndPushPatch('${esc(p.repo)}', ${p.pr_number})">✅ Approve &amp; Push Staged Fix</button>
              <button class="danger" onclick="discardStagedPatch('${esc(p.repo)}', ${p.pr_number})">🗑 Discard Patch</button>
            </div>
          </div>
          <pre>${esc(p.staged_patch)}</pre>
        </div>` : '';

      const pendingHtml = pending.length ? `
        <h3 style="margin-top:18px;">Queued / Pending Review Feedback (${pending.length})</h3>
        ${pending.map(item => {
          const loc = item.path ? (item.line ? `${item.path}:${item.line}` : item.path) : item.kind;
          return `<div class="obs-card">
            <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;">
              <div><span class="badge b-running">${esc(item.kind)}</span> <strong>@${esc(item.author)}</strong></div>
              <span style="font-size:11.5px;color:var(--muted);">${esc(item.created_at)}</span>
            </div>
            <div style="margin-top:4px;font-size:12px;color:#93c5fd;"><code>${esc(loc)}</code></div>
            <div style="margin-top:6px;white-space:pre-wrap;">${esc(item.body)}</div>
          </div>`;
        }).join('')}` : '';

      const handledHtml = handledFiltered.map(h => {
        const aClass = (h.action === 'fixed' || h.action === 'already_addressed') ? 'b-fixed' : (h.action === 'explained_rationale' ? 'b-explained' : 'b-changes');
        const loc = h.path ? (h.line ? `${h.path}:${h.line}` : h.path) : 'PR discussion';
        return `<div class="obs-card">
          <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
            <div>
              <span class="badge ${aClass}">${esc(h.action)}</span>
              <strong>@${esc(h.source_author)}</strong>
              ${h.resolved_thread ? '<span class="badge b-pass">✓ Thread Resolved</span>' : ''}
              ${h.reply_posted ? '<span class="badge b-muted">✓ Reply Posted</span>' : ''}
            </div>
            ${h.commit_sha ? `<span style="font-size:11.5px;color:var(--muted);">Commit: <code>${esc(h.commit_sha.slice(0, 7))}</code></span>` : ''}
          </div>
          <div style="margin-top:6px;font-size:12px;color:#93c5fd;"><code>${esc(loc)}</code></div>
          <div style="margin-top:6px;"><strong>Summary:</strong> ${esc(h.summary)}</div>
          <div style="margin-top:6px;color:#d1fae5;"><strong>Agent Reply:</strong> ${esc(h.reply_text)}</div>
        </div>`;
      }).join('') || '<div style="color:var(--muted);padding:10px 0;">No handled items match this filter yet.</div>';

      box.innerHTML = `
        <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap;">
          <div>
            <h2 style="margin-bottom:4px;">${esc(p.repo)} #${p.pr_number} — ${esc(p.title)}</h2>
            <div style="font-size:12px;color:var(--muted);">
              Branch: <code>${esc(p.head_ref)}</code> → <code>${esc(p.base_ref)}</code> • Author: <code>${esc(p.author)}</code> • Tended SHA: <code>${esc((p.last_tended_sha || '-').slice(0, 7))}</code>${p.last_commit_pushed ? ` • Pushed: <code>${esc(p.last_commit_pushed.slice(0, 7))}</code>` : ''}
            </div>
          </div>
          <div style="display:flex;gap:8px;flex-wrap:wrap;">
            <a class="btn secondary" href="${esc(ghUrl)}" target="_blank" rel="noopener">↗ GitHub PR</a>
            <button class="secondary" onclick="openPrInReviewer('${esc(p.repo)}', ${p.pr_number})">🔍 View in Multi-Lens Reviewer</button>
            <button class="secondary" onclick="triggerPrTend('${esc(p.repo)}', ${p.pr_number}, true)">🧪 Stage Patch Only</button>
            <button class="tender-btn" onclick="triggerPrTend('${esc(p.repo)}', ${p.pr_number}, false)">🛠️ Auto-Tend Now</button>
          </div>
        </div>
        ${p.last_summary ? `<div class="card" style="background:var(--panel-2);margin-top:12px;"><strong>Latest Tender Summary:</strong> ${esc(p.last_summary)}</div>` : ''}
        ${stagedHtml}
        ${pendingHtml}
        <h3 style="margin-top:18px;">Handled Review Items &amp; Replies (${handledAll.length})</h3>
        <div class="filter-bar">
          <button class="${TENDER_ITEM_FILTER === 'all' ? 'tender-btn' : 'secondary'}" onclick="setTenderItemFilter('all')">All (${handledAll.length})</button>
          <button class="${TENDER_ITEM_FILTER === 'fixed' ? 'tender-btn' : 'secondary'}" onclick="setTenderItemFilter('fixed')">Fixed (${handledAll.filter(h => h.action === 'fixed' || h.action === 'already_addressed').length})</button>
          <button class="${TENDER_ITEM_FILTER === 'explained' ? 'tender-btn' : 'secondary'}" onclick="setTenderItemFilter('explained')">Explained (${handledAll.filter(h => h.action === 'explained_rationale').length})</button>
          <button class="${TENDER_ITEM_FILTER === 'human' ? 'tender-btn' : 'secondary'}" onclick="setTenderItemFilter('human')">Needs Human (${handledAll.filter(h => h.action === 'needs_human').length})</button>
        </div>
        ${handledHtml}
      `;
    }

    function renderTenderReposAndAgents() {
      if (!STATE) return;
      document.getElementById('tender-repos-tbody').innerHTML = (STATE.repos || []).map(r => `
        <tr>
          <td><strong>${esc(r.full_name)}</strong></td>
          <td><code>${esc(r.default_branch)}</code></td>
          <td><code>${esc(r.local_check_cmd || '(auto-detect)')}</code></td>
          <td><span class="badge ${r.enabled ? 'b-pass' : 'b-muted'}">${r.enabled ? 'REVIEWER ON' : 'PAUSED'}</span></td>
          <td><span class="badge ${r.tender_enabled ? 'b-pass' : 'b-muted'}">${r.tender_enabled ? 'TENDER ENABLED' : 'TENDER PAUSED'}</span></td>
          <td>
            <button class="${r.tender_enabled ? 'secondary' : 'tender-btn'}" style="padding:4px 10px;font-size:11.5px;" onclick="toggleRepoTender('${esc(r.full_name)}')">
              ${r.tender_enabled ? 'Pause Tender' : 'Enable Tender'}
            </button>
          </td>
        </tr>
      `).join('');

      const agents = STATE.tender_agents || ['01-tender-triage', '02-tender-fixer', '03-tender-verifier'];
      document.getElementById('tender-agents-list-box').innerHTML = agents.map(a => `
        <div class="pr-item ${a === SELECTED_TENDER_AGENT ? 'selected-tender' : ''}" onclick="loadTenderAgentDetail('${esc(a)}')">
          <strong>${esc(a)}</strong>
        </div>
      `).join('');
    }

    async function loadTenderAgentDetail(agentId) {
      SELECTED_TENDER_AGENT = agentId;
      renderTenderReposAndAgents();
      document.getElementById('tender-agent-detail-title').textContent = `Tender Agent: ${agentId}`;
      const d = await api('/api/tender-agent?id=' + encodeURIComponent(agentId));
      document.getElementById('ta-prompt').value = d.prompt || '';
      document.getElementById('ta-notes').value = d.notes || '';
    }

    async function saveTenderAgentPrompt() {
      const prompt = document.getElementById('ta-prompt').value;
      const notes = document.getElementById('ta-notes').value;
      await api('/api/tender-agent/save', {
        method: 'POST',
        body: JSON.stringify({
          agent: SELECTED_TENDER_AGENT,
          prompt,
          notes
        })
      });
      showToast(`Saved prompt & notes for ${SELECTED_TENDER_AGENT}`);
    }

    function selectTenderPr(key) { SELECTED_TENDER_PR_KEY = key; renderTenderPrList(); }
    function setTenderItemFilter(f) { TENDER_ITEM_FILTER = f; renderTenderPrList(); }

    async function triggerTenderPollNow() {
      const res = await api('/api/actions/tend-poll', { method: 'POST', body: '{}' });
      showToast(res.message || 'Code Tender parallel poll dispatched!');
      setTimeout(refreshAll, 900);
    }

    async function triggerQuickTend() {
      const repo = document.getElementById('qt-repo-custom').value.trim() || document.getElementById('qt-repo-select').value;
      const pr = parseInt(document.getElementById('qt-pr').value || '0', 10);
      const stageRaw = document.getElementById('qt-stage-only').value;
      const leadRaw = document.getElementById('qt-lead-runner').value;
      if (!repo || !pr) {
        showToast('Please choose a repository and valid PR number.');
        return;
      }
      const payload = { repo, pr };
      if (stageRaw === 'true') payload.stage_only = true;
      if (stageRaw === 'false') payload.stage_only = false;
      if (leadRaw !== 'auto') payload.lead_runner = leadRaw;

      const res = await api('/api/actions/tend', {
        method: 'POST',
        body: JSON.stringify(payload)
      });
      showToast(res.message || `Launched Code Tender worker for ${repo}#${pr}`);
      setTimeout(refreshAll, 900);
    }

    async function triggerPrTend(repo, pr, stageOnly) {
      const res = await api('/api/actions/tend', {
        method: 'POST',
        body: JSON.stringify({ repo, pr, stage_only: Boolean(stageOnly) })
      });
      showToast(res.message || `Launched Code Tender for ${repo}#${pr}`);
      switchTenderTab('tender-live');
      setTimeout(refreshAll, 900);
    }

    async function approveAndPushPatch(repo, pr) {
      const res = await api('/api/actions/tend/approve-push', {
        method: 'POST',
        body: JSON.stringify({ repo, pr })
      });
      showToast(res.message || `Approved and pushed fix for ${repo}#${pr}!`);
      refreshAll();
    }

    async function discardStagedPatch(repo, pr) {
      const res = await api('/api/actions/tend/discard-patch', {
        method: 'POST',
        body: JSON.stringify({ repo, pr })
      });
      showToast(res.message || `Discarded staged patch for ${repo}#${pr}`);
      refreshAll();
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
      showToast('Saved Code Tender AI settings!');
      refreshAll();
    }

    switchApp(detectInitialApp(), false);
    refreshAll();
    setInterval(refreshAll, 2500);
  </script>
</body>
</html>
"##;
