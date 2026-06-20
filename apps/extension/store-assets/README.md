# Store assets — AI Job Hunter browser extension

Deterministic, regenerable Chrome Web Store assets. Every file is composited
from HTML/CSS via headless Chromium (Playwright) — no AI image generation — so
text is crisp and pixel dimensions are exact. The brand language (paper / ink /
red, hand-drawn doodles, film grain, the same Google Fonts) is lifted from
`landing/index.html`.

Regenerate everything:

```bash
pnpm -F @ajh/extension build:chrome        # produces dist/chrome/popup.{html,css,js}
node apps/extension/scripts/gen-store-assets.mjs
```

The generator (`apps/extension/scripts/gen-store-assets.mjs`) captures the REAL
built popup (it blocks `popup.js` so the 3s timeout can't force the offline view,
lets `popup.css` load, and forces each UI state by DOM manipulation), then
composites the store-facing assets around those captures.

## Files

| File                           | Size (px) | Chrome Web Store field       |
| ------------------------------ | --------- | ---------------------------- |
| `screenshots/01-private.png`   | 1280×800  | Screenshot 1 (store listing) |
| `screenshots/02-pair-once.png` | 1280×800  | Screenshot 2 (store listing) |
| `screenshots/03-one-click.png` | 1280×800  | Screenshot 3 (store listing) |
| `promo/promo-tile-440x280.png` | 440×280   | Small promo tile             |
| `promo/marquee-1400x560.png`   | 1400×560  | Marquee promo tile           |

The three screenshots are ordered to give the store a clean narrative:
**private → pair → import**.

### Screenshot captions

- `01-private.png` — "private by default — it only talks to the app on your own
  computer" (offline popup state; red arrow → the empty-state title).
- `02-pair-once.png` — "pair once — paste the token from the desktop app"
  (pairing popup state; red arrow → **Save & pair**).
- `03-one-click.png` — "one click → the job's saved in your app, tagged New"
  (connected popup state; red arrow → **Import via URL**).

## Intermediate artifacts

`screenshots/raw/` contains 6 element screenshots of the real popup card — light
and dark variants for each of the three states:
`popup-{offline,pairing,connected}.png` and `dark-popup-{offline,pairing,connected}.png`.
Each is captured at a 360px CSS viewport × 2 device scale (720px output width).
They are inputs to the composited screenshots above; not uploaded to the store.

## Brand tokens (from `landing/index.html`)

- Paper `#f4ecdc`, ink `#1c1812`, red `#e24b4a`.
- Fonts (Google Fonts): Patrick Hand, Gloria Hallelujah, Anton, Space Mono.
- Film-grain overlay: the landing's `feTurbulence` SVG data-URI at ~6% opacity,
  `mix-blend-mode: overlay`.
- Hand-drawn red arrow: the landing's `.dc-arrow` path, recoloured to the brand
  red and re-oriented per shot.
- Brand mark (promo art): the extension's real icon, `src/icons/icon-128.png`.
