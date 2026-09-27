Navigator row icons
===================

Rasterized Lucide line icons (#442), replacing the hand-drawn portable `Glyph`
approximations on the navigator rows.

  svg/     vendored Lucide sources, pinned to lucide-static 1.48.0 (ISC).
  light/   PNGs rasterized at 32x32 with the light theme's text colour (#1B1B1B).
  dark/    PNGs rasterized at 32x32 with the dark theme's text colour (#FFFFFF).

32 px is 2x the tree's 16-dip row icon slot, so the bitmap stays crisp on
HiDPI and downscales cleanly at 100%. `star.svg` is reused outlined (Starred)
and filled (Most Played).

Regenerate after changing the sources:

  npm install --no-save @resvg/resvg-js
  node scripts/gen-navigator-icons.mjs

Lucide is ISC-licensed; see THIRD-PARTY-NOTICES.md.
