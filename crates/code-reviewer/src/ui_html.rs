//! Embedded single-binary Neo-Brutalist Web Dashboard HTML/CSS/JS for `code-reviewer` + `Code Tender`.

pub const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>CODE TENDER & MULTI-LENS REVIEWER — M1 INTERNAL CI</title>
  <style>
    :root {
      --bg: #080808;
      --surface: #121212;
      --card: #161616;
      --card-hover: #1e1e1e;
      --ink: #ffffff;
      --ink-dark: #0a0a0a;
      --muted: #8e8e8e;
      --rule: #2c2c2c;
      --yellow: #F7D842;
      --amber: #DE9E1E;
      --coral: #F07A74;
      --mint: #4ADE80;
      --cyan: #38BDF8;
      --shadow-sm: 2px 2px 0 #ffffff;
      --shadow: 4px 4px 0 #ffffff;
      --shadow-lg: 6px 6px 0 #ffffff;
      --font-display: "Arial Black", "Inter", "Helvetica Neue", Impact, system-ui, -apple-system, sans-serif;
      --font-mono: "JetBrains Mono", "IBM Plex Mono", "SFMono-Regular", "Cascadia Mono", Menlo, Consolas, ui-monospace, monospace;
    }
    * {
      box-sizing: border-box;
      border-radius: 0 !important;
    }
    html, body {
      margin: 0;
      padding: 0;
      background: var(--bg);
      color: var(--ink);
      font-family: var(--font-mono);
      font-size: 13px;
      line-height: 1.5;
      -webkit-font-smoothing: antialiased;
    }
    a {
      color: var(--yellow);
      text-decoration: underline;
      text-underline-offset: 3px;
    }
    a:hover {
      background: var(--yellow);
      color: var(--ink-dark);
      text-decoration: none;
    }
    code {
      font-family: var(--font-mono);
      background: #222222;
      padding: 1px 5px;
      border: 1px solid #3a3a3a;
      font-size: 12px;
    }

    /* TOP COMMAND BAR */
    header.topbar {
      position: sticky;
      top: 0;
      z-index: 40;
      background: var(--bg);
      border-bottom: 2px solid var(--ink);
      padding: 10px 22px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
    }
    .topbar-left, .topbar-right {
      display: flex;
      align-items: center;
      gap: 10px;
      flex-wrap: wrap;
    }
    .brand-mark {
      background: var(--ink);
      color: var(--ink-dark);
      font-family: var(--font-display);
      font-weight: 900;
      font-size: 13px;
      letter-spacing: 0.04em;
      padding: 6px 10px;
      text-transform: uppercase;
      border: 2px solid var(--ink);
    }
    .mode-nav {
      display: inline-flex;
      align-items: stretch;
      border: 2px solid var(--ink);
      background: var(--surface);
      box-shadow: var(--shadow-sm);
    }
    .mode-nav button {
      border: none;
      border-right: 2px solid var(--ink);
      background: transparent;
      color: var(--muted);
      padding: 7px 14px;
      font-family: var(--font-mono);
      font-size: 12px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.04em;
      box-shadow: none;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 8px;
    }
    .mode-nav button:last-child {
      border-right: none;
    }
    .mode-nav button:hover {
      background: #202020;
      color: var(--ink);
      transform: none;
      box-shadow: none;
    }
    .mode-nav button.active-mode {
      background: var(--yellow);
      color: var(--ink-dark);
    }
    .mode-count {
      border: 1.5px solid currentColor;
      padding: 0 5px;
      font-size: 11px;
      font-weight: 800;
      line-height: 1.35;
    }

    .status-chip {
      display: inline-flex;
      align-items: center;
      gap: 7px;
      padding: 6px 10px;
      border: 1.5px solid #444444;
      background: var(--surface);
      color: var(--ink);
      font-size: 11.5px;
      text-transform: uppercase;
      letter-spacing: 0.03em;
    }
    .sq-dot {
      width: 8px;
      height: 8px;
      display: inline-block;
      background: var(--mint);
    }
    .sq-dot.warn { background: var(--amber); }
    .sq-dot.err { background: var(--coral); }
    .sq-dot.run {
      background: var(--yellow);
      animation: blink 0.9s steps(2, start) infinite;
    }
    @keyframes blink {
      to { opacity: 0.2; }
    }

    /* BUTTONS */
    button, .btn {
      font-family: var(--font-mono);
      font-size: 12px;
      font-weight: 700;
      text-transform: uppercase;
      letter-spacing: 0.04em;
      padding: 7px 13px;
      border: 2px solid var(--ink);
      background: var(--yellow);
      color: var(--ink-dark);
      box-shadow: var(--shadow-sm);
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      text-decoration: none;
      transition: transform 0.06s ease, box-shadow 0.06s ease;
    }
    button:hover, .btn:hover {
      background: #ffe159;
      color: var(--ink-dark);
      transform: translate(-1px, -1px);
      box-shadow: 3px 3px 0 #ffffff;
      text-decoration: none;
    }
    button:active, .btn:active {
      transform: translate(2px, 2px);
      box-shadow: 0 0 0 #ffffff;
    }
    button:disabled {
      opacity: 0.45;
      cursor: not-allowed;
      transform: none;
    }
    button.secondary, .btn.secondary {
      background: var(--card);
      color: var(--ink);
    }
    button.secondary:hover, .btn.secondary:hover {
      background: #252525;
      color: var(--ink);
    }
    button.tender-btn, .btn.tender-btn {
      background: var(--yellow);
      color: var(--ink-dark);
    }
    button.mint-btn, .btn.mint-btn {
      background: var(--mint);
      color: var(--ink-dark);
    }
    button.danger, .btn.danger {
      background: var(--coral);
      color: var(--ink-dark);
    }
    button.pixel-active {
      background: var(--ink);
      color: var(--ink-dark);
    }

    /* SUB-VIEW BAR */
    .subnav {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 12px 24px 0;
      max-width: 1460px;
      margin: 0 auto;
      flex-wrap: wrap;
    }
    .subnav-tabs {
      display: inline-flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .subnav-tabs button {
      background: var(--bg);
      color: var(--muted);
      border: 2px solid #3a3a3a;
      box-shadow: none;
      padding: 6px 13px;
    }
    .subnav-tabs button:hover {
      border-color: var(--ink);
      color: var(--ink);
      transform: none;
      box-shadow: none;
    }
    .subnav-tabs button.active {
      background: var(--ink);
      color: var(--ink-dark);
      border-color: var(--ink);
      box-shadow: 3px 3px 0 var(--yellow);
    }

    /* WORKSPACE & SPLIT SHELL */
    #workspace-shell {
      display: block;
      min-height: calc(100vh - 58px);
    }
    #workspace-shell.mode-split {
      display: grid;
      grid-template-columns: minmax(0, 1fr) minmax(400px, 43%);
      align-items: start;
    }
    .workspace-left { min-width: 0; }
    main.page {
      max-width: 1460px;
      margin: 0 auto;
      padding: 18px 24px 64px;
    }

    /* SIGNAL YELLOW MASTHEAD */
    .masthead {
      background: var(--yellow);
      color: var(--ink-dark);
      border: 3px solid var(--ink);
      box-shadow: var(--shadow-lg);
      padding: 22px 26px 14px;
      margin-bottom: 22px;
    }
    .masthead-top {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: 20px;
      flex-wrap: wrap;
    }
    .masthead-title {
      margin: 0;
      font-family: var(--font-display);
      font-weight: 900;
      font-size: clamp(32px, 4.4vw, 54px);
      line-height: 0.94;
      letter-spacing: -0.025em;
      text-transform: uppercase;
      color: var(--ink-dark);
    }
    .masthead-stat {
      text-align: right;
      margin-left: auto;
    }
    .masthead-num {
      font-family: var(--font-display);
      font-weight: 900;
      font-size: clamp(40px, 4.8vw, 60px);
      line-height: 0.9;
      letter-spacing: -0.03em;
      color: var(--ink-dark);
    }
    .masthead-caption {
      font-family: var(--font-mono);
      font-size: 11.5px;
      font-weight: 800;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      margin-top: 6px;
      color: var(--ink-dark);
    }
    .masthead-ticker {
      border-top: 1.5px solid var(--ink-dark);
      margin-top: 16px;
      padding-top: 10px;
      display: flex;
      flex-wrap: wrap;
      gap: 8px 22px;
      font-family: var(--font-mono);
      font-size: 11.5px;
      font-weight: 700;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: var(--ink-dark);
    }

    /* NARRATIVE CALLOUT STRIP */
    .narrative-strip {
      border-left: 4px solid var(--ink);
      padding: 4px 0 4px 16px;
      margin: 22px 0 24px;
      max-width: 980px;
      font-size: 13.5px;
      line-height: 1.85;
      color: #efefef;
    }
    .inv-chip {
      display: inline-block;
      background: var(--ink);
      color: var(--ink-dark);
      font-weight: 800;
      padding: 0 6px;
      line-height: 1.45;
      margin: 0 2px;
    }
    .inv-chip.yellow { background: var(--yellow); color: var(--ink-dark); }
    .inv-chip.coral { background: var(--coral); color: var(--ink-dark); }
    .inv-chip.mint { background: var(--mint); color: var(--ink-dark); }

    /* SECTION HEADERS (INVERTED BLOCK + SUBTITLE) */
    .sec-head {
      display: flex;
      align-items: baseline;
      gap: 12px;
      flex-wrap: wrap;
      margin: 28px 0 14px;
    }
    .sec-tag {
      background: var(--ink);
      color: var(--ink-dark);
      font-family: var(--font-display);
      font-weight: 900;
      font-size: 14px;
      letter-spacing: 0.04em;
      padding: 3px 9px;
      text-transform: uppercase;
    }
    .sec-sub {
      color: var(--muted);
      font-size: 12.5px;
    }

    /* VITALS 5-COLUMN GRID */
    .vitals-grid {
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 16px;
      margin-bottom: 28px;
    }
    @media (max-width: 1180px) {
      .vitals-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
    }
    @media (max-width: 700px) {
      .vitals-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    .vital-tile {
      border: 2px solid var(--ink);
      box-shadow: var(--shadow);
      padding: 15px 16px;
      min-height: 148px;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      background: var(--card);
      color: var(--ink);
    }
    .vital-tile.amber {
      background: var(--amber);
      color: var(--ink-dark);
    }
    .vital-tile.coral {
      background: var(--coral);
      color: var(--ink-dark);
    }
    .vital-tile.dark {
      background: #171717;
      color: var(--ink);
    }
    .vital-label {
      font-size: 11.5px;
      font-weight: 800;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      line-height: 1.35;
    }
    .vital-val {
      font-family: var(--font-display);
      font-weight: 900;
      font-size: 42px;
      line-height: 1;
      margin: 10px 0 8px;
      letter-spacing: -0.03em;
    }
    .vital-dash {
      width: 28px;
      height: 4px;
      background: currentColor;
      margin: 18px 0 14px;
    }
    .vital-foot {
      font-size: 11.5px;
      line-height: 1.4;
      opacity: 0.88;
    }
    .vital-tile.dark .vital-foot {
      color: var(--muted);
      opacity: 1;
    }

    /* WATCH COMMAND BAR */
    .watch-strip {
      display: flex;
      align-items: stretch;
      gap: 10px;
      flex-wrap: wrap;
      border: 2px solid var(--ink);
      background: var(--card);
      box-shadow: var(--shadow);
      padding: 12px 14px;
      margin-bottom: 24px;
    }
    .watch-strip-label {
      display: inline-flex;
      align-items: center;
      font-weight: 800;
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: var(--yellow);
      white-space: nowrap;
      padding-right: 4px;
    }
    .watch-strip input {
      flex: 1;
      min-width: 240px;
    }

    /* 3-COLUMN BRUTALIST KANBAN BOARD */
    .board-grid {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 20px;
      align-items: start;
    }
    @media (max-width: 1024px) {
      .board-grid { grid-template-columns: 1fr; }
    }
    .board-col-head {
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      gap: 10px;
      border-bottom: 2px solid var(--ink);
      padding-bottom: 7px;
      margin-bottom: 14px;
      font-size: 12px;
      font-weight: 800;
      letter-spacing: 0.07em;
      text-transform: uppercase;
    }
    .board-col-stack {
      display: flex;
      flex-direction: column;
      gap: 14px;
    }
    .ticket-card {
      background: var(--card);
      border: 2px solid var(--ink);
      box-shadow: var(--shadow);
      padding: 15px 16px;
      cursor: pointer;
      transition: transform 0.07s ease, box-shadow 0.07s ease, background 0.07s ease;
    }
    .ticket-card:hover {
      background: var(--card-hover);
      transform: translate(-2px, -2px);
      box-shadow: 6px 6px 0 #ffffff;
    }
    .ticket-card.selected-card {
      border-color: var(--yellow);
      box-shadow: 4px 4px 0 var(--yellow);
    }
    .ticket-card.standby-card {
      cursor: default;
      border-style: dashed;
      box-shadow: none;
      background: #101010;
    }
    .ticket-card.standby-card:hover {
      transform: none;
      box-shadow: none;
      background: #101010;
    }
    .ticket-top {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 10px;
      margin-bottom: 9px;
    }
    .ticket-id {
      font-family: var(--font-mono);
      font-size: 18px;
      font-weight: 800;
      letter-spacing: -0.02em;
      color: var(--ink);
    }
    .ticket-repo {
      font-size: 11px;
      color: var(--muted);
      display: block;
      margin-top: 1px;
    }
    .rect-tag {
      display: inline-block;
      padding: 2px 7px;
      font-family: var(--font-mono);
      font-size: 11px;
      font-weight: 800;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      border: 1.5px solid var(--ink);
      background: transparent;
      color: var(--ink);
      white-space: nowrap;
    }
    .rect-tag.yellow { background: var(--yellow); color: var(--ink-dark); border-color: var(--ink); }
    .rect-tag.amber { background: var(--amber); color: var(--ink-dark); border-color: var(--ink); }
    .rect-tag.coral { background: var(--coral); color: var(--ink-dark); border-color: var(--ink); }
    .rect-tag.mint { background: var(--mint); color: var(--ink-dark); border-color: var(--ink); }
    .rect-tag.muted { border-color: #555555; color: #aaaaaa; }

    .ticket-title {
      font-size: 13.5px;
      font-weight: 700;
      line-height: 1.42;
      color: var(--ink);
      margin-bottom: 10px;
      word-break: break-word;
    }
    .ticket-meta {
      font-size: 11.5px;
      color: var(--muted);
      line-height: 1.55;
    }
    .dashed-callout {
      margin-top: 11px;
      padding: 7px 10px;
      border: 1.5px dashed var(--coral);
      color: var(--coral);
      font-size: 11.5px;
      font-weight: 700;
      line-height: 1.4;
    }
    .dashed-callout.amber {
      border-color: var(--yellow);
      color: var(--yellow);
    }
    .dashed-callout.mint {
      border-color: var(--mint);
      color: var(--mint);
    }

    /* GENERAL BRUTALIST CARDS, FORMS, TABLES */
    .box-card {
      background: var(--card);
      border: 2px solid var(--ink);
      box-shadow: var(--shadow);
      padding: 18px;
      min-width: 0;
    }
    .box-card h2, .box-card h3 {
      margin: 0 0 12px;
      font-family: var(--font-mono);
      font-size: 13.5px;
      font-weight: 800;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      flex-wrap: wrap;
    }
    .grid-2 {
      display: grid;
      grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
      gap: 20px;
    }
    .split-sidebar {
      display: grid;
      grid-template-columns: 300px minmax(0, 1fr);
      gap: 20px;
      align-items: start;
    }
    @media (max-width: 980px) {
      .grid-2, .split-sidebar { grid-template-columns: 1fr; }
    }

    input, select, textarea {
      background: #0a0a0a;
      border: 2px solid #555555;
      color: var(--ink);
      font-family: var(--font-mono);
      font-size: 12.5px;
      padding: 8px 10px;
      width: 100%;
    }
    input:focus, select:focus, textarea:focus {
      outline: none;
      border-color: var(--yellow);
      background: #0f0f0f;
    }
    textarea.code-editor {
      min-height: 180px;
      line-height: 1.45;
      resize: vertical;
    }
    label {
      display: block;
      font-size: 11px;
      font-weight: 800;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: var(--muted);
      margin-bottom: 5px;
    }
    .form-row {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 12px;
      margin-bottom: 12px;
    }
    pre {
      background: #060606;
      border: 1.5px solid #3a3a3a;
      padding: 12px;
      overflow-x: auto;
      white-space: pre-wrap;
      word-break: break-word;
      font-family: var(--font-mono);
      font-size: 12px;
      color: #e5e5e5;
      margin: 8px 0 0;
      max-height: 360px;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 12px;
    }
    th, td {
      text-align: left;
      padding: 9px 10px;
      border-bottom: 1px solid #2e2e2e;
      vertical-align: top;
    }
    th {
      color: var(--yellow);
      font-weight: 800;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      border-bottom: 2px solid var(--ink);
    }

    /* ACCORDIONS */
    details.brutal-acc {
      border: 2px solid var(--ink);
      background: var(--card);
      box-shadow: var(--shadow);
      margin-top: 18px;
    }
    details.brutal-acc > summary {
      cursor: pointer;
      padding: 12px 16px;
      font-weight: 800;
      font-size: 12.5px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      list-style: none;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      user-select: none;
    }
    details.brutal-acc > summary::-webkit-details-marker { display: none; }
    details.brutal-acc > summary::after { content: '[ + EXPAND ]'; color: var(--yellow); font-size: 11px; }
    details.brutal-acc[open] > summary {
      border-bottom: 2px solid var(--ink);
      background: #1d1d1d;
    }
    details.brutal-acc[open] > summary::after { content: '[ − COLLAPSE ]'; }
    details.brutal-acc .acc-body { padding: 16px; }

    /* SLIDE-OVER INSPECTOR DRAWER */
    .drawer-backdrop {
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.72);
      z-index: 70;
      display: flex;
      justify-content: flex-end;
    }
    .drawer-panel {
      width: min(820px, 95vw);
      height: 100vh;
      background: #0d0d0d;
      border-left: 3px solid var(--ink);
      box-shadow: -8px 0 0 #ffffff;
      display: flex;
      flex-direction: column;
      overflow: hidden;
    }
    .drawer-header {
      background: var(--yellow);
      color: var(--ink-dark);
      border-bottom: 3px solid var(--ink);
      padding: 14px 20px;
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 14px;
      flex-wrap: wrap;
    }
    .drawer-body {
      flex: 1;
      overflow-y: auto;
      padding: 20px;
    }

    /* STEPPER & THREAD CARDS IN DRAWER */
    .stepper {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 10px;
      margin-bottom: 16px;
    }
    @media (max-width: 680px) { .stepper { grid-template-columns: 1fr; } }
    .step-box {
      border: 2px solid #444444;
      background: var(--surface);
      padding: 10px 12px;
    }
    .step-box.done { border-color: var(--mint); }
    .step-box.active { border-color: var(--yellow); background: #1d1a0c; }
    .step-box.warn { border-color: var(--coral); background: #1f1010; }
    .step-title {
      font-weight: 800;
      font-size: 11.5px;
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
    .step-sub {
      font-size: 11.5px;
      color: var(--muted);
      margin-top: 3px;
    }

    .banner-box {
      border: 2px solid var(--ink);
      background: var(--card);
      box-shadow: var(--shadow-sm);
      padding: 14px 16px;
      margin-bottom: 16px;
    }
    .banner-box.coral { border-color: var(--coral); }
    .banner-box.amber { border-color: var(--yellow); }
    .banner-box.mint { border-color: var(--mint); }

    .thread-card {
      border: 2px solid #3c3c3c;
      background: var(--card);
      padding: 13px 15px;
      margin-bottom: 12px;
    }
    .thread-card.needs-human {
      border-color: var(--coral);
      box-shadow: 3px 3px 0 var(--coral);
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
      background: #0a0a0a;
      border: 1px solid #3a3a3a;
      font-size: 11px;
    }
    .brain-chip.running { border-color: var(--yellow); }
    .brain-chip.done { border-color: var(--mint); }

    .list-item {
      border: 2px solid #3a3a3a;
      background: #121212;
      padding: 10px 12px;
      margin-bottom: 8px;
      cursor: pointer;
    }
    .list-item:hover, .list-item.selected {
      border-color: var(--yellow);
      background: #1b1b1b;
      box-shadow: 3px 3px 0 var(--yellow);
    }

    .repo-row {
      border: 2px solid var(--ink);
      background: #121212;
      padding: 12px 14px;
      margin-bottom: 10px;
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 12px;
      flex-wrap: wrap;
    }

    /* PIXEL OFFICE PANE */
    .pixel-pane {
      background: #060606;
      border-left: 2px solid var(--ink);
      display: flex;
      flex-direction: column;
      height: calc(100vh - 58px);
      position: sticky;
      top: 58px;
    }
    #workspace-shell.mode-pixel .pixel-pane {
      border-left: none;
      position: static;
      height: calc(100vh - 58px);
    }
    .pixel-bar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 10px 16px;
      background: var(--surface);
      border-bottom: 2px solid var(--ink);
      flex-wrap: wrap;
    }
    .pixel-frame {
      flex: 1;
      width: 100%;
      border: none;
      background: #060606;
      min-height: 460px;
    }
    .pixel-fallback {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 24px;
      gap: 16px;
      text-align: center;
      background: #080808;
      overflow-y: auto;
    }
    #pixelFallbackCanvas {
      border: 2px solid var(--ink);
      box-shadow: var(--shadow);
      background: #0a0a0a;
      image-rendering: pixelated;
      max-width: 100%;
    }
    @media (max-width: 1100px) {
      #workspace-shell.mode-split { grid-template-columns: 1fr; }
      .pixel-pane { position: static; height: 580px; border-left: none; border-top: 2px solid var(--ink); }
    }

    .toast {
      position: fixed;
      bottom: 22px;
      right: 22px;
      background: var(--yellow);
      color: var(--ink-dark);
      border: 2px solid var(--ink);
      box-shadow: var(--shadow);
      padding: 10px 16px;
      font-weight: 800;
      font-size: 12.5px;
      z-index: 120;
      display: none;
      max-width: 460px;
    }
    .hidden { display: none !important; }
  </style>
</head>
<body>
  <header class="topbar">
    <div class="topbar-left">
      <div class="brand-mark" id="brand-logo">M1 // CI</div>
      <nav class="mode-nav" aria-label="Subsystem Switcher">
        <button id="nav-app-tender" onclick="switchApp('tender')">
          <span>CODE TENDER</span>
          <span class="mode-count" id="nav-tender-badge">0</span>
        </button>
        <button id="nav-app-reviewer" onclick="switchApp('reviewer')">
          <span>PR REVIEWER</span>
          <span class="mode-count" id="nav-reviewer-badge">0</span>
        </button>
        <button id="nav-app-pixel" onclick="switchApp('pixel')">
          <span>PIXEL OFFICE</span>
          <span class="mode-count" id="nav-pixel-badge">...</span>
        </button>
      </nav>
    </div>
    <div class="topbar-right">
      <span class="status-chip" id="pill-poll-mode"><span class="sq-dot"></span>2M AUTO-WATCH</span>
      <span class="status-chip" id="pill-system" title="AI Runners &amp; Host Power"><span class="sq-dot"></span>CLAUDE · AGY · M1</span>
      <button class="secondary" id="btn-split-pixel" onclick="toggleSplitPixel()" title="Dock Pixel Agents Office side-by-side">SPLIT OFFICE</button>
      <button id="primary-poll-btn" onclick="triggerPrimaryAction()">↻ SYNC PULLS</button>
    </div>
  </header>

  <div id="workspace-shell">
    <div class="workspace-left" id="workspace-left">

      <!-- ========================================================= -->
      <!-- APP 1: CODE TENDER (BRUTALIST STATUS + 3-COL KANBAN)      -->
      <!-- ========================================================= -->
      <div id="app-tender" class="hidden">
        <div class="subnav">
          <div class="subnav-tabs" id="tender-tabs">
            <button class="active" data-tab="tender-prs" onclick="switchTenderTab('tender-prs')">01 // LIVE TENDER BOARD</button>
            <button data-tab="tender-repos" onclick="switchTenderTab('tender-repos')">02 // REPOS, RULES &amp; LOGS</button>
          </div>
          <div style="font-size:11.5px;color:var(--muted);">
            CLICK ANY PULL CARD TO INSPECT THREADS, STAGED DIFFS, OR REPLY &amp; RESOLVE
          </div>
        </div>

        <main class="page">
          <!-- TENDER VIEW 1: LIVE BOARD -->
          <section id="tab-tender-prs">
            <div class="masthead">
              <div class="masthead-top">
                <h1 class="masthead-title">CODE<br/>TENDER</h1>
                <div class="masthead-stat">
                  <div class="masthead-num" id="tender-hero-num">0</div>
                  <div class="masthead-caption" id="tender-hero-caption">OPEN WATCHED PULLS</div>
                </div>
              </div>
              <div class="masthead-ticker" id="tender-hero-ticker">
                <span>SNAPSHOT LOADING...</span>
              </div>
            </div>

            <div class="narrative-strip" id="tender-narrative">
              Syncing watched repositories and review threads...
            </div>

            <div class="watch-strip">
              <span class="watch-strip-label">WATCH PULL //</span>
              <input
                id="watch-pr-input"
                placeholder="PASTE GITHUB PR URL OR OWNER/REPO#123..."
                onkeydown="if(event.key==='Enter') watchPrFromInput()"
              />
              <button onclick="watchPrFromInput()">+ WATCH PR</button>
              <button class="secondary" id="btn-open-pr-picker" onclick="toggleOpenPrPicker()">PICK OPEN PR</button>
              <button class="secondary" id="tender-mode-summary" onclick="switchTenderTab('tender-repos')" title="Adjust Code Tender delivery &amp; poller settings">AUTO-TEND: ON</button>
            </div>

            <div id="open-pr-picker-box" class="box-card hidden" style="margin-bottom:24px;">
              <h2>
                <span>OPEN PULL REQUESTS ON GITHUB</span>
                <div style="display:flex;gap:8px;">
                  <button class="secondary" onclick="loadOpenPrPicker()">↻ REFRESH</button>
                  <button class="secondary" onclick="toggleOpenPrPicker(false)">✕ CLOSE</button>
                </div>
              </h2>
              <div id="open-pr-picker-list" style="max-height:300px;overflow-y:auto;"></div>
            </div>

            <div class="sec-head">
              <span class="sec-tag">VITALS</span>
              <span class="sec-sub">Counts from the latest 2m M1 snapshot</span>
            </div>
            <div class="vitals-grid" id="tender-vitals-grid"></div>

            <div class="sec-head">
              <span class="sec-tag">TENDER BOARD</span>
              <span class="sec-sub">Watch open pulls &rarr; detect review comments &rarr; fix locally, verify checks &amp; push</span>
            </div>
            <div class="board-grid" id="tender-board-grid"></div>
          </section>

          <!-- TENDER VIEW 2: REPOS, RULES & LOGS -->
          <section id="tab-tender-repos" class="hidden">
            <div class="sec-head" style="margin-top:8px;">
              <span class="sec-tag">WATCHED REPOS</span>
              <span class="sec-sub">Open PRs on Auto-Watch repositories are tracked automatically every 2 minutes</span>
            </div>
            <div class="grid-2" style="margin-bottom:22px;">
              <div class="box-card">
                <h2>
                  <span>CONFIGURED REPOSITORIES</span>
                  <span style="font-size:11px;color:var(--muted);font-weight:400;">CLICK EDIT TO LOAD INTO FORM</span>
                </h2>
                <div id="tender-repos-list"></div>
              </div>

              <div class="box-card">
                <h2 id="tr-form-heading">ADD / UPDATE REPOSITORY</h2>
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
                  <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--ink);">
                    <input type="checkbox" id="tr-tender-enabled" checked style="width:auto;" />
                    <span>AUTO-WATCH &amp; TEND OPEN PRS</span>
                  </label>
                  <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--ink);">
                    <input type="checkbox" id="tr-enabled" style="width:auto;" />
                    <span>ALSO RUN 8-BRAIN REVIEWER</span>
                  </label>
                </div>
                <div style="margin-bottom:12px;">
                  <label>Local App Checkout on M1 (for full repo context &amp; local checks)</label>
                  <div style="display:flex;gap:6px;flex-wrap:wrap;">
                    <input id="tr-local-path" style="flex:1;min-width:190px;" placeholder="~/apps/web-app" />
                    <button type="button" class="secondary" onclick="openM1Browser('tr')">BROWSE M1</button>
                    <button type="button" class="secondary" onclick="loadGitInfoFromPath('tr')">LOAD GIT</button>
                    <button type="button" class="secondary" onclick="detectLocalRepo('tr')">DETECT</button>
                  </div>
                </div>
                <div style="margin-bottom:12px;">
                  <label>Local Check Command (runs before push; auto-inferred if blank)</label>
                  <input id="tr-check" placeholder="pnpm --filter @app/web typecheck" />
                </div>
                <details class="brutal-acc" style="margin:12px 0 14px;box-shadow:none;">
                  <summary style="padding:9px 12px;font-size:11.5px;">ADVANCED REPO OPTIONS (ACCOUNT &amp; DRIVE)</summary>
                  <div class="acc-body" style="padding:12px;">
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
                <button onclick="submitWatchedRepoForm('tr')">SAVE REPOSITORY</button>
              </div>
            </div>

            <div class="box-card">
              <h2><span>CORE TENDER BEHAVIOR</span></h2>
              <div class="form-row" style="margin-bottom:0;">
                <div>
                  <label>When Review Comments Land</label>
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
                  <button style="width:100%;justify-content:center;" onclick="saveTenderSettings()">SAVE SETTINGS</button>
                </div>
              </div>
            </div>

            <details class="brutal-acc">
              <summary><span>ADVANCED CONCURRENCY &amp; SAFETY LIMITS</span></summary>
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
                <button onclick="saveTenderSettings()">SAVE ADVANCED SETTINGS</button>
              </div>
            </details>

            <details class="brutal-acc" ontoggle="if(this.open){loadTenderKb();loadTenderAgentDetail(SELECTED_TENDER_AGENT);}">
              <summary><span>CONTINUOUS-LEARNING KB &amp; AGENT PROMPTS (01..03)</span></summary>
              <div class="acc-body">
                <div class="box-card" style="box-shadow:none;margin-bottom:16px;background:#101010;">
                  <h3>+ APPEND LEARNED RULE</h3>
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
                  <button onclick="submitTenderKbRule()">+ APPEND RULE</button>
                </div>

                <div class="grid-2">
                  <div>
                    <label>Patterns Fixed (`tender-agents/kb/patterns-fixed.md`)</label>
                    <textarea id="tkb-patterns" class="code-editor" style="min-height:150px;"></textarea>
                    <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('patterns-fixed', 'tkb-patterns')">SAVE PATTERNS-FIXED.MD</button>
                  </div>
                  <div>
                    <label>Local Check Recipes (`tender-agents/kb/check-recipes.md`)</label>
                    <textarea id="tkb-recipes" class="code-editor" style="min-height:150px;"></textarea>
                    <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('check-recipes', 'tkb-recipes')">SAVE CHECK-RECIPES.MD</button>
                  </div>
                </div>
                <div style="margin:14px 0 20px;">
                  <label>Do Not Repeat Guardrails (`tender-agents/kb/do-not-repeat.md`)</label>
                  <textarea id="tkb-dnr" class="code-editor" style="min-height:130px;"></textarea>
                  <button class="secondary" style="margin-top:6px;" onclick="saveTenderKbFile('do-not-repeat', 'tkb-dnr')">SAVE DO-NOT-REPEAT.MD</button>
                </div>

                <div class="split-sidebar">
                  <div class="box-card" style="box-shadow:none;">
                    <h3>TENDER AGENTS (01..03)</h3>
                    <div id="tender-agents-list-box"></div>
                  </div>
                  <div class="box-card" style="box-shadow:none;">
                    <h3 id="tender-agent-detail-title">TENDER AGENT PROMPT &amp; GUARDRAILS</h3>
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
                    <button onclick="saveTenderAgentPrompt()">SAVE AGENT PROMPT &amp; NOTES</button>
                  </div>
                </div>
              </div>
            </details>

            <details class="brutal-acc">
              <summary><span>LIVE POLLER LOGS &amp; RECENT 2M POLL HISTORY</span></summary>
              <div class="acc-body">
                <pre id="tender-logs-box">Loading tender logs...</pre>
                <h3 style="margin:16px 0 8px;">RECENT 2M GRAPHQL POLL TICKS <span class="rect-tag mint">0 AI TOKENS / POLL</span></h3>
                <div style="max-height:220px;overflow-y:auto;">
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

      <!-- ========================================================= -->
      <!-- APP 2: MULTI-LENS CODE REVIEWER (BRUTALIST BOARD)         -->
      <!-- ========================================================= -->
      <div id="app-reviewer">
        <div class="subnav">
          <div class="subnav-tabs" id="reviewer-tabs">
            <button class="active" data-tab="board" onclick="switchReviewerTab('board')">01 // REVIEW BOARD</button>
            <button data-tab="repos" onclick="switchReviewerTab('repos')">02 // REPOS &amp; ENGINE</button>
            <button data-tab="brains" onclick="switchReviewerTab('brains')">03 // 8 BRAINS &amp; LIVING DOCS</button>
          </div>
          <div style="font-size:11.5px;color:var(--muted);" id="parallel-cap-label">
            8 SPECIALIST BRAINS IN PARALLEL · CLAUDE ⇄ AGY
          </div>
        </div>

        <main class="page">
          <!-- REVIEWER VIEW 1: UNIFIED REVIEW BOARD -->
          <section id="tab-rev-board">
            <div class="masthead">
              <div class="masthead-top">
                <h1 class="masthead-title">MULTI-LENS<br/>REVIEWER</h1>
                <div class="masthead-stat">
                  <div class="masthead-num" id="rev-hero-num">0</div>
                  <div class="masthead-caption" id="rev-hero-caption">TRACKED PULL REQUESTS</div>
                </div>
              </div>
              <div class="masthead-ticker" id="rev-hero-ticker">
                <span>SNAPSHOT LOADING...</span>
              </div>
            </div>

            <div class="narrative-strip" id="rev-narrative">
              Loading multi-lens review state...
            </div>

            <div class="sec-head">
              <span class="sec-tag">VITALS</span>
              <span class="sec-sub">Multi-lens review verdicts, open blockers &amp; active 8-brain workers</span>
            </div>
            <div class="vitals-grid" id="rev-vitals-grid"></div>

            <div class="sec-head">
              <span class="sec-tag">REVIEW BOARD</span>
              <span class="sec-sub">Click any pull card to inspect observations, traceability matrix &amp; Google Drive spec docs</span>
            </div>
            <div class="board-grid" id="rev-board-grid"></div>
          </section>

          <!-- REVIEWER VIEW 2: REPOS, ENGINE SETTINGS & LOGS -->
          <section id="tab-rev-repos" class="hidden">
            <div class="sec-head" style="margin-top:8px;">
              <span class="sec-tag">REPOS &amp; ENGINE</span>
              <span class="sec-sub">Watched repositories (`config/repos.toml`), fast model parallelism &amp; live poller stream</span>
            </div>

            <div class="box-card" style="margin-bottom:20px;overflow-x:auto;">
              <h2>WATCHED REPOSITORIES</h2>
              <table>
                <thead><tr><th>Repository</th><th>Branch</th><th>Identity Policy</th><th>Review Event</th><th>Reviewer</th><th>Tender</th><th>Local Checkout / Check</th><th>Actions</th></tr></thead>
                <tbody id="repos-tbody"></tbody>
              </table>
            </div>

            <div class="grid-2" style="margin-bottom:20px;">
              <div class="box-card">
                <h2>ADD / UPDATE WATCHED REPOSITORY</h2>
                <div class="form-row">
                  <div><label>Owner / Repo (or GitHub URL)</label><input id="r-name" placeholder="acme-org/web-app" onblur="normalizeRepoInputAndDetect('r')" /></div>
                  <div><label>Default Branch</label><input id="r-branch" value="main" /></div>
                </div>
                <div class="form-row">
                  <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--ink);">
                    <input type="checkbox" id="r-enabled" checked style="width:auto;" />
                    <span>RUN MULTI-LENS CODE REVIEWER</span>
                  </label>
                  <label style="display:flex;align-items:center;gap:8px;cursor:pointer;color:var(--ink);">
                    <input type="checkbox" id="r-tender-enabled" checked style="width:auto;" />
                    <span>RUN CODE TENDER (AUTO-FIX)</span>
                  </label>
                </div>
                <div style="margin-bottom:12px;">
                  <label>Local App Checkout Path on M1</label>
                  <div style="display:flex;gap:8px;flex-wrap:wrap;">
                    <input id="r-local-path" style="flex:1;min-width:190px;" placeholder="~/apps/web-app" />
                    <button type="button" class="secondary" onclick="openM1Browser('r')">BROWSE M1</button>
                    <button type="button" class="secondary" onclick="loadGitInfoFromPath('r')">LOAD GIT</button>
                    <button type="button" class="secondary" onclick="detectLocalRepo('r')">AUTO-DETECT</button>
                  </div>
                </div>
                <div style="margin-bottom:12px;">
                  <label>Local Verification Check Command</label>
                  <input id="r-check" placeholder="npm run typecheck && npm test" />
                </div>
                <div class="form-row">
                  <div>
                    <label>Account Policy</label>
                    <select id="r-acct-pol">
                      <option value="swap">swap (primary ⇄ secondary)</option>
                      <option value="forced">forced (single account)</option>
                    </select>
                  </div>
                  <div><label>Forced Account (if forced)</label><input id="r-forced" placeholder="optional" /></div>
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
                <button onclick="submitWatchedRepoForm('r')">SAVE WATCHED REPOSITORY</button>
              </div>

              <div class="box-card">
                <h2>FAST MODELS, LOAD-SPLITTING &amp; PARALLELISM</h2>
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
                    <label>Parallel Brains per PR (`8` = All)</label>
                    <input type="number" id="cfg-max-brains" min="1" max="8" value="8" />
                  </div>
                  <div>
                    <label>2m Poller Token Guard</label>
                    <select id="cfg-poll-only">
                      <option value="false">Event-Only AI (0 AI on idle polls)</option>
                      <option value="true">Strict Poll-Only (Pause Auto-AI)</option>
                    </select>
                  </div>
                </div>
                <button onclick="saveAgentSettings()">SAVE FAST MODEL &amp; PARALLEL SETTINGS</button>

                <h3 style="margin-top:20px;">RECENT 2M GRAPHQL POLLS <span class="rect-tag mint">0 AI TOKENS / POLL</span></h3>
                <div style="max-height:200px;overflow-y:auto;">
                  <table>
                    <thead><tr><th>Tick</th><th>Engine</th><th>Duration</th><th>Started (UTC)</th></tr></thead>
                    <tbody id="history-tbody"></tbody>
                  </table>
                </div>
              </div>
            </div>

            <div class="box-card">
              <h2><span>LIVE POLLER &amp; WORKER STREAM</span><span class="rect-tag muted">AUTO-UPDATING 2.5S</span></h2>
              <pre id="live-logs-box">Loading logs...</pre>
            </div>
          </section>

          <!-- REVIEWER VIEW 3: 8 BRAINS & LIVING DOCS -->
          <section id="tab-rev-brains" class="hidden">
            <div class="sec-head" style="margin-top:8px;">
              <span class="sec-tag">8 SPECIALIST BRAINS</span>
              <span class="sec-sub">Isolated knowledge bases (`lessons.md`, `false-positives.md`, `exemplars.md`)</span>
            </div>
            <div class="split-sidebar" style="margin-bottom:28px;">
              <div class="box-card">
                <h2>REVIEW BRAINS (01..08)</h2>
                <div id="brains-list-box"></div>
              </div>
              <div class="box-card">
                <h2 id="brain-detail-title">BRAIN DETAILS &amp; ISOLATED KB</h2>
                <div class="box-card" style="box-shadow:none;background:#101010;margin-bottom:16px;">
                  <h3>+ APPEND RULE TO THIS BRAIN'S KB</h3>
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
                  <button onclick="submitKbRule()">+ APPEND TO BRAIN KB</button>
                </div>
                <div id="brain-files-box"></div>
              </div>
            </div>

            <div class="sec-head">
              <span class="sec-tag">LIVING DOCS</span>
              <span class="sec-sub">Continuously updated repository architecture &amp; conventions (`living-docs/`)</span>
            </div>
            <div class="box-card">
              <h2>
                <span>LIVING REPOSITORY CONTEXT</span>
                <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
                  <select id="docs-repo-select" onchange="loadLivingDocs()" style="width:260px;"></select>
                  <button class="secondary" onclick="syncLivingDocsDrive()">SYNC TO GOOGLE DRIVE</button>
                </div>
              </h2>
              <div id="living-docs-box"></div>
            </div>
          </section>
        </main>
      </div>
    </div>

    <!-- ========================================================= -->
    <!-- APP 3 / SPLIT DOCK: PIXEL AGENTS VISUAL OFFICE            -->
    <!-- ========================================================= -->
    <aside id="pixel-office-pane" class="pixel-pane hidden">
      <div class="pixel-bar">
        <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;">
          <strong>PIXEL AGENTS OFFICE</strong>
          <span class="rect-tag muted" id="pixel-conn-badge">CHECKING :7421</span>
          <span id="pixel-workers-pill" style="font-size:11.5px;color:var(--muted);"></span>
        </div>
        <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;">
          <button class="secondary" style="padding:4px 9px;font-size:11px;" onclick="startPixelAgentsServer()">▶ START :7421</button>
          <button class="secondary" style="padding:4px 9px;font-size:11px;" onclick="loadPixelOffice(true)">↻ RELOAD</button>
          <a id="pixel-open-ext" class="btn secondary" style="padding:4px 9px;font-size:11px;" href="#" target="_blank" rel="noopener">↗ OPEN TAB</a>
        </div>
      </div>
      <iframe id="pixelOfficeFrame" class="pixel-frame hidden" title="Pixel Agents Visual Office"></iframe>
      <div id="pixelFallbackPane" class="pixel-fallback">
        <canvas id="pixelFallbackCanvas" width="760" height="320"></canvas>
        <div style="max-width:640px;">
          <div style="font-weight:800;font-size:14px;text-transform:uppercase;margin-bottom:6px;" id="pixel-fallback-title">LIVE M1 AGENT FLOORPLAN PREVIEW</div>
          <div style="font-size:12px;color:var(--muted);margin-bottom:14px;" id="pixel-fallback-desc">
            Connecting to standalone <code>pixel-agents v1.6.0</code> on port <code>7421</code>...
          </div>
          <div style="display:flex;gap:10px;justify-content:center;flex-wrap:wrap;">
            <button onclick="startPixelAgentsServer()">▶ START PIXEL SERVER (:7421)</button>
            <button class="secondary" onclick="loadPixelOffice(true)">↻ RETRY CONNECTION</button>
          </div>
        </div>
      </div>
    </aside>
  </div>

  <!-- ========================================================= -->
  <!-- BRUTALIST SLIDE-OVER INSPECTOR DRAWER (TENDER & REVIEWER) -->
  <!-- ========================================================= -->
  <div id="inspector-drawer" class="drawer-backdrop hidden" onclick="if(event.target === this) closeInspectorDrawer()">
    <div class="drawer-panel" role="dialog" aria-modal="true">
      <div class="drawer-header" id="inspector-header"></div>
      <div class="drawer-body" id="inspector-body"></div>
    </div>
  </div>

  <!-- ========================================================= -->
  <!-- M1 DIRECTORY BROWSER MODAL                                -->
  <!-- ========================================================= -->
  <div id="m1-browser-modal" class="hidden" style="position:fixed;inset:0;background:rgba(0,0,0,0.82);z-index:100;display:none;align-items:center;justify-content:center;padding:20px;" onclick="if(event.target === this) closeM1Browser()">
    <div class="box-card" style="width:min(880px,100%);max-height:86vh;display:flex;flex-direction:column;gap:12px;overflow:hidden;">
      <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;">
        <h2 style="margin:0;">SELECT REPOSITORY FOLDER ON M1</h2>
        <button class="secondary" onclick="closeM1Browser()">✕ CLOSE</button>
      </div>
      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
        <button id="m1-up-btn" class="secondary" onclick="navigateM1Up()">⬆ UP</button>
        <input id="m1-path-input" style="flex:1;min-width:240px;" placeholder="~/" onkeydown="if(event.key==='Enter') navigateM1Browser(this.value)" />
        <button class="secondary" onclick="navigateM1Browser(document.getElementById('m1-path-input').value)">OPEN PATH</button>
        <button onclick="pickM1Folder(document.getElementById('m1-path-input').value)">✓ USE FOLDER</button>
      </div>
      <div id="m1-browser-status" style="font-size:12px;color:var(--muted);"></div>
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;overflow:hidden;min-height:280px;">
        <div style="border:2px solid #3a3a3a;padding:12px;overflow-y:auto;">
          <h3 style="margin:0 0 10px;">DISCOVERED GIT REPOS ON M1</h3>
          <div id="m1-discovered-list" style="display:flex;flex-direction:column;gap:8px;"></div>
        </div>
        <div style="border:2px solid #3a3a3a;padding:12px;overflow-y:auto;">
          <h3 style="margin:0 0 10px;">FOLDERS IN DIRECTORY</h3>
          <div id="m1-dir-list" style="display:flex;flex-direction:column;gap:8px;"></div>
        </div>
      </div>
    </div>
  </div>

  <div class="toast" id="toast"></div>

  <script>
    let STATE = null;
    let ACTIVE_APP = 'reviewer';
    let ACTIVE_REVIEWER_TAB = 'board';
    let ACTIVE_TENDER_TAB = 'tender-prs';
    let SELECTED_PR_KEY = null;
    let SELECTED_TENDER_PR_KEY = null;
    let DRAWER_MODE = null; // 'tender' | 'reviewer' | null
    let OBS_FILTER = 'all';
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
    const TENDER_REPLY_DRAFTS = {};
    const OPEN_ACCORDIONS = new Set();

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

    function escJsAttr(s) {
      return String(s ?? '')
        .replace(/\\/g, '\\\\')
        .replace(/'/g, "\\'")
        .replace(/&/g, '&amp;')
        .replace(/"/g, '&quot;');
    }

    function formatUtcSnapshot() {
      const months = ['JAN','FEB','MAR','APR','MAY','JUN','JUL','AUG','SEP','OCT','NOV','DEC'];
      const d = new Date();
      const day = String(d.getUTCDate()).padStart(2, '0');
      const mon = months[d.getUTCMonth()];
      const yr = d.getUTCFullYear();
      const hh = String(d.getUTCHours()).padStart(2, '0');
      const mm = String(d.getUTCMinutes()).padStart(2, '0');
      return `SNAPSHOT ${day} ${mon} ${yr} ${hh}:${mm} UTC`;
    }

    function shortRepo(full) {
      const s = String(full || '');
      const idx = s.indexOf('/');
      return idx >= 0 ? s.slice(idx + 1) : s;
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

    function renderVitalTile(tone, label, count, zeroAsDash, footHtml) {
      const showDash = zeroAsDash && Number(count) === 0;
      return `<div class="vital-tile ${tone}">
        <div class="vital-label">${label}</div>
        ${showDash ? '<div class="vital-dash" aria-label="0"></div>' : `<div class="vital-val">${count}</div>`}
        <div class="vital-foot">${footHtml}</div>
      </div>`;
    }

    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        const m1 = document.getElementById('m1-browser-modal');
        if (m1 && !m1.classList.contains('hidden')) {
          closeM1Browser();
          return;
        }
        if (DRAWER_MODE) {
          closeInspectorDrawer();
          return;
        }
        if (OPEN_PR_PICKER_VISIBLE) {
          toggleOpenPrPicker(false);
        }
      }
    });

    function closeInspectorDrawer() {
      DRAWER_MODE = null;
      document.getElementById('inspector-drawer').classList.add('hidden');
      renderTenderBoard();
      renderReviewerBoard();
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
      splitBtn.textContent = SPLIT_PIXEL ? '✕ HIDE OFFICE' : 'SPLIT OFFICE';

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

      document.getElementById('nav-app-reviewer').classList.toggle('active-mode', ACTIVE_APP === 'reviewer');
      document.getElementById('nav-app-tender').classList.toggle('active-mode', isTender);
      document.getElementById('nav-app-pixel').classList.toggle('active-mode', isPixel);

      document.getElementById('brand-logo').textContent = isPixel ? 'M1 // PX' : (isTender ? 'M1 // CT' : 'M1 // CR');

      const primaryBtn = document.getElementById('primary-poll-btn');
      primaryBtn.textContent = isPixel
        ? '↻ RELOAD OFFICE'
        : (isTender ? '↻ SYNC PULLS' : '⚡ SYNC & REVIEW');

      if (DRAWER_MODE && ((isTender && DRAWER_MODE !== 'tender') || (!isTender && DRAWER_MODE !== 'reviewer') || isPixel)) {
        closeInspectorDrawer();
      }

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
      const normalized = (tab === 'repos' || tab === 'brains') ? tab : 'board';
      ACTIVE_REVIEWER_TAB = normalized;
      document.querySelectorAll('#reviewer-tabs button').forEach(b => {
        b.classList.toggle('active', b.dataset.tab === normalized);
      });
      document.getElementById('tab-rev-board').classList.toggle('hidden', normalized !== 'board');
      document.getElementById('tab-rev-repos').classList.toggle('hidden', normalized !== 'repos');
      document.getElementById('tab-rev-brains').classList.toggle('hidden', normalized !== 'brains');
      if (normalized === 'brains') {
        loadBrainDetail(SELECTED_BRAIN);
        loadLivingDocs();
      }
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
      closeInspectorDrawer();
      switchApp('tender');
      switchTenderTab('tender-prs');
      const key = `${repo}#${prNumber}`;
      const exists = (STATE?.tender_pull_requests || []).some(p => p.key === key);
      if (exists) {
        selectTenderPr(key);
      } else {
        await setPrWatched(repo, prNumber, true);
        selectTenderPr(key);
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
        renderReviewerSettingsAndLogs();
        renderReviewerBoard();
        renderRepos();
        renderBrainsSidebar();
        populateDocsRepoSelect();
        renderTenderSettingsAndLogs();
        renderTenderBoard();
        renderTenderReposAndAgents();
        if (DRAWER_MODE === 'tender' && SELECTED_TENDER_PR_KEY) {
          renderTenderDrawer();
        } else if (DRAWER_MODE === 'reviewer' && SELECTED_PR_KEY) {
          renderReviewerDrawer();
        }
      } catch (e) {
        console.error(e);
      }
    }

    function renderTopStatusBar() {
      if (!STATE) return;
      const revActive = (STATE.active_workers || []).length;
      const tendActive = (STATE.tender_active_workers || []).length;
      const watchedOpenCount = (STATE.tender_pull_requests || []).filter(p => p.is_open !== false && p.watched !== false).length;

      document.getElementById('nav-reviewer-badge').textContent = revActive ? `${revActive} LIVE` : String((STATE.pull_requests || []).length);
      document.getElementById('nav-tender-badge').textContent = tendActive ? `${tendActive} LIVE` : String(watchedOpenCount);

      const pmPill = document.getElementById('pill-poll-mode');
      const isTender = ACTIVE_APP === 'tender';
      const pollPaused = isTender ? Boolean(STATE.tender?.poll_only_mode) : Boolean(STATE.agents?.poll_only_mode);
      if (pollPaused) {
        pmPill.innerHTML = `<span class="sq-dot warn"></span>AUTO-AI PAUSED`;
      } else if (isTender && STATE.tender && !STATE.tender.enabled) {
        pmPill.innerHTML = `<span class="sq-dot warn"></span>MANUAL MODE`;
      } else {
        pmPill.innerHTML = `<span class="sq-dot"></span>2M AUTO-WATCH`;
      }

      const sysPill = document.getElementById('pill-system');
      if (sysPill) {
        const cOk = Boolean(STATE.runners?.claude_ready);
        const aOk = Boolean(STATE.runners?.agy_ready);
        const pOk = Boolean(STATE.host?.ac_power);
        const allOk = cOk && aOk && pOk;
        const pwrShort = pOk ? 'AC' : 'BATT';
        sysPill.innerHTML = `<span class="sq-dot ${allOk ? '' : 'warn'}"></span>CLAUDE ${cOk ? '✓' : '!'} · AGY ${aOk ? '✓' : '!'} · ${pwrShort}`;
        sysPill.title = `${STATE.host?.summary || 'M1 Host'} | claude: ${cOk ? 'ready' : 'sign-in'} | agy: ${aOk ? 'ready' : 'sign-in'}`;
      }
      updatePixelWorkersSummary();
    }

    // =========================================================
    // MULTI-LENS CODE REVIEWER — BRUTALIST BOARD & INSPECTOR
    // =========================================================

    function renderReviewerSettingsAndLogs() {
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

      document.getElementById('parallel-cap-label').textContent =
        `UP TO ${STATE.agents.max_parallel_prs} ASYNC PRS · ${STATE.agents.max_parallel_brains_per_pr} BRAINS/WAVE · NEXT LEAD: ${(STATE.next_lead_runner || 'claude').toUpperCase()}`;

      document.getElementById('live-logs-box').textContent = STATE.recent_logs || '(no logs yet)';
      document.getElementById('history-tbody').innerHTML = (STATE.run_history || []).map(h => `
        <tr>
          <td><span class="rect-tag ${h.status === 'pass' ? 'mint' : 'coral'}">${esc(h.status)}</span></td>
          <td><code>GRAPHQL (0 AI)</code> <code>${esc((h.commit_sha || '').slice(0, 7))}</code></td>
          <td>${h.duration_secs}s</td>
          <td style="color:var(--muted);">${new Date(h.started_epoch_secs * 1000).toISOString().replace('T', ' ').slice(0, 19)}</td>
        </tr>
      `).join('');
    }

    function renderReviewerBoard() {
      if (!STATE) return;
      const prs = STATE.pull_requests || [];
      const workers = STATE.active_workers || [];
      const enabledRepos = (STATE.repos || []).filter(r => r.enabled);
      const passList = prs.filter(p => p.verdict === 'PASS');
      const changesList = prs.filter(p => p.verdict !== 'PASS');

      let openObs = 0, critHigh = 0;
      prs.forEach(p => (p.observations || []).forEach(o => {
        if (o.status === 'open') {
          openObs++;
          if (o.severity === 'CRITICAL' || o.severity === 'HIGH') critHigh++;
        }
      }));

      document.getElementById('rev-hero-num').textContent = String(prs.length);
      document.getElementById('rev-hero-caption').textContent =
        workers.length > 0 ? `${workers.length} REVIEW SESSIONS IN-FLIGHT` : `TRACKED PULL REQUESTS`;

      const repoSummary = enabledRepos.length
        ? enabledRepos.map(r => r.full_name).join(', ')
        : ((STATE.repos || []).length ? 'REVIEWER PAUSED ON REPOS' : 'NO REPOS CONFIGURED');

      document.getElementById('rev-hero-ticker').innerHTML = `
        <span>${esc(formatUtcSnapshot())}</span>
        <span>MODEL: ${esc((STATE.agents?.claude_model || 'sonnet').toUpperCase())} (${esc((STATE.agents?.reasoning_effort || 'medium').toUpperCase())})</span>
        <span>PARALLEL: ${STATE.agents?.max_parallel_brains_per_pr || 8} BRAINS</span>
        <span>WATCHING: ${esc(repoSummary.toUpperCase())}</span>
      `;

      const lastPollStatus = (STATE.run_history && STATE.run_history[0]) ? STATE.run_history[0].status : 'idle';
      document.getElementById('rev-narrative').innerHTML =
        `<span class="inv-chip">${passList.length}</span> of <span class="inv-chip">${prs.length}</span> reviewed pull requests hold a clean <span class="inv-chip mint">PASS</span> verdict. ` +
        `<span class="inv-chip ${critHigh > 0 ? 'coral' : ''}">${critHigh}</span> blocking CRITICAL/HIGH observations and <span class="inv-chip">${openObs}</span> total observations remain open. ` +
        `Latest 2m GraphQL poller tick: <span class="inv-chip">${esc(lastPollStatus)}</span>.`;

      document.getElementById('rev-vitals-grid').innerHTML = [
        renderVitalTile('coral', 'CRITICAL / HIGH OPEN', critHigh, true,
          critHigh > 0 ? `blocking observations across ${changesList.length} PRs` : 'zero blocking CRITICAL/HIGH findings'),
        renderVitalTile('amber', 'OPEN OBSERVATIONS', openObs, true,
          openObs > 0 ? 'inline findings waiting resolution' : 'no unresolved review observations'),
        renderVitalTile('amber', 'IN-FLIGHT SESSIONS', workers.length, true,
          `capacity ${STATE.agents?.max_parallel_prs || 8} async PRs`),
        renderVitalTile('dark', 'CHANGES NEEDED', changesList.length, false,
          `PRs with open findings or non-PASS`),
        renderVitalTile('dark', 'VERIFIED PASS', passList.length, false,
          `${enabledRepos.length} active reviewer repo${enabledRepos.length === 1 ? '' : 's'}`)
      ].join('');

      const col1Cards = workers.length ? workers.map(w => {
        const chips = Object.entries(w.brains || {}).map(([bName, bState]) => `
          <div class="brain-chip ${esc(bState.status)}">
            <span>${esc(bName.replace(/^\d+-/, ''))}</span>
            <span>${esc((bState.runner || '').toUpperCase())} · ${esc((bState.status || '').toUpperCase())}</span>
          </div>
        `).join('');
        return `<div class="ticket-card" onclick="selectPr('${esc(w.repo + '#' + w.pr_number)}')">
          <div class="ticket-top">
            <div>
              <span class="ticket-id">#${w.pr_number}</span>
              <span class="ticket-repo">${esc(w.repo)}</span>
            </div>
            <span class="rect-tag yellow">${esc(w.phase || 'RUNNING')}</span>
          </div>
          <div class="ticket-title">${esc(w.title)}</div>
          <div class="ticket-meta">
            SHA: ${esc((w.head_sha || '').slice(0, 7))} · Lead: ${esc(w.lead_runner)} ⇄ ${esc(w.alt_runner)}
          </div>
          <div class="brain-chips">${chips}</div>
        </div>`;
      }).join('') : `<div class="ticket-card standby-card">
        <div class="ticket-top">
          <span class="ticket-id" style="font-size:14px;">STANDBY</span>
          <span class="rect-tag muted">0 IN-FLIGHT</span>
        </div>
        <div class="ticket-meta">
          Zero active 8-brain review workers right now. Next PR open/synchronize event will spawn an async session automatically.
        </div>
      </div>`;

      const renderRevPrCard = (p) => {
        const openCnt = (p.observations || []).filter(o => o.status === 'open').length;
        const blkCnt = (p.observations || []).filter(o => o.status === 'open' && (o.severity === 'CRITICAL' || o.severity === 'HIGH')).length;
        const isPass = p.verdict === 'PASS';
        const selectedCls = (DRAWER_MODE === 'reviewer' && SELECTED_PR_KEY === p.key) ? 'selected-card' : '';
        return `<div class="ticket-card ${selectedCls}" onclick="selectPr('${escJsAttr(p.key)}')">
          <div class="ticket-top">
            <div>
              <span class="ticket-id">#${p.pr_number}</span>
              <span class="ticket-repo">${esc(shortRepo(p.repo))}</span>
            </div>
            <span class="rect-tag ${isPass ? 'mint' : 'coral'}">${esc(p.verdict || 'PENDING')}</span>
          </div>
          <div class="ticket-title">${esc(p.title)}</div>
          <div class="ticket-meta">
            Who: @${esc(p.author || '-')} · Runs: ${p.runs_count || 0}<br/>
            Branch: ${esc(p.head_ref || '-')} &rarr; ${esc(p.base_ref || '-')}
          </div>
          ${blkCnt > 0
            ? `<div class="dashed-callout">${blkCnt} blocking CRITICAL/HIGH · ${openCnt} open total</div>`
            : (openCnt > 0 ? `<div class="dashed-callout amber">${openCnt} open observation${openCnt === 1 ? '' : 's'}</div>` : '')}
        </div>`;
      };

      document.getElementById('rev-board-grid').innerHTML = `
        <div>
          <div class="board-col-head"><span>IN-FLIGHT SESSIONS</span><span>${workers.length} ACTIVE</span></div>
          <div class="board-col-stack">${col1Cards}</div>
        </div>
        <div>
          <div class="board-col-head"><span>CHANGES NEEDED</span><span>${changesList.length} OPEN</span></div>
          <div class="board-col-stack">
            ${changesList.map(renderRevPrCard).join('') || `<div class="ticket-card standby-card"><div class="ticket-meta">Zero PRs currently blocked on review observations.</div></div>`}
          </div>
        </div>
        <div>
          <div class="board-col-head"><span>VERIFIED PASS</span><span>${passList.length} CLEAN</span></div>
          <div class="board-col-stack">
            ${passList.map(renderRevPrCard).join('') || `<div class="ticket-card standby-card"><div class="ticket-meta">No PASS verdicts recorded yet.</div></div>`}
          </div>
        </div>
      `;
    }

    function selectPr(key) {
      SELECTED_PR_KEY = key;
      DRAWER_MODE = 'reviewer';
      document.getElementById('inspector-drawer').classList.remove('hidden');
      renderReviewerBoard();
      renderReviewerDrawer();
    }

    function setObsFilter(f) {
      OBS_FILTER = f;
      renderReviewerDrawer();
    }

    function renderReviewerDrawer() {
      const prs = STATE?.pull_requests || [];
      const p = prs.find(x => x.key === SELECTED_PR_KEY);
      if (!p) return;

      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const obsAll = p.observations || [];
      const obsFiltered = obsAll.filter(o => {
        if (OBS_FILTER === 'open') return o.status === 'open';
        if (OBS_FILTER === 'blocking') return o.status === 'open' && (o.severity === 'CRITICAL' || o.severity === 'HIGH');
        if (OBS_FILTER === 'resolved') return o.status !== 'open';
        return true;
      });

      document.getElementById('inspector-header').innerHTML = `
        <div style="min-width:0;flex:1;">
          <div style="font-size:11px;font-weight:800;letter-spacing:0.06em;text-transform:uppercase;">${esc(p.repo)} // VERDICT: ${esc(p.verdict)}</div>
          <div style="font-family:var(--font-display);font-weight:900;font-size:22px;line-height:1.1;margin-top:3px;text-transform:uppercase;">
            #${p.pr_number} — ${esc(p.title)}
          </div>
          <div style="font-size:11.5px;font-weight:700;margin-top:6px;">
            ${esc(p.head_ref)} &rarr; ${esc(p.base_ref)} · @${esc(p.author)} · SHA ${esc((p.last_reviewed_sha || '-').slice(0, 7))}
          </div>
        </div>
        <button class="secondary" onclick="closeInspectorDrawer()">✕ CLOSE · ESC</button>
      `;

      const obsHtml = obsFiltered.map(o => {
        const sevTone = (o.severity === 'CRITICAL' || o.severity === 'HIGH') ? 'coral' : (o.severity === 'MEDIUM' ? 'amber' : 'mint');
        const stTone = o.status === 'open' ? 'coral' : 'mint';
        const loc = o.line ? `${o.path}:${o.line}` : o.path;
        return `<div class="thread-card ${o.status === 'open' && (o.severity === 'CRITICAL' || o.severity === 'HIGH') ? 'needs-human' : ''}">
          <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
            <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center;">
              <span class="rect-tag ${sevTone}">${esc(o.severity)}</span>
              <span class="rect-tag ${stTone}">${esc(o.status.toUpperCase())}</span>
              <span class="rect-tag muted">${esc(o.brain_id)}</span>
              <strong>${esc(o.title)}</strong>
            </div>
            <span style="font-size:11px;color:var(--muted);">RUN #${o.opened_in_run} · ${esc(o.origin_account)}</span>
          </div>
          <div style="margin-top:8px;"><code>${esc(loc)}</code></div>
          <div style="margin-top:8px;line-height:1.55;">${esc(o.claim)}</div>
          <div style="margin-top:8px;color:#d4d4d4;"><strong>FIX GUIDANCE:</strong> ${esc(o.fix_guidance)}</div>
          ${o.resolution_note ? `<div style="margin-top:8px;color:var(--mint);"><strong>RESOLUTION:</strong> ${esc(o.resolution_note)}</div>` : ''}
          ${o.suggested_code ? `<pre>${esc(o.suggested_code)}</pre>` : ''}
          <div style="margin-top:10px;">
            <button class="secondary" style="padding:4px 9px;font-size:11px;" onclick="promoteObsToFp('${escJsAttr(o.brain_id)}', '${escJsAttr(p.repo)}', '${escJsAttr(o.title.replace(/'/g, ''))}')">+ FALSE-POSITIVE GUARDRAIL (${esc(o.brain_id)})</button>
          </div>
        </div>`;
      }).join('') || `<div style="color:var(--muted);padding:12px 0;">No observations match this filter.</div>`;

      const traceHtml = (p.traceability || []).length ? `
        <div class="sec-head" style="margin-top:22px;">
          <span class="sec-tag">TRACEABILITY</span>
          <span class="sec-sub">Ticket-to-code requirement verification</span>
        </div>
        <table>
          <thead><tr><th>ID</th><th>Requirement</th><th>Code Evidence</th><th>Test Evidence</th><th>Status</th></tr></thead>
          <tbody>
            ${p.traceability.map(c => `<tr><td><code>${esc(c.id)}</code></td><td>${esc(c.requirement)}</td><td><code>${esc(c.code_evidence)}</code></td><td><code>${esc(c.test_evidence)}</code></td><td><span class="rect-tag muted">${esc(c.status)}</span></td></tr>`).join('')}
          </tbody>
        </table>` : '';

      document.getElementById('inspector-body').innerHTML = `
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:16px;">
          <a class="btn secondary" href="${esc(ghUrl)}" target="_blank" rel="noopener">↗ GITHUB PR</a>
          ${p.gdrive_doc_url ? `<a class="btn secondary" href="${esc(p.gdrive_doc_url)}" target="_blank" rel="noopener">GOOGLE DRIVE SPEC</a>` : ''}
          <button class="secondary" onclick="openPrInTender('${escJsAttr(p.repo)}', ${p.pr_number})">OPEN IN CODE TENDER</button>
          <button onclick="triggerAsyncReview('${escJsAttr(p.repo)}', ${p.pr_number})">⚡ START ASYNC RE-REVIEW</button>
        </div>
        ${p.latest_summary ? `<div class="banner-box" style="margin-bottom:16px;"><strong>EXECUTIVE SUMMARY //</strong> ${esc(p.latest_summary)}</div>` : ''}
        <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:14px;">
          <button class="${OBS_FILTER === 'all' ? '' : 'secondary'}" onclick="setObsFilter('all')">ALL (${obsAll.length})</button>
          <button class="${OBS_FILTER === 'open' ? '' : 'secondary'}" onclick="setObsFilter('open')">OPEN (${obsAll.filter(o => o.status === 'open').length})</button>
          <button class="${OBS_FILTER === 'blocking' ? '' : 'secondary'}" onclick="setObsFilter('blocking')">CRITICAL / HIGH</button>
          <button class="${OBS_FILTER === 'resolved' ? '' : 'secondary'}" onclick="setObsFilter('resolved')">RESOLVED (${obsAll.filter(o => o.status !== 'open').length})</button>
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
          <td><span class="rect-tag ${r.enabled ? 'mint' : 'muted'}">${r.enabled ? 'ENABLED' : 'PAUSED'}</span></td>
          <td><span class="rect-tag ${r.tender_enabled ? 'yellow' : 'muted'}">${r.tender_enabled ? 'TENDER ON' : 'OFF'}</span></td>
          <td style="font-size:11.5px;">
            <div><code>${esc(r.local_repo_path || '(auto-detect)')}</code></div>
            <div style="color:var(--muted);margin-top:2px;"><code>${esc(r.local_check_cmd || '(auto-infer)')}</code></div>
          </td>
          <td>
            <div style="display:flex;gap:6px;flex-wrap:wrap;">
              <button class="secondary" style="padding:4px 8px;font-size:11px;" onclick="loadRepoIntoForm('r', '${escJsAttr(r.full_name)}')">EDIT</button>
              <button class="secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleRepo('${escJsAttr(r.full_name)}')">${r.enabled ? 'PAUSE REV' : 'ENABLE REV'}</button>
              <button class="secondary" style="padding:4px 8px;font-size:11px;" onclick="toggleRepoTender('${escJsAttr(r.full_name)}')">${r.tender_enabled ? 'PAUSE TEND' : 'ENABLE TEND'}</button>
              <button class="danger" style="padding:4px 8px;font-size:11px;" onclick="removeRepo('${escJsAttr(r.full_name)}')">✕</button>
            </div>
          </td>
        </tr>
      `).join('');
    }

    function renderBrainsSidebar() {
      const brains = STATE.brains || [];
      document.getElementById('brains-list-box').innerHTML = brains.map(b => `
        <div class="list-item ${b === SELECTED_BRAIN ? 'selected' : ''}" onclick="loadBrainDetail('${escJsAttr(b)}')">
          <strong>${esc(b)}</strong>
        </div>
      `).join('');
    }

    async function loadBrainDetail(brainId) {
      SELECTED_BRAIN = brainId;
      renderBrainsSidebar();
      document.getElementById('brain-detail-title').textContent = `BRAIN // ${brainId}`;
      const d = await api('/api/brain?id=' + encodeURIComponent(brainId));
      document.getElementById('brain-files-box').innerHTML = `
        <h3 style="margin-top:8px;">ACTIVE LESSONS (<code>kb/lessons.md</code>)</h3><pre>${esc(d.lessons)}</pre>
        <h3 style="margin-top:16px;">FALSE-POSITIVE GUARDRAILS (<code>kb/false-positives.md</code>)</h3><pre>${esc(d.false_positives)}</pre>
        <h3 style="margin-top:16px;">REPO EXEMPLARS (<code>kb/exemplars.md</code>)</h3><pre>${esc(d.exemplars)}</pre>
        <h3 style="margin-top:16px;">SYSTEM PROMPT (<code>PROMPT.md</code>)</h3><pre>${esc(d.prompt)}</pre>
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
      closeInspectorDrawer();
      switchReviewerTab('board');
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

    // =========================================================
    // REPO & M1 FOLDER BROWSER HELPERS
    // =========================================================

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
      if (info.path) document.getElementById(prefix + '-local-path').value = info.path;
      if (info.full_name) document.getElementById(prefix + '-name').value = info.full_name;
      if (info.default_branch) document.getElementById(prefix + '-branch').value = info.default_branch;
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
        if (res.local_repo_path) document.getElementById(prefix + '-local-path').value = res.local_repo_path;
        if (res.default_branch) document.getElementById(prefix + '-branch').value = res.default_branch;
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
          ? `<span class="rect-tag mint">GIT REPO</span> <code>${esc(cur.full_name || cur.origin_url || 'no origin')}</code> · branch <code>${esc(cur.default_branch || 'main')}</code>`
          : `Browsing <code>${esc(d.current_path || '')}</code>`;

        document.getElementById('m1-discovered-list').innerHTML = (d.discovered_repos || []).map(r => `
          <div class="list-item" style="margin:0;" onclick="pickM1Folder('${escJsAttr(r.path)}')">
            <div style="display:flex;justify-content:space-between;gap:6px;align-items:center;">
              <strong>${esc(r.full_name || r.name)}</strong>
              <span class="rect-tag mint">${esc(r.default_branch || 'main')}</span>
            </div>
            <div style="font-size:11px;color:var(--muted);margin-top:3px;"><code>${esc(r.path)}</code></div>
          </div>
        `).join('') || '<div style="color:var(--muted);font-size:12px;">No git clones auto-discovered in standard roots; browse on the right.</div>';

        document.getElementById('m1-dir-list').innerHTML = (d.directories || []).map(dir => `
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;padding:7px 9px;border:1.5px solid #3a3a3a;background:#121212;">
            <div style="cursor:pointer;flex:1;min-width:0;" onclick="navigateM1Browser('${escJsAttr(dir.path)}')">
              <div style="font-size:12.5px;font-weight:700;">
                ${dir.is_git_repo ? '[GIT]' : '[DIR]'} ${esc(dir.name)}
                ${dir.full_name ? `<span class="rect-tag mint" style="margin-left:4px;">${esc(dir.full_name)}</span>` : ''}
              </div>
            </div>
            <button type="button" class="secondary" style="padding:4px 8px;font-size:11px;" onclick="pickM1Folder('${escJsAttr(dir.path)}')">SELECT</button>
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
    // CODE TENDER — BRUTALIST STATUS, 3-COL BOARD & INSPECTOR
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
        return { col: 'action', tagTone: 'yellow', tagText: 'TENDING ⚡', calloutTone: 'amber', callout: 'Agents fixing & running local check gate now' };
      }
      if (!isOpen) {
        const fixedAny = (p.handled_items || []).some(h => h.action === 'fixed');
        const tag = p.status_badge === 'FIXED_PUSHED' || fixedAny ? 'FIXED · MERGED' : 'CLOSED';
        return { col: 'done', tagTone: 'muted', tagText: tag, callout: '' };
      }
      if (!isWatched) {
        return { col: 'done', tagTone: 'muted', tagText: 'PAUSED', callout: '' };
      }
      if (p.status_badge === 'CHECK_FAILED') {
        return { col: 'action', tagTone: 'coral', tagText: 'CHECK FAILED', calloutTone: 'coral', callout: 'Local check failed — auto-push blocked' };
      }
      if (p.status_badge === 'NEEDS_HUMAN') {
        return { col: 'action', tagTone: 'coral', tagText: 'NEEDS INPUT', calloutTone: 'coral', callout: 'Waiting for your reply or issue decision' };
      }
      if (p.status_badge === 'STAGED_PATCH' || p.staged_patch) {
        return { col: 'action', tagTone: 'amber', tagText: 'READY TO PUSH', calloutTone: 'amber', callout: 'Verified patch waiting approval to push' };
      }
      if (openFb > 0) {
        return { col: 'action', tagTone: 'yellow', tagText: `${openFb} NEW`, calloutTone: 'amber', callout: `${openFb} review comment${openFb === 1 ? '' : 's'} queued to tend` };
      }
      if (p.status_badge === 'FIXED_PUSHED') {
        const sha = p.last_commit_pushed ? p.last_commit_pushed.slice(0, 7) : '';
        return { col: 'watching', tagTone: 'mint', tagText: 'FIXED & WATCHING', calloutTone: 'mint', callout: sha ? `Pushed ${sha} · listening for follow-up reviews` : 'Verified fix pushed · watching' };
      }
      if (p.status_badge === 'REPLIED_ONLY') {
        return { col: 'watching', tagTone: 'mint', tagText: 'REPLIED', calloutTone: '', callout: '' };
      }
      return { col: 'watching', tagTone: '', tagText: 'WATCHING', calloutTone: '', callout: '' };
    }

    function renderTenderSettingsAndLogs() {
      if (!STATE || !STATE.tender) return;
      const tc = STATE.tender;

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
          modeEl.textContent = 'MODE: MANUAL';
        } else {
          const deliv = tc.delivery_mode === 'stage_patch_only' ? 'STAGE ONLY' : 'AUTO-PUSH';
          modeEl.textContent = `AUTO-TEND · ${deliv}`;
        }
      }

      const logEl = document.getElementById('tender-logs-box');
      if (logEl) logEl.textContent = STATE.tender_logs || '(no Code Tender logs yet)';
      const histEl = document.getElementById('tender-history-tbody');
      if (histEl) {
        histEl.innerHTML = (STATE.tender_history || []).map(h => `
          <tr>
            <td><span class="rect-tag ${h.status === 'pass' ? 'mint' : 'coral'}">${esc(h.status)}</span></td>
            <td><code>GRAPHQL (0 AI)</code></td>
            <td>${h.duration_secs}s</td>
            <td style="color:var(--muted);">${new Date(h.started_epoch_secs * 1000).toISOString().replace('T', ' ').slice(0, 19)}</td>
          </tr>
        `).join('');
      }
    }

    function renderTenderBoard() {
      if (!STATE) return;
      const allPrs = STATE.tender_pull_requests || [];
      const activeWorkers = STATE.tender_active_workers || [];
      const autoRepos = (STATE.repos || []).filter(r => r.tender_enabled).map(r => r.full_name);
      const tc = STATE.tender || {};

      const colWatching = [];
      const colAction = [];
      const colDone = [];

      let openWatchedCount = 0;
      let commentsWaitingCount = 0;
      let checkBlockersCount = 0;
      let decisionsWaitingCount = 0;

      allPrs.forEach(p => {
        const meta = classifyTenderPr(p);
        const isOpenWatched = p.is_open !== false && p.watched !== false;
        if (isOpenWatched) {
          openWatchedCount++;
          commentsWaitingCount += Math.max(Number(p.open_feedback_count || 0), (p.pending_items || []).length);
          if (p.status_badge === 'CHECK_FAILED') checkBlockersCount++;
          if (p.status_badge === 'NEEDS_HUMAN' || p.status_badge === 'STAGED_PATCH' || p.staged_patch) {
            decisionsWaitingCount++;
          }
        }
        if (meta.col === 'watching') colWatching.push({ p, meta });
        else if (meta.col === 'action') colAction.push({ p, meta });
        else colDone.push({ p, meta });
      });

      // Masthead Hero
      document.getElementById('tender-hero-num').textContent = String(openWatchedCount);
      const repoLabelShort = autoRepos.length === 1
        ? shortRepo(autoRepos[0]).toUpperCase()
        : `${autoRepos.length} AUTO-WATCH REPOS`;
      document.getElementById('tender-hero-caption').textContent =
        activeWorkers.length > 0
          ? `${activeWorkers.length} WORKER TENDING NOW`
          : `OPEN PULLS ON ${repoLabelShort}`;

      const pwr = STATE.host?.ac_power ? 'AC POWER' : 'BATTERY';
      const cReady = STATE.runners?.claude_ready ? 'CLAUDE ✓' : 'CLAUDE !';
      const aReady = STATE.runners?.agy_ready ? 'AGY ✓' : 'AGY !';
      const delivMode = (!tc.enabled || tc.poll_only_mode)
        ? 'MANUAL'
        : (tc.delivery_mode === 'stage_patch_only' ? 'STAGE-ONLY' : 'AUTO-PUSH');
      const repoTicker = autoRepos.length ? autoRepos.join(', ').toUpperCase() : 'NONE';

      document.getElementById('tender-hero-ticker').innerHTML = `
        <span>${esc(formatUtcSnapshot())}</span>
        <span>M1 HOST: ${pwr}</span>
        <span>${cReady} · ${aReady}</span>
        <span>WATCHING: ${esc(repoTicker)}</span>
        <span>DELIVERY: ${delivMode}</span>
      `;

      // Narrative Status Callout Strip
      const lastPoll = (STATE.tender_history && STATE.tender_history[0]) ? STATE.tender_history[0].status : 'success';
      const watchedRepoHtml = autoRepos.length
        ? autoRepos.map(r => `<span class="inv-chip">${esc(r)}</span>`).join(' ')
        : `<span class="inv-chip">0 auto-watch repos</span>`;
      document.getElementById('tender-narrative').innerHTML =
        `<span class="inv-chip">${openWatchedCount}</span> open pull request${openWatchedCount === 1 ? '' : 's'} currently watched on ${watchedRepoHtml}. ` +
        `<span class="inv-chip ${commentsWaitingCount > 0 ? 'yellow' : ''}">${commentsWaitingCount}</span> review comment${commentsWaitingCount === 1 ? '' : 's'} waiting to tend, ` +
        `<span class="inv-chip ${checkBlockersCount > 0 ? 'coral' : ''}">${checkBlockersCount}</span> local check blocker${checkBlockersCount === 1 ? '' : 's'}. ` +
        `Last 2m GraphQL poller run: <span class="inv-chip mint">${esc(lastPoll)}</span>.`;

      // 5-Tile VITALS Grid
      const perRepoFoot = autoRepos.length
        ? autoRepos.map(r => `${esc(shortRepo(r))} ${allPrs.filter(p => p.repo === r && p.is_open !== false && p.watched !== false).length}`).join(' · ')
        : 'paste any PR URL above';
      document.getElementById('tender-vitals-grid').innerHTML = [
        renderVitalTile('amber', 'COMMENTS WAITING', commentsWaitingCount, true,
          commentsWaitingCount > 0 ? 'queued for next 2m tend cycle' : 'zero untended review comments'),
        renderVitalTile('coral', 'CHECK BLOCKERS', checkBlockersCount, true,
          checkBlockersCount > 0 ? 'local check failed before push' : 'zero failed local check gates'),
        renderVitalTile('amber', 'DECISIONS WAITING', decisionsWaitingCount, true,
          decisionsWaitingCount > 0 ? 'needs reply/issue or patch sign-off' : 'no threads awaiting human input'),
        renderVitalTile('dark', 'OPEN PULL REQUESTS', openWatchedCount, false, perRepoFoot),
        renderVitalTile('dark', 'TENDED & CLOSED', colDone.length, false,
          `${allPrs.reduce((acc, p) => acc + (p.handled_items || []).length, 0)} historical threads tended`)
      ].join('');

      const renderTenderCard = ({ p, meta }) => {
        const selectedCls = (DRAWER_MODE === 'tender' && SELECTED_TENDER_PR_KEY === p.key) ? 'selected-card' : '';
        const handledCnt = (p.handled_items || []).length;
        const worker = (STATE.tender_active_workers || []).find(
          w => String(w.repo || '').toLowerCase() === String(p.repo || '').toLowerCase() && w.pr_number === p.pr_number
        );
        const workerHtml = worker ? `
          <div class="brain-chips">
            ${Object.entries(worker.workers || {}).map(([name, ws]) => `
              <div class="brain-chip ${esc(ws.status)}">
                <span>${esc(name)}</span>
                <span>${esc((ws.runner || '').toUpperCase())} · ${esc((ws.status || '').toUpperCase())}</span>
              </div>
            `).join('')}
          </div>` : '';

        return `<div class="ticket-card ${selectedCls}" onclick="selectTenderPr('${escJsAttr(p.key)}')">
          <div class="ticket-top">
            <div>
              <span class="ticket-id">#${p.pr_number}</span>
              <span class="ticket-repo">${esc(shortRepo(p.repo))}</span>
            </div>
            <span class="rect-tag ${meta.tagTone}">${esc(meta.tagText)}</span>
          </div>
          <div class="ticket-title">${esc(p.title)}</div>
          <div class="ticket-meta">
            Who: @${esc(p.author || '-')} · Threads tended: ${handledCnt}<br/>
            ${p.head_ref ? `Branch: ${esc(p.head_ref)} &rarr; ${esc(p.base_ref)}` : `SHA: ${esc(((p.head_sha || p.last_tended_sha || '-') + '').slice(0, 7))}`}
          </div>
          ${meta.callout ? `<div class="dashed-callout ${meta.calloutTone || ''}">${esc(meta.callout)}</div>` : ''}
          ${workerHtml}
        </div>`;
      };

      const emptyWatchingHtml = autoRepos.length ? `
        <div class="ticket-card standby-card">
          <div class="ticket-top">
            <span class="ticket-id" style="font-size:14px;">AUTO-WATCH STANDBY</span>
            <span class="rect-tag mint">2M POLL</span>
          </div>
          <div class="ticket-title">0 open pull requests on ${esc(autoRepos.join(', '))}</div>
          <div class="ticket-meta">
            Polling GitHub every 2 minutes on M1 (0 AI tokens while idle). As soon as a new PR opens and receives review comments, Code Tender watches it and fixes threads automatically.
          </div>
        </div>` : `
        <div class="ticket-card standby-card">
          <div class="ticket-meta">Paste a GitHub PR URL above or enable Auto-Watch on a repository in 02 // REPOS, RULES &amp; LOGS.</div>
        </div>`;

      document.getElementById('tender-board-grid').innerHTML = `
        <div>
          <div class="board-col-head"><span>WATCHING OPEN</span><span>${colWatching.length} OPEN</span></div>
          <div class="board-col-stack">
            ${colWatching.map(renderTenderCard).join('') || emptyWatchingHtml}
          </div>
        </div>
        <div>
          <div class="board-col-head"><span>ACTION &amp; IN-FLIGHT</span><span>${colAction.length} ACTIVE</span></div>
          <div class="board-col-stack">
            ${colAction.map(renderTenderCard).join('') || `<div class="ticket-card standby-card"><div class="ticket-meta">Zero check blockers, staged patches, or pending review threads right now.</div></div>`}
          </div>
        </div>
        <div>
          <div class="board-col-head"><span>TENDED &amp; MERGED</span><span>${colDone.length} DONE</span></div>
          <div class="board-col-stack">
            ${colDone.map(renderTenderCard).join('') || `<div class="ticket-card standby-card"><div class="ticket-meta">No closed or historical pulls yet.</div></div>`}
          </div>
        </div>
      `;
    }

    function selectTenderPr(key) {
      SELECTED_TENDER_PR_KEY = key;
      DRAWER_MODE = 'tender';
      document.getElementById('inspector-drawer').classList.remove('hidden');
      renderTenderBoard();
      renderTenderDrawer();
    }

    function setTenderItemFilter(f) {
      TENDER_ITEM_FILTER = f;
      renderTenderDrawer();
    }

    function renderTenderDrawer() {
      const allPrs = STATE?.tender_pull_requests || [];
      const p = allPrs.find(x => x.key === SELECTED_TENDER_PR_KEY);
      if (!p) return;

      const bodyEl = document.getElementById('inspector-body');
      if (
        document.activeElement &&
        document.activeElement.classList &&
        document.activeElement.classList.contains('tender-inline-reply') &&
        bodyEl.dataset.prKey === SELECTED_TENDER_PR_KEY
      ) {
        return;
      }
      bodyEl.dataset.prKey = SELECTED_TENDER_PR_KEY;

      const meta = classifyTenderPr(p);
      const ghUrl = `https://github.com/${p.repo}/pull/${p.pr_number}`;
      const isWatched = p.watched !== false;
      const isOpen = p.is_open !== false;
      const running = isPrActivelyTending(p.repo, p.pr_number);
      const pending = p.pending_items || [];
      const handledAll = p.handled_items || [];
      const openFb = Math.max(Number(p.open_feedback_count || 0), pending.length);

      document.getElementById('inspector-header').innerHTML = `
        <div style="min-width:0;flex:1;">
          <div style="font-size:11px;font-weight:800;letter-spacing:0.06em;text-transform:uppercase;">
            ${esc(p.repo)} // ${esc(meta.tagText)}
          </div>
          <div style="font-family:var(--font-display);font-weight:900;font-size:22px;line-height:1.1;margin-top:3px;text-transform:uppercase;">
            #${p.pr_number} — ${esc(p.title)}
          </div>
          <div style="font-size:11.5px;font-weight:700;margin-top:6px;">
            ${p.head_ref ? `${esc(p.head_ref)} &rarr; ${esc(p.base_ref)} · ` : ''}
            ${p.author ? `@${esc(p.author)} · ` : ''}
            HEAD ${esc(((p.head_sha || p.last_tended_sha || '-') + '').slice(0, 7))}
          </div>
        </div>
        <button class="secondary" onclick="closeInspectorDrawer()">✕ CLOSE · ESC</button>
      `;

      const step1Cls = !isOpen ? '' : (isWatched ? 'done' : 'warn');
      const step1Sub = !isOpen ? 'PR closed / merged' : (isWatched ? 'Polling every 2m (0 AI)' : 'Paused by user');

      let step2Cls = '';
      let step2Sub = 'Listening for review comments';
      if (running || openFb > 0) {
        step2Cls = 'active';
        step2Sub = running ? 'Triaging & fixing comments' : `${openFb} new comment${openFb === 1 ? '' : 's'} detected`;
      } else if (handledAll.length > 0) {
        step2Cls = 'done';
        step2Sub = `${handledAll.length} thread${handledAll.length === 1 ? '' : 's'} processed`;
      }

      let step3Cls = '';
      let step3Sub = 'Standby until comments land';
      if (running) {
        step3Cls = 'active';
        step3Sub = 'Running local check gate...';
      } else if (p.status_badge === 'CHECK_FAILED') {
        step3Cls = 'warn';
        step3Sub = 'Local check failed — push blocked';
      } else if (p.status_badge === 'NEEDS_HUMAN') {
        step3Cls = 'warn';
        step3Sub = 'Waiting for your reply / issue';
      } else if (p.status_badge === 'STAGED_PATCH' || p.staged_patch) {
        step3Cls = 'active';
        step3Sub = 'Verified patch ready to push';
      } else if (p.status_badge === 'FIXED_PUSHED') {
        step3Cls = 'done';
        step3Sub = p.last_commit_pushed ? `Pushed ${p.last_commit_pushed.slice(0, 7)}` : 'Verified fix pushed';
      } else if (p.status_badge === 'REPLIED_ONLY') {
        step3Cls = 'done';
        step3Sub = 'Replied & resolved on GitHub';
      }

      const diffAccKey = `diff::${p.key}`;
      const diffOpenAttr = OPEN_ACCORDIONS.has(diffAccKey) ? 'open' : '';

      let statusBannerHtml = '';
      if (!isOpen) {
        statusBannerHtml = `
          <div class="banner-box">
            <strong>PULL REQUEST MERGED / CLOSED</strong>
            <div style="font-size:12px;color:var(--muted);margin-top:4px;">Closed on GitHub. Historical tended threads and replies are shown below.</div>
          </div>`;
      } else if (running) {
        statusBannerHtml = `
          <div class="banner-box amber">
            <strong>⚡ ACTIVELY TENDING THIS PULL REQUEST</strong>
            <div style="font-size:12px;color:var(--muted);margin-top:4px;">Applying fixes in local M1 checkout context and verifying checks before pushing.</div>
          </div>`;
      } else if (p.status_badge === 'CHECK_FAILED') {
        statusBannerHtml = `
          <div class="banner-box coral">
            <div style="display:flex;justify-content:space-between;align-items:flex-start;gap:12px;flex-wrap:wrap;">
              <div style="flex:1;min-width:220px;">
                <strong style="color:var(--coral);">LOCAL VERIFICATION CHECK FAILED — AUTO-PUSH BLOCKED</strong>
                <div style="font-size:12px;margin-top:4px;">${esc(p.last_summary || 'Local check command exited non-zero.')}</div>
                ${p.last_check_cmd_used ? `<div style="font-size:11.5px;color:var(--muted);margin-top:4px;">CHECK CMD: <code>${esc(p.last_check_cmd_used)}</code></div>` : ''}
              </div>
              <div style="display:flex;gap:8px;flex-wrap:wrap;">
                <button onclick="triggerPrTend('${escJsAttr(p.repo)}', ${p.pr_number}, false)">↻ RETRY TEND</button>
                ${p.staged_patch ? `<button class="secondary" onclick="approveAndPushPatch('${escJsAttr(p.repo)}', ${p.pr_number})">PUSH DRAFT ANYWAY</button>` : ''}
                ${p.staged_patch ? `<button class="danger" onclick="discardStagedPatch('${escJsAttr(p.repo)}', ${p.pr_number})">DISCARD</button>` : ''}
              </div>
            </div>
            ${p.staged_patch ? `<details class="brutal-acc" ${diffOpenAttr} ontoggle="if(this.open) OPEN_ACCORDIONS.add('${escJsAttr(diffAccKey)}'); else OPEN_ACCORDIONS.delete('${escJsAttr(diffAccKey)}');" style="box-shadow:none;margin-top:12px;"><summary style="padding:8px 12px;font-size:11.5px;">INSPECT DRAFTED DIFF (${esc(p.staged_commit_message || 'fix: address review feedback')})</summary><div class="acc-body" style="padding:10px;"><pre style="margin:0;">${esc(p.staged_patch)}</pre></div></details>` : ''}
          </div>`;
      } else if (p.staged_patch) {
        statusBannerHtml = `
          <div class="banner-box amber">
            <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
              <div>
                <strong style="color:var(--yellow);">VERIFIED FIX READY TO PUSH</strong>
                <div style="font-size:12px;color:var(--muted);margin-top:3px;">COMMIT: <code>${esc(p.staged_commit_message || 'fix: address code review feedback')}</code></div>
              </div>
              <div style="display:flex;gap:8px;flex-wrap:wrap;">
                <button class="mint-btn" onclick="approveAndPushPatch('${escJsAttr(p.repo)}', ${p.pr_number})">✓ APPROVE &amp; PUSH</button>
                <button class="danger" onclick="discardStagedPatch('${escJsAttr(p.repo)}', ${p.pr_number})">DISCARD</button>
              </div>
            </div>
            <details class="brutal-acc" ${diffOpenAttr} ontoggle="if(this.open) OPEN_ACCORDIONS.add('${escJsAttr(diffAccKey)}'); else OPEN_ACCORDIONS.delete('${escJsAttr(diffAccKey)}');" style="box-shadow:none;margin-top:12px;"><summary style="padding:8px 12px;font-size:11.5px;">VIEW VERIFIED PATCH DIFF</summary><div class="acc-body" style="padding:10px;"><pre style="margin:0;">${esc(p.staged_patch)}</pre></div></details>
          </div>`;
      } else if (openFb > 0) {
        statusBannerHtml = `
          <div class="banner-box amber">
            <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;flex-wrap:wrap;">
              <div>
                <strong>${openFb} NEW REVIEW COMMENT${openFb === 1 ? '' : 'S'} READY TO TEND</strong>
                <div style="font-size:12px;color:var(--muted);margin-top:3px;">Will auto-tend on the next 2m tick, or click TEND NOW to run immediately.</div>
              </div>
              <button onclick="triggerPrTend('${escJsAttr(p.repo)}', ${p.pr_number}, false)">⚡ TEND NOW</button>
            </div>
          </div>`;
      } else if (p.status_badge === 'NEEDS_HUMAN') {
        statusBannerHtml = `
          <div class="banner-box coral">
            <strong style="color:var(--coral);">THREAD NEEDS YOUR DECISION</strong>
            <div style="font-size:12px;margin-top:4px;">${esc(p.last_summary || 'Reply or create a follow-up issue directly inside the thread card below — Code Tender will post it on GitHub, resolve the thread, and clear Needs Input.')}</div>
          </div>`;
      } else if (p.status_badge === 'FIXED_PUSHED') {
        statusBannerHtml = `
          <div class="banner-box mint">
            <strong style="color:var(--mint);">✓ VERIFIED FIX PUSHED${p.last_commit_pushed ? ` (${esc(p.last_commit_pushed.slice(0, 7))})` : ''}</strong>
            <div style="font-size:12px;margin-top:4px;">${esc(p.last_summary || 'Review comments fixed, verified locally, and pushed.')}</div>
          </div>`;
      }

      const pendingHtml = pending.length ? `
        <div class="sec-head" style="margin-top:18px;">
          <span class="sec-tag">PENDING</span>
          <span class="sec-sub">${pending.length} incoming review comment${pending.length === 1 ? '' : 's'}</span>
        </div>
        ${pending.map(item => {
          const loc = item.path ? (item.line ? `${item.path}:${item.line}` : item.path) : item.kind;
          const cleaned = cleanReviewBody(item.body);
          return `<div class="thread-card">
            <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
              <div><span class="rect-tag yellow">PENDING</span> <strong>@${esc(item.author)}</strong> · <code>${esc(loc)}</code></div>
              <span style="font-size:11px;color:var(--muted);">${esc((item.created_at || '').replace('T', ' ').slice(0, 16))}</span>
            </div>
            <div style="margin-top:8px;white-space:pre-wrap;">${esc(cleaned)}</div>
          </div>`;
        }).join('')}` : '';

      const handledFiltered = handledAll.filter(h => {
        if (TENDER_ITEM_FILTER === 'fixed') return h.action === 'fixed' || h.action === 'already_addressed';
        if (TENDER_ITEM_FILTER === 'explained') return h.action === 'explained_rationale' || h.action === 'created_issue';
        if (TENDER_ITEM_FILTER === 'human') return h.action === 'needs_human';
        return true;
      });

      const handledSectionHtml = handledAll.length ? `
        <div style="display:flex;justify-content:space-between;align-items:center;gap:10px;flex-wrap:wrap;margin:20px 0 12px;">
          <div class="sec-head" style="margin:0;">
            <span class="sec-tag">THREADS</span>
            <span class="sec-sub">${handledAll.length} tended comment${handledAll.length === 1 ? '' : 's'} &amp; replies</span>
          </div>
          ${handledAll.length > 1 ? `
            <div style="display:flex;gap:6px;flex-wrap:wrap;">
              <button class="${TENDER_ITEM_FILTER === 'all' ? '' : 'secondary'}" style="padding:4px 9px;font-size:11px;" onclick="setTenderItemFilter('all')">ALL (${handledAll.length})</button>
              <button class="${TENDER_ITEM_FILTER === 'fixed' ? '' : 'secondary'}" style="padding:4px 9px;font-size:11px;" onclick="setTenderItemFilter('fixed')">FIXED (${handledAll.filter(h => h.action === 'fixed' || h.action === 'already_addressed').length})</button>
              <button class="${TENDER_ITEM_FILTER === 'explained' ? '' : 'secondary'}" style="padding:4px 9px;font-size:11px;" onclick="setTenderItemFilter('explained')">REPLIED / ISSUE (${handledAll.filter(h => h.action === 'explained_rationale' || h.action === 'created_issue').length})</button>
              <button class="${TENDER_ITEM_FILTER === 'human' ? '' : 'secondary'}" style="padding:4px 9px;font-size:11px;" onclick="setTenderItemFilter('human')">NEEDS INPUT (${handledAll.filter(h => h.action === 'needs_human').length})</button>
            </div>` : ''}
        </div>
        ${handledFiltered.map((h, idx) => {
          const isFixed = h.action === 'fixed' || h.action === 'already_addressed';
          const isIssue = h.action === 'created_issue';
          const isExplained = h.action === 'explained_rationale' || isIssue;
          const tagTone = isFixed ? 'mint' : (isExplained ? 'yellow' : 'coral');
          const aLabel = h.action === 'fixed'
            ? 'FIXED'
            : (h.action === 'already_addressed'
              ? 'ADDRESSED'
              : (isIssue
                ? (h.issue_number ? `ISSUE #${h.issue_number}` : 'ISSUE CREATED')
                : (h.action === 'explained_rationale' ? 'REPLIED' : 'NEEDS INPUT')));
          const loc = h.path ? (h.line ? `${h.path}:${h.line}` : h.path) : 'PR thread';
          const draftKey = `${p.key}::${h.feedback_id}`;
          const draftVal = TENDER_REPLY_DRAFTS[draftKey] ?? '';
          const inputId = `tender-reply-${idx}`;
          const issuePill = (h.issue_number && h.issue_url)
            ? `<a href="${esc(h.issue_url)}" target="_blank" rel="noopener" class="rect-tag yellow" style="text-decoration:none;">↗ #${h.issue_number}</a>`
            : '';
          const respondBar = h.action === 'needs_human' ? `
            <div style="margin-top:12px;padding-top:10px;border-top:1.5px dashed var(--coral);display:flex;gap:8px;flex-wrap:wrap;align-items:center;">
              <input id="${inputId}" class="tender-inline-reply" data-draft-key="${esc(draftKey)}" value="${esc(draftVal)}"
                oninput="TENDER_REPLY_DRAFTS[this.dataset.draftKey] = this.value"
                onkeydown="if(event.key==='Enter' && !event.shiftKey){event.preventDefault();respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', false);}"
                placeholder="Write reply addressing concern (or note for follow-up issue)..."
                style="flex:1;min-width:220px;" />
              <button onclick="respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', false)">REPLY &amp; RESOLVE</button>
              <button class="secondary" onclick="respondTenderItem('${escJsAttr(p.repo)}', ${p.pr_number}, '${escJsAttr(h.feedback_id)}', '${inputId}', true)">CREATE ISSUE &amp; RESOLVE</button>
            </div>` : '';

          const cs = cleanReviewBody(h.summary);
          const cr = cleanReviewBody(h.reply_text);
          return `<div class="thread-card ${h.action === 'needs_human' ? 'needs-human' : ''}">
            <div style="display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;align-items:center;">
              <div style="display:flex;align-items:center;gap:7px;flex-wrap:wrap;">
                <span class="rect-tag ${tagTone}">${esc(aLabel)}</span>
                ${issuePill}
                <strong>@${esc(h.source_author)}</strong>
                <code>${esc(loc)}</code>
                ${h.resolved_thread ? '<span class="rect-tag mint">✓ RESOLVED</span>' : ''}
              </div>
              ${h.commit_sha ? `<code>${esc(h.commit_sha.slice(0, 7))}</code>` : ''}
            </div>
            ${cs ? `<div style="margin-top:8px;line-height:1.5;">${esc(cs)}</div>` : ''}
            ${cr ? `<div style="margin-top:8px;color:var(--mint);line-height:1.5;">&rarr; ${esc(cr)}</div>` : ''}
            ${respondBar}
          </div>`;
        }).join('')}` : `<div style="color:var(--muted);padding:10px 0;">Zero review threads processed on this pull request so far.</div>`;

      const learningList = p.learned_rules || [];
      const metaAccKey = `meta::${p.key}`;
      const metaOpenAttr = OPEN_ACCORDIONS.has(metaAccKey) ? 'open' : '';
      const metaDetailsHtml = (learningList.length || p.local_context_used || p.last_check_cmd_used) ? `
        <details class="brutal-acc" ${metaOpenAttr} ontoggle="if(this.open) OPEN_ACCORDIONS.add('${escJsAttr(metaAccKey)}'); else OPEN_ACCORDIONS.delete('${escJsAttr(metaAccKey)}');" style="box-shadow:none;">
          <summary style="padding:10px 14px;font-size:11.5px;">
            <span>LOCAL CONTEXT, CHECK GATE${learningList.length ? ` &amp; ${learningList.length} LEARNED RULE${learningList.length === 1 ? '' : 'S'}` : ''}</span>
          </summary>
          <div class="acc-body" style="font-size:12px;">
            <div style="color:var(--muted);line-height:1.7;">
              LOCAL CONTEXT: <code>${esc(p.local_context_used || 'auto-detected on next run')}</code><br/>
              CHECK GATE: <code>${esc(p.last_check_cmd_used || 'auto-inferred on next run')}</code> · CYCLES RUN: <code>${p.runs_count || 0}</code>
            </div>
            ${learningList.map(lr => `
              <div style="border-top:1px solid #333;margin-top:8px;padding-top:8px;">
                <span class="rect-tag mint">${esc(lr.scope)}</span> <code>${esc(lr.target)}</code> — ${esc(lr.rule)}
              </div>
            `).join('')}
          </div>
        </details>` : '';

      bodyEl.innerHTML = `
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:16px;">
          <a class="btn secondary" href="${esc(ghUrl)}" target="_blank" rel="noopener">↗ GITHUB PR</a>
          ${isWatched
            ? `<button class="secondary" onclick="setPrWatched('${escJsAttr(p.repo)}', ${p.pr_number}, false)">⏸ PAUSE WATCH</button>`
            : `<button onclick="setPrWatched('${escJsAttr(p.repo)}', ${p.pr_number}, true)">▶ RESUME WATCH</button>`
          }
          <button onclick="triggerPrTend('${escJsAttr(p.repo)}', ${p.pr_number}, false)">⚡ TEND NOW</button>
          <button class="danger" onclick="removeWatchedPr('${escJsAttr(p.repo)}', ${p.pr_number})">✕ REMOVE</button>
        </div>

        <div class="stepper">
          <div class="step-box ${step1Cls}">
            <div class="step-title">01 // WATCH PULL</div>
            <div class="step-sub">${esc(step1Sub)}</div>
          </div>
          <div class="step-box ${step2Cls}">
            <div class="step-title">02 // REVIEW COMMENTS</div>
            <div class="step-sub">${esc(step2Sub)}</div>
          </div>
          <div class="step-box ${step3Cls}">
            <div class="step-title">03 // TEND &amp; PUSH</div>
            <div class="step-sub">${esc(step3Sub)}</div>
          </div>
        </div>

        ${statusBannerHtml}
        ${pendingHtml}
        ${handledSectionHtml}
        ${metaDetailsHtml}
      `;
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
        showToast(res.message || `Watching ${res.key}`);
        await refreshAll();
        if (res.key) selectTenderPr(res.key);
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
          closeInspectorDrawer();
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
        listEl.innerHTML = `<div style="color:var(--coral);padding:8px 0;">Could not load open PRs: ${esc(e.message)}</div>`;
      }
    }

    function renderOpenPrPicker() {
      const listEl = document.getElementById('open-pr-picker-list');
      if (!listEl || OPEN_PR_PICKER_LOADING) return;
      if (!OPEN_PR_PICKER_ITEMS.length) {
        listEl.innerHTML = `<div style="color:var(--muted);padding:8px 0;">Zero open PRs found on your configured repositories right now. Paste any GitHub PR URL above or add a repository in 02 // REPOS, RULES &amp; LOGS.</div>`;
        return;
      }
      listEl.innerHTML = OPEN_PR_PICKER_ITEMS.map(item => {
        const watched = Boolean(item.watched);
        const fb = Number(item.open_feedback_count || 0);
        return `<div class="repo-row" style="margin-bottom:8px;">
          <div style="min-width:0;flex:1;">
            <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
              <strong>${esc(item.repo)} #${item.pr_number}</strong>
              ${item.is_draft ? '<span class="rect-tag muted">DRAFT</span>' : ''}
              ${fb > 0 ? `<span class="rect-tag yellow">${fb} COMMENTS</span>` : '<span class="rect-tag muted">0 NEW</span>'}
            </div>
            <div style="margin-top:4px;font-weight:700;">${esc(item.title)}</div>
            <div style="font-size:11.5px;color:var(--muted);margin-top:3px;">
              <code>${esc(item.head_ref)}</code> &rarr; <code>${esc(item.base_ref)}</code> · @${esc(item.author)}
            </div>
          </div>
          <div>
            ${watched
              ? `<button class="secondary" onclick="setPrWatched('${escJsAttr(item.repo)}', ${item.pr_number}, false)">✓ WATCHING</button>`
              : `<button onclick="setPrWatched('${escJsAttr(item.repo)}', ${item.pr_number}, true)">+ WATCH PR</button>`
            }
          </div>
        </div>`;
      }).join('');
    }

    function renderTenderReposAndAgents() {
      if (!STATE) return;
      const reposList = document.getElementById('tender-repos-list');
      if (reposList) {
        reposList.innerHTML = (STATE.repos || []).map(r => `
          <div class="repo-row">
            <div style="min-width:0;flex:1;">
              <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
                <strong>${esc(r.full_name)}</strong>
                <code>${esc(r.default_branch)}</code>
              </div>
              <div style="font-size:11.5px;color:var(--muted);margin-top:4px;">
                M1 PATH: <code>${esc(r.local_repo_path || 'auto-detect')}</code> · CHECK: <code>${esc(r.local_check_cmd || 'auto-infer')}</code>
              </div>
            </div>
            <div style="display:flex;gap:6px;flex-wrap:wrap;align-items:center;">
              <button class="${r.tender_enabled ? '' : 'secondary'}" style="padding:5px 10px;font-size:11px;" onclick="toggleRepoTender('${escJsAttr(r.full_name)}')">
                ${r.tender_enabled ? '✓ AUTO-WATCH ON' : 'AUTO-WATCH OFF'}
              </button>
              <button class="secondary" style="padding:5px 9px;font-size:11px;" onclick="toggleRepo('${escJsAttr(r.full_name)}')">
                ${r.enabled ? '✓ REVIEWER ON' : 'REVIEWER OFF'}
              </button>
              <button class="secondary" style="padding:5px 9px;font-size:11px;" onclick="loadRepoIntoForm('tr', '${escJsAttr(r.full_name)}')">EDIT</button>
              <button class="danger" style="padding:5px 9px;font-size:11px;" onclick="removeRepo('${escJsAttr(r.full_name)}')">✕</button>
            </div>
          </div>
        `).join('') || `<div style="color:var(--muted);">No repositories configured yet. Add one on the right or paste a PR URL on the board.</div>`;
      }

      const agentsBox = document.getElementById('tender-agents-list-box');
      if (agentsBox) {
        const agents = STATE.tender_agents || ['01-tender-triage', '02-tender-fixer', '03-tender-verifier'];
        agentsBox.innerHTML = agents.map(a => `
          <div class="list-item ${a === SELECTED_TENDER_AGENT ? 'selected' : ''}" onclick="loadTenderAgentDetail('${escJsAttr(a)}')">
            <strong>${esc(a)}</strong>
          </div>
        `).join('');
      }
    }

    async function loadTenderAgentDetail(agentId) {
      SELECTED_TENDER_AGENT = agentId;
      renderTenderReposAndAgents();
      document.getElementById('tender-agent-detail-title').textContent = `TENDER AGENT // ${agentId}`;
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
        showToast('Write a reply first, or click CREATE ISSUE & RESOLVE');
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
    // PIXEL AGENTS VISUAL OFFICE & BRUTALIST FALLBACK CANVAS
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
          badge.className = 'rect-tag mint';
          navBadge.textContent = 'LIVE';
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
          badge.textContent = `REMOTE :${port}`;
          badge.className = 'rect-tag yellow';
          navBadge.textContent = '7421';
          document.getElementById('pixel-fallback-title').textContent = 'PIXEL AGENTS LIVE ON M1 — FORWARD PORT 7421 FOR FULL OFFICE';
          document.getElementById('pixel-fallback-desc').innerHTML =
            `<code>pixel-agents v1.6.0</code> is live on M1 at <code>127.0.0.1:${port}</code>. Forward <code>-L 127.0.0.1:${port}:127.0.0.1:${port}</code> on SSH and click RETRY.`;
        } else {
          badge.textContent = 'OFFLINE';
          badge.className = 'rect-tag muted';
          navBadge.textContent = 'OFF';
          document.getElementById('pixel-fallback-title').textContent = 'PIXEL AGENTS STANDALONE SERVER NOT RUNNING';
          document.getElementById('pixel-fallback-desc').textContent =
            'Click START PIXEL SERVER (:7421) to launch the live pixel-agents office on M1.';
        }
      } catch (e) {
        PIXEL_ONLINE = false;
        badge.textContent = 'OFFLINE';
        badge.className = 'rect-tag muted';
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
        el.textContent = `${rev} REVIEWER · ${tend} TENDER ACTIVE`;
      }
      if (!PIXEL_ONLINE) {
        const navBadge = document.getElementById('nav-pixel-badge');
        if (navBadge && navBadge.textContent === '...') {
          navBadge.textContent = 'V1.6';
        }
      }
    }

    let PIXEL_TICK = 0;
    function drawFallbackPixelOffice() {
      const canvas = document.getElementById('pixelFallbackCanvas');
      if (!canvas) return;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      PIXEL_TICK++;

      const W = canvas.width, H = canvas.height;
      ctx.fillStyle = '#0a0a0a';
      ctx.fillRect(0, 0, W, H);

      for (let x = 0; x < W; x += 24) {
        for (let y = 52; y < H; y += 24) {
          ctx.fillStyle = ((x / 24 + y / 24) % 2 === 0) ? '#141414' : '#0e0e0e';
          ctx.fillRect(x, y, 24, 24);
        }
      }

      ctx.fillStyle = '#F7D842';
      ctx.fillRect(0, 0, W, 50);
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 50, W, 2);
      ctx.fillStyle = '#0a0a0a';
      ctx.font = 'bold 13px monospace';
      ctx.fillText('M1 INTERNAL CI FLOORPLAN // CLAUDE ⇄ AGY CLUSTER', 16, 30);

      const stations = [];
      (STATE?.active_workers || []).forEach(w => {
        stations.push({ label: `CR #${w.pr_number}`, runner: w.lead_runner, active: true });
      });
      (STATE?.tender_active_workers || []).forEach(w => {
        stations.push({ label: `CT #${w.pr_number}`, runner: w.lead_runner, active: true });
      });
      while (stations.length < 6) {
        const idx = stations.length + 1;
        stations.push({
          label: idx <= 3 ? `BRAIN-0${idx}` : `TENDER-0${idx - 3}`,
          runner: idx % 2 === 0 ? 'agy' : 'claude',
          active: false
        });
      }

      stations.slice(0, 6).forEach((st, i) => {
        const col = i % 3, row = Math.floor(i / 3);
        const dx = 65 + col * 235;
        const dy = 96 + row * 112;

        ctx.fillStyle = '#ffffff';
        ctx.fillRect(dx + 3, dy + 3, 148, 28);
        ctx.fillStyle = '#262626';
        ctx.fillRect(dx, dy, 148, 28);

        const screenColor = st.active
          ? (PIXEL_TICK % 6 < 3 ? '#F7D842' : '#4ADE80')
          : '#1f1f1f';
        ctx.fillStyle = '#ffffff';
        ctx.fillRect(dx + 51, dy - 23, 46, 25);
        ctx.fillStyle = '#080808';
        ctx.fillRect(dx + 52, dy - 22, 44, 24);
        ctx.fillStyle = screenColor;
        ctx.fillRect(dx + 56, dy - 18, 36, 16);

        const bob = st.active ? (PIXEL_TICK % 4 < 2 ? -2 : 0) : 0;
        const shirt = st.runner === 'claude' ? '#DE9E1E' : '#F07A74';
        ctx.fillStyle = '#fde68a';
        ctx.fillRect(dx + 64, dy + 26 + bob, 20, 16);
        ctx.fillStyle = '#080808';
        ctx.fillRect(dx + 64, dy + 24 + bob, 20, 6);
        ctx.fillStyle = shirt;
        ctx.fillRect(dx + 60, dy + 42 + bob, 28, 18);

        ctx.fillStyle = st.active ? '#F7D842' : '#8e8e8e';
        ctx.font = 'bold 11px monospace';
        ctx.fillText(`${st.label} [${st.runner.toUpperCase()}]`, dx + 6, dy + 76);
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
