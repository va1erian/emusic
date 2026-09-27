// Regenerates the navigator's rasterized Lucide icons (#442).
//
// The navigator rows carry a bitmap icon instead of the old hand-drawn
// `Glyph`s, so the software (canvas) backend renders the same crisp shapes as
// the native one. The sources are the Lucide line icons, pinned to the version
// below and vendored under crates/app/assets/navigator/svg/ (ISC licence; see
// THIRD-PARTY-NOTICES.md).
//
// Each SVG is rasterized twice — once with the light theme's text colour and
// once with the dark theme's — into
//   crates/app/assets/navigator/<theme>/<view-slug>.png
// at 2x the tree's 16-dip row icon slot, so the bitmap stays sharp on HiDPI
// and downscales cleanly at 100%. `star` is reused outlined (Starred) and
// filled (Most Played).
//
// Run it (Node 18+) after a one-time `npm install --no-save @resvg/resvg-js`
// from the repository root:
//
//   node scripts/gen-navigator-icons.mjs
//
// Commit the regenerated PNGs with the SVG sources. The app never runs this
// script; it only embeds the committed PNGs.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { Resvg } from '@resvg/resvg-js';

/** The pinned Lucide release the vendored SVGs come from. */
const LUCIDE_VERSION = '1.48.0';
/** The rasterized edge, in pixels: 2x the tree's 16-dip icon slot. */
const SIZE = 32;

/** Stroke colour that reads on the light theme surface (its `text` token). */
const LIGHT_STROKE = '#1b1b1b';
/** Stroke colour that reads on the dark theme surface (its `text` token). */
const DARK_STROKE = '#ffffff';

/**
 * The navigator rows and their Lucide source. `file` names the vendored SVG
 * and `filled` sets `fill` to the stroke colour so a solid shape can reuse an
 * outline icon (`star`).
 */
const ICONS = [
  { slug: 'music', file: 'music', filled: false },
  { slug: 'albums', file: 'disc-3', filled: false },
  { slug: 'artists', file: 'users', filled: false },
  { slug: 'genres', file: 'tags', filled: false },
  { slug: 'folders', file: 'folder', filled: false },
  { slug: 'starred', file: 'star', filled: false },
  { slug: 'most-played', file: 'star', filled: true },
  { slug: 'history', file: 'history', filled: false },
  { slug: 'now-playing', file: 'play', filled: false },
  { slug: 'visualization', file: 'monitor', filled: false },
  { slug: 'settings', file: 'settings', filled: false },
];

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const assets = join(root, 'crates', 'app', 'assets', 'navigator');

/**
 * Prepares one vendored SVG for rasterization: pins the stroke colour, opts
 * into a filled shape, and scales the 24px viewBox up to {@link SIZE}.
 */
function prepare(svg, color, filled) {
  return svg
    .replaceAll('currentColor', color)
    .replace('fill="none"', filled ? 'fill="currentColor"' : 'fill="none"')
    .replace('width="24"', `width="${SIZE}"`)
    .replace('height="24"', `height="${SIZE}"`);
}

/** Rasterizes `svg` to a transparent PNG at {@link SIZE} px. */
function rasterize(svg) {
  const resvg = new Resvg(svg, {
    fitTo: { mode: 'width', value: SIZE },
    background: 'rgba(0,0,0,0)',
  });
  return resvg.render().asPng();
}

for (const { slug, file, filled } of ICONS) {
  const source = readFileSync(join(assets, 'svg', `${file}.svg`), 'utf8');
  for (const [theme, color] of [
    ['light', LIGHT_STROKE],
    ['dark', DARK_STROKE],
  ]) {
    const png = rasterize(prepare(source, color, filled));
    const out = join(assets, theme, `${slug}.png`);
    writeFileSync(out, png);
    console.log(`wrote ${out.slice(root.length + 1)} (${png.length} bytes)`);
  }
}

console.log(`navigator icons regenerated from Lucide ${LUCIDE_VERSION}`);
