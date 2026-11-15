// Generates the cheap-otto logo set as font-independent SVG (rects only, crisp edges):
// the o=o mark, the pixel-block wordmark and Otto the pixel otter, each for light and dark grounds.
//
// usage: node scripts/brand/gen-logos.js [outDir]     (default: docs/assets)
//
// The output is deterministic. Edit the grids or colours below, re-run, and commit the SVGs.
const fs = require("fs");
const path = require("path");
const out = process.argv[2] || path.join(__dirname, "..", "..", "docs", "assets");
fs.mkdirSync(out, { recursive: true });

const INK = '#0a0a0a';
const WHITE = '#ffffff';
const YELLOW = '#F7D842';
const FUR = '#9a6a3c';
const MUZZLE = '#f1dfc0';

// ---------- block letters (2-unit strokes, 7 rows) ----------
const G = {
  C: ['XXXXX', 'XX...', 'XX...', 'XX...', 'XX...', 'XX...', 'XXXXX'],
  H: ['XX.XX', 'XX.XX', 'XX.XX', 'XXXXX', 'XX.XX', 'XX.XX', 'XX.XX'],
  E: ['XXXXX', 'XX...', 'XX...', 'XXXX.', 'XX...', 'XX...', 'XXXXX'],
  A: ['XXXXX', 'XX.XX', 'XX.XX', 'XXXXX', 'XX.XX', 'XX.XX', 'XX.XX'],
  P: ['XXXXX', 'XX.XX', 'XX.XX', 'XXXXX', 'XX...', 'XX...', 'XX...'],
  '-': ['...', '...', '...', 'XXX', '...', '...', '...'],
  O: ['XXXXX', 'XX.XX', 'XX.XX', 'XX.XX', 'XX.XX', 'XX.XX', 'XXXXX'],
  T: ['XXXXXX', '..XX..', '..XX..', '..XX..', '..XX..', '..XX..', '..XX..'],
};

// merge horizontal runs of one char into rects
function runs(rows, ch, ox, oy, p) {
  const r = [];
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      if (row[x] === ch) {
        let w = 0;
        while (row[x + w] === ch) w++;
        r.push(`<rect x="${(ox + x) * p}" y="${(oy + y) * p}" width="${w * p}" height="${p}"/>`);
        x += w;
      } else x++;
    }
  });
  return r.join('');
}

function layoutWord(word, gap = 1) {
  let x = 0;
  const parts = [];
  for (const ch of word) {
    parts.push({ ch, x });
    x += G[ch][0].length + gap;
  }
  return { parts, width: x - gap };
}

// Wordmark: "CHEAP-" in ink, "OTTO" knocked out of a yellow slab with hard shadow.
function wordmark(ink, shadow) {
  const p = 8;
  const a = layoutWord('CHEAP-');
  const b = layoutWord('OTTO');
  const pad = 2; // slab padding in units
  const slabX = a.width + 2;
  const slabW = b.width + pad * 2;
  const slabH = 7 + pad * 2;
  const W = (slabX + slabW + 1) * p; // + shadow
  const H = (slabH + 1) * p;
  const oyA = pad;
  const left = a.parts.map((q) => runs(G[q.ch], 'X', q.x, oyA, p)).join('');
  const right = b.parts.map((q) => runs(G[q.ch], 'X', slabX + pad + q.x, pad, p)).join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="cheap-otto" shape-rendering="crispEdges">
<title>cheap-otto</title>
<g fill="${ink}">${left}</g>
<rect x="${(slabX + 1) * p}" y="${p}" width="${slabW * p}" height="${slabH * p}" fill="${shadow}"/>
<rect x="${slabX * p + 2}" y="2" width="${slabW * p - 4}" height="${slabH * p - 4}" fill="${YELLOW}" stroke="${INK}" stroke-width="4"/>
<g fill="${INK}">${right}</g>
</svg>
`;
}

// Mark: o=o — two hollow square "o"s (the two machines) joined by a double bar (the bridge, the "tt").
function mark(shadow) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120" width="120" height="120" role="img" aria-label="cheap-otto mark" shape-rendering="crispEdges">
<title>cheap-otto mark</title>
<rect x="12" y="12" width="104" height="104" fill="${shadow}"/>
<rect x="4" y="4" width="104" height="104" fill="${YELLOW}" stroke="${INK}" stroke-width="8"/>
<g fill="none" stroke="${INK}" stroke-width="10">
<rect x="20" y="35" width="24" height="42"/>
<rect x="68" y="35" width="24" height="42"/>
</g>
<g fill="${INK}">
<rect x="49" y="46" width="14" height="7"/>
<rect x="49" y="59" width="14" height="7"/>
</g>
</svg>
`;
}

// Mascot: Otto the pixel otter peeking over a yellow laptop.
const OTTER = [
  '......KKKKKKKK......',
  '.KK.KKBBBBBBBBKK.KK.',
  'KBBKBBBBBBBBBBBBKBBK',
  'KBBBBBBBBBBBBBBBBBBK',
  'KBBBWKBBBBBBBBWKBBBK',
  'KBBBKKBBBBBBBBKKBBBK',
  'KBBBBBCCCCCCCCBBBBBK',
  'KBBBBCCCKKKKCCCBBBBK',
  'KBBBCCCCCKKCCCCCBBBK',
  'KBBBCKCCCCCCCCKCBBBK',
  'KBBBBCCCKCCKCCCBBBBK',
  '.KBBBBCCCKKCCCBBBBK.',
  'KKKKCCCKKKKKKCCCKKKK',
  'KYYKCCCKYYYYKCCCKYYK',
  'KYYYKKKYYYYYYKKKYYYK',
  'KYYYYYYYYYYYYYYYYYYK',
  'KYYYYYYYYYYYYYYYYYYK',
  'KKKKKKKKKKKKKKKKKKKK',
];
function otter(shadow) {
  const p = 10;
  const w = OTTER[0].length, h = OTTER.length;
  const sh = OTTER.map((r) => r.replace(/[^.]/g, 'S'));
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${(w + 1) * p} ${(h + 1) * p}" width="${(w + 1) * p}" height="${(h + 1) * p}" role="img" aria-label="Otto, the cheap-otto pixel otter" shape-rendering="crispEdges">
<title>Otto the otter</title>
<g fill="${shadow}">${runs(sh, 'S', 1, 1, p)}</g>
<g fill="${WHITE}">${runs(OTTER, 'W', 0, 0, p)}</g>
<g fill="${FUR}">${runs(OTTER, 'B', 0, 0, p)}</g>
<g fill="${MUZZLE}">${runs(OTTER, 'C', 0, 0, p)}</g>
<g fill="${YELLOW}">${runs(OTTER, 'Y', 0, 0, p)}</g>
<g fill="${INK}">${runs(OTTER, 'K', 0, 0, p)}</g>
</svg>
`;
}

const files = {
  'otto-mark.svg': mark(INK),
  'otto-mark-on-dark.svg': mark(WHITE),
  'otto-wordmark-ink.svg': wordmark(INK, INK),
  'otto-wordmark-white.svg': wordmark(WHITE, WHITE),
  'otto-pixel.svg': otter(INK),
  'otto-pixel-on-dark.svg': otter(YELLOW),
};
for (const [n, s] of Object.entries(files)) fs.writeFileSync(path.join(out, n), s);
console.log(Object.keys(files).join('\n'));
