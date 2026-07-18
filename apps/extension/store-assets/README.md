# Store assets — AI Job Hunter browser extension

Deterministic, regenerable Chrome Web Store assets. Every file is composited
from HTML/CSS via headless Chromium (Playwright) — no AI image generation — so
text is crisp and pixel dimensions are exact. The brand language (paper / ink /
red, hand-drawn doodles, film grain, the same Google Fonts) is lifted from
`landing/index.html`.

Firefox AMO reuses the **same** 1280×800 screenshot set (AMO accepts the same
dimensions); no separate render is needed.

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

| File                            | Size (px) | Chrome Web Store field       |
| ------------------------------- | --------- | ---------------------------- |
| `screenshots/01-private.png`    | 1280×800  | Screenshot 1 (store listing) |
| `screenshots/02-pair-once.png`  | 1280×800  | Screenshot 2 (store listing) |
| `screenshots/03-one-click.png`  | 1280×800  | Screenshot 3 (store listing) |
| `screenshots/04-fill-forms.png` | 1280×800  | Screenshot 4 (store listing) |
| `screenshots/05-answers.png`    | 1280×800  | Screenshot 5 (store listing) |
| `promo/promo-tile-440x280.png`  | 440×280   | Small promo tile             |
| `promo/marquee-1400x560.png`    | 1400×560  | Marquee promo tile           |

Chrome caps a listing at five screenshots; this is the full set. They are ordered
to give the store a clean narrative:
**private → pair → import → autofill → answer tools**.

### Screenshot captions

- `01-private.png` — "private by default — it only talks to the app on your own
  computer" (offline popup state; red arrow → **Get the app**).
- `02-pair-once.png` — "pair once — paste the token from the desktop app"
  (pairing popup state; red arrow → **Save & pair**).
- `03-one-click.png` — "one click → the job's saved in your app, tagged New"
  (connected popup state; red arrow → **Import this job**).
- `04-fill-forms.png` — "opt-in autofill — your saved details, filled on your
  command. you review every field; it never submits." (connected popup state,
  post-fill message shown; red arrow → **Fill this form**).
- `05-answers.png` — "stuck on 'why do you want to work here?' — it drafts an
  answer from your resume. copy it when you're happy." (connected popup state,
  Answer tools open with a drafted answer; red arrow → **Help me answer…**).

Captions stay honest and non-overstated: autofill is opt-in, fills only empty
visible fields on your command, and never submits; the answer draft is written
from your own résumé and page context and is copy-only (nothing auto-typed).

## Intermediate artifacts

`screenshots/raw/` contains five element screenshots of the real popup card, one
per forced UI state: `popup-{offline,pairing,connected,fill,answers}.png`. Each
is a light-mode element screenshot of `main.app` at 2× device scale, so the PNG
width tracks the card's rendered CSS width (currently 340px → 680px output) and
the height varies with each state's content. They are inputs to the composited
screenshots above; not uploaded to the store.

## Brand tokens (from `landing/index.html`)

- Paper `#f4ecdc`, ink `#1c1812`, red `#e24b4a`.
- Fonts (Google Fonts): Patrick Hand, Gloria Hallelujah, Anton, Space Mono.
- Film-grain overlay: the landing's `feTurbulence` SVG data-URI at ~6% opacity,
  `mix-blend-mode: overlay`.
- Hand-drawn red arrow: computed per shot at composite time — the tail anchors
  just off the measured caption bbox, the tip lands just outside the card edge
  level with the measured target button, and the curve (bow, wobble, stroke
  width, arrowhead) is derived from that geometry with seeded per-shot jitter,
  so no two arrows are identical and the stroke never touches the caption
  (asserted with 12px clearance). The marquee accent still uses the landing's
  `.dc-arrow` path, recoloured to the brand red.
- Brand mark (promo art): the extension's real icon, `src/icons/icon-128.png`.
