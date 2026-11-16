---
paths:
  - "docs/**"
  - ".github/assets/**"
  - "scripts/brand/**"
  - "README.md"
---

# Brand & GitHub Pages Site

The brand is the neo-brutalist look of the reviewer dashboard, made official: **a terminal with the lid shut, printed in signal yellow.**

## 1. The idea

- **o=o.** "otto" is a palindrome: it reads the same from both ends, the way the bridge makes Windows and macOS act as one machine. The mark is two hollow square o's (the machines) joined by a double bar (the bridge).
- **Otto** is the mascot: a pixel otter peeking over a yellow laptop, because he lives in the old MacBook. The pixel style comes from the pixel-agents office.
- Tagline: **Put the old MacBook to work.**

## 2. Tokens

Two themes. Night Shift is primary; Paper is for docs and print.

| Token | Night Shift | Paper | Use |
| --- | --- | --- | --- |
| `bg` | `#080808` | `#f4f1e8` | Page ground |
| `surface` | `#121212` | `#ebe7da` | Bars, nav rails, chips |
| `card` | `#161616` | `#ffffff` | Cards, tiles, inputs |
| `ink` | `#ffffff` | `#0a0a0a` | Text, every border, the hard shadow |
| `ink-dark` | `#0a0a0a` | `#0a0a0a` | Text on any coloured fill; never flips |
| `muted` | `#8e8e8e` | `#5c5a52` | Secondary text |
| `yellow` | `#F7D842` | same | Signal Yellow: one primary thing per view |
| `mint` / `amber` / `coral` / `cyan` | `#4ADE80` / `#DE9E1E` / `#F07A74` / `#38BDF8` | same | ok / warn / fail / info fills, always with `ink-dark` text and a word |
| `otter-fur` / `otter-muzzle` | `#9a6a3c` / `#f1dfc0` | same | Mascot only |

- **Type:** Archivo Black for titles, tags and big numbers, always UPPERCASE with tight tracking. JetBrains Mono for everything else, body copy included. No third face, no italics.
- **Shape:** radius 0 everywhere. Frames are a 2px `ink` border plus a hard shadow (`4px 4px 0`, no blur); 3px and `6px 6px 0` only for a masthead or hero. No gradients, no soft shadows.
- **Motion:** 60–70ms. Hover lifts 1–2px and grows the shadow, press sinks into it. The only loop is a status dot blinking in two hard steps. Honour `prefers-reduced-motion`.
- **Contrast:** yellow is a text colour on night grounds only; on Paper it is a fill. Mint and coral differ mostly by hue, so status always carries a word.

## 3. Voice

- Lead with the machine and the number: "caps the battery at **78%**", "0 AI tokens on idle ticks", "100% reversible".
- Commands are the copy. Introduce a feature with the command that runs it, in a terminal block.
- Say "you", not "we". The product is lowercase `cheap-otto` in prose and code.
- Honest about the hack: it's a MacBook with a broken screen doing real work.
- No decorative emoji. Allowed glyphs are functional: `✓`, `✗`, `■`, `→`, `⇄`, and the status emoji the reviewer posts on GitHub.
- **No invented flags or features.** Every command shown on the site or in the README must run as written against the current code.

## 4. Assets are generated, not drawn

| File | Made by |
| --- | --- |
| `docs/assets/otto-mark*.svg`, `otto-wordmark-*.svg`, `otto-pixel*.svg` | `scripts/brand/gen-logos.js` (pixel grids in code; rects only, so no font is needed) |
| `.github/assets/readme-hero-{dark,light}.png`, `docs/assets/social-preview.png` | `scripts/brand/hero.html` rendered by headless Chrome |

- Regenerate everything with `bash scripts/brand/render.sh`. Pass a temp dir (`render.sh <dir>`) to inspect output without touching tracked files. The SVGs from `gen-logos.js` are deterministic. The PNGs are visually reproducible only with the same browser version and fonts, so a re-render may differ in bytes and must be reviewed visually.
- **Never hand-edit a generated SVG or PNG.** Change the generator or the template, re-run, look at the result, commit both.
- Use the `-ink` wordmark and plain mark on light grounds, `-white` and `-on-dark` on dark grounds. Don't recolour the yellow slab, rotate the mark or smooth Otto: scale him in whole multiples with `image-rendering: pixelated`.
- The GitHub social preview can only be set by hand: Settings → General → Social preview → upload `docs/assets/social-preview.png`. Tell the user when that file changes.

## 5. The Pages site (`docs/`)

- GitHub Pages serves `main:/docs` at `https://cokitron.github.io/cheap-otto/`. A merge to `main` deploys; there is no build step and no workflow. `docs/.nojekyll` must stay.
- `docs/index.html` is one self-contained file: inline CSS and JS, tokens as CSS variables under `[data-theme="night"]` and `[data-theme="paper"]`. The only external request is Google Fonts. Keep it a single file with no framework and no bundler.
- **The blog** is `docs/blog/`: `index.html` lists the posts newest first, each post is `YYYY-MM-DD-<slug>.html`, and every page shares `blog.css` (the same tokens, cut down) and the `otto-theme` theme switch. A post is hand-written HTML with no build step, linked from the index and the `Blog` links in `docs/index.html`. It follows the voice above: every command runs as written, every number comes from the repo or GitHub (PRs, diffs, test counts), and sample output is a real dry run.
- **The demos mirror the code.** When the Rust side changes, update the site in the same PR:
  - the arbiter demo's `evaluate()` and reason strings ↔ `ArbiterPolicy::evaluate` in `src/arbiter.rs`;
  - the doctor demo's eight check names ↔ `src/doctor.rs`;
  - the `pmset` lines in the battery demo ↔ `src/macos.rs`;
  - the eight brain names ↔ `crates/code-reviewer/brains/`;
  - quickstart tabs and the hero terminal ↔ real CLI flags.
- The farm, doctor and reviewer panels are labelled simulations with invented sample data. Keep them labelled, and never present sample numbers as measurements.
- Check both themes, a 390px-wide viewport (no horizontal scroll, 16px gutters) and reduced motion before shipping. Text needs 4.5:1 contrast in both themes.
- The README opens with a `<picture>` that swaps the dark and light hero. Keep both images at the same aspect ratio.
