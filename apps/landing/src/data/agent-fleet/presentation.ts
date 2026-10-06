// ── AgentFleet.tsx presentation data (additive — moved out of the component
//    in the PR-3 split so the component stays render-only) ──────────────────

// The hero illustration's SVG geometry — numeric attributes only; the shape
// types/classes/order stay in HeroSection.tsx.
export const HERO_SCENE = {
  viewBox: '0 0 560 220',
  authorBox: { x: 40, y: 70, width: 120, height: 92, rx: 14 },
  authorEyeL: { cx: 74, cy: 104, r: 8 },
  authorEyeR: { cx: 126, cy: 104, r: 8 },
  authorMouth: 'M78 134 q22 12 44 0',
  authorAntenna: 'M100 70 V52',
  authorAntennaTip: { cx: 100, cy: 48, r: 5 },
  authorArm: 'M150 120 l34 14',
  authorArrow: 'M182 128 l14 10 -10 6 z',
  diffCardBox: { x: 250, y: 92, width: 60, height: 74, rx: 7 },
  diffLine1: 'M262 112 h36',
  diffLine2: 'M262 126 h26',
  diffLine3: 'M262 140 h32',
  connector: 'M196 150 q90 30 150 -6',
  criticBox: { x: 400, y: 70, width: 120, height: 92, rx: 14 },
  criticBrow: 'M424 100 q12 -8 24 0',
  criticEyeL: { cx: 436, cy: 108, r: 7 },
  criticEyeR: { cx: 488, cy: 106, r: 11 },
  criticHandle: 'M496 114 l10 10',
  criticMouth: 'M428 140 q24 -8 48 0',
} as const;

// The six `<Divider>` squiggle paths, in page order.
export const DIVIDER_HERO = 'M2 9 Q160 2 320 8 T618 7';
export const DIVIDER_INTAKE = 'M2 8 Q160 14 320 7 T618 9';
export const DIVIDER_BELT = 'M2 9 Q160 3 320 9 T618 7';
export const DIVIDER_FLEET = 'M2 8 Q160 13 320 7 T618 9';
export const DIVIDER_TOGETHER = 'M2 9 Q160 2 320 8 T618 8';
export const DIVIDER_VERSUS = 'M2 8 Q160 13 320 7 T618 9';

// The Fleet section's color-key legend rows. Labels carry a leading space to
// match the single-text-node shape the hand-authored markup rendered.
export const FLEET_LEGEND: readonly { swatchClass: string; label: string }[] = [
  { swatchClass: 'sw author', label: ' author (writes)' },
  { swatchClass: 'sw critic', label: ' critic (audits)' },
  { swatchClass: 'sw cross', label: ' cross-cutting / risk' },
  { swatchClass: 'sw line', label: ' author ↔ critic pairing' },
  { swatchClass: 'sw line risk', label: ' risk secondary' },
];

// The "Work Together" cross-layer delegate prompt. The copied text and the
// displayed text differ only in quote style (straight vs curly-safe &quot;),
// so both variants share one phrase + body instead of duplicating the whole
// sentence.
const SAVE_JOB_PHRASE = 'save job to a folder';
const SAVE_JOB_PROMPT_BODY =
  'feature end-to-end: new IPC command + Rust store, a renderer panel to pick the folder, and ATS-aware tagging. Route it through the agent fleet.';
export const SAVE_JOB_PROMPT_COPY = `Implement a '${SAVE_JOB_PHRASE}' ${SAVE_JOB_PROMPT_BODY}`;
export const SAVE_JOB_PROMPT_TEXT = `Implement a "${SAVE_JOB_PHRASE}" ${SAVE_JOB_PROMPT_BODY}`;

// The three "Work Together" team cards. Each list item is a sequence of
// segments — `code` renders as <code>, `text` as plain text — so mixed
// inline-code + prose reproduces exactly without hardcoding JSX per item.
export type TeamSegment = { code: string } | { text: string };

export interface Team {
  title: string;
  items: readonly (readonly TeamSegment[])[];
}

export const WORK_TOGETHER_TEAMS: readonly Team[] = [
  {
    title: 'Disjoint files, in parallel',
    items: [
      [
        { code: 'rust-backend-author' },
        { text: ' — IPC command + SQLite store (' },
        { code: 'commands/' },
        { text: ', ' },
        { code: 'data_store.rs' },
        { text: ')' },
      ],
      [
        { code: 'frontend-author' },
        { text: ' — the folder-picker panel (' },
        { code: 'renderer/**' },
        { text: ')' },
      ],
      [{ code: 'job-match-author' }, { text: ' — the ATS tag logic' }],
    ],
  },
  {
    title: 'Critics challenge each other',
    items: [
      [
        { code: 'rust-backend-architect' },
        { text: ' + ' },
        { code: 'tauri-security-reviewer' },
        { text: ' on the new IPC surface' },
      ],
      [
        { code: 'frontend-reviewer' },
        { text: ' + ' },
        { code: 'ui-ux-expert' },
        { text: ' on the panel' },
      ],
      [{ code: 'job-match-expert' }, { text: ' on the scoring' }],
    ],
  },
  {
    title: 'Then the close-out',
    items: [
      [
        { code: 'test-author' },
        { text: ' → ' },
        { code: 'testing-reviewer' },
        { text: ' across all three' },
      ],
      [{ code: 'cleanup' }, { text: ' sweep' }],
      [{ code: 'project-steward' }, { text: ' docs + lessons' }],
    ],
  },
];
