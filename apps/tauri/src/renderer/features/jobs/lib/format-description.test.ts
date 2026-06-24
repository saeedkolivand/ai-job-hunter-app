/**
 * Unit tests for formatJobDescription.
 *
 * Coverage:
 *  - empty / null-ish input
 *  - plain wall-of-text (strip_html output — one flat line)
 *  - bullet list with canonical • marker (html_to_text output)
 *  - bullet list with mixed markers (-, *, –, ▪, ‣)
 *  - numbered list (1. / 1))
 *  - heading-then-paragraph (single-line heading chunk)
 *  - heading embedded as first line of a multi-line chunk
 *  - mixed content (prose + heading + bullets + prose)
 *  - ALL-CAPS heading detection
 *  - colon-suffixed heading detection
 *  - no content loss (round-trip word preservation) on every case
 *  - 3+ consecutive blank lines collapsed (not split into empty blocks)
 *  - trailing spaces stripped (no stray whitespace in items/text)
 *
 * NOTE: The Rust layer already strips HTML tags and decodes entities, so this
 * formatter is NOT responsible for handling raw HTML/entities — no such tests.
 */

import { describe, expect, it } from 'vitest';

import { type DescriptionBlock, formatJobDescription } from './format-description';

// ── helpers ───────────────────────────────────────────────────────────────────

/** All words in all blocks, joined — used for round-trip content checks. */
function allWords(blocks: DescriptionBlock[]): string {
  return blocks
    .flatMap((b) => {
      if (b.type === 'list') return b.items;
      return [b.text];
    })
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim();
}

/** All words in the raw input, normalised the same way for fair comparison. */
function rawWords(raw: string): string {
  return raw.replace(/\s+/g, ' ').trim();
}

/**
 * Tokens that are intentionally stripped by the formatter (bullet/number
 * markers). The round-trip guarantee covers content words only — not markers.
 */
const MARKER_TOKENS = new Set(['•', '▪', '·', '‣', '–', '-', '*']);
const NUMBERED_TOKEN_RE = /^\d+[.)]$/;

/** Asserts no content words were dropped — markers are excluded. */
function assertNoContentLoss(raw: string, blocks: DescriptionBlock[]): void {
  const got = new Set(allWords(blocks).split(' ').filter(Boolean));
  const want = new Set(
    rawWords(raw)
      .split(' ')
      .filter(Boolean)
      .filter((w) => !MARKER_TOKENS.has(w) && !NUMBERED_TOKEN_RE.test(w))
  );
  for (const w of want) {
    expect(got, `word "${w}" was lost`).toContain(w);
  }
}

// ── empty / null-ish ──────────────────────────────────────────────────────────

describe('formatJobDescription — empty input', () => {
  it('returns [] for empty string', () => {
    expect(formatJobDescription('')).toEqual([]);
  });

  it('returns [] for whitespace-only string', () => {
    expect(formatJobDescription('   \n\n\t  ')).toEqual([]);
  });
});

// ── plain wall-of-text (strip_html output: one flat line) ─────────────────────

describe('formatJobDescription — flat wall-of-text', () => {
  it('wraps a single long sentence in one paragraph block', () => {
    const raw =
      'We are looking for a talented engineer to join our team and help us build great products. You will work closely with cross-functional teams.';
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]?.type).toBe('paragraph');
    assertNoContentLoss(raw, blocks);
  });

  it('multiple sentences with no newlines → single paragraph (no content loss)', () => {
    const raw =
      'Build and maintain scalable systems. Collaborate with product managers. Write clean, tested code. Mentor junior engineers.';
    const blocks = formatJobDescription(raw);
    // May be 1 paragraph (no structure to detect) — must not drop content.
    expect(blocks.length).toBeGreaterThanOrEqual(1);
    expect(blocks.every((b) => b.type === 'paragraph')).toBe(true);
    assertNoContentLoss(raw, blocks);
  });

  it('does not produce heading blocks for plain prose', () => {
    const raw = 'This is a job description with no special formatting at all.';
    const blocks = formatJobDescription(raw);
    expect(blocks.every((b) => b.type !== 'heading')).toBe(true);
  });
});

// ── bullet list with canonical • marker (html_to_text output) ────────────────

describe('formatJobDescription — canonical bullet list', () => {
  const raw = ['• Design and build APIs', '• Write unit tests', '• Review pull requests'].join(
    '\n'
  );

  it('produces a single list block', () => {
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]?.type).toBe('list');
  });

  it('strips the • marker from each item', () => {
    const blocks = formatJobDescription(raw);
    const list = blocks[0];
    expect(list?.type).toBe('list');
    if (list?.type !== 'list') return;
    expect(list.items).toEqual([
      'Design and build APIs',
      'Write unit tests',
      'Review pull requests',
    ]);
  });

  it('no content loss', () => {
    assertNoContentLoss(raw, formatJobDescription(raw));
  });
});

// ── mixed bullet markers ──────────────────────────────────────────────────────

describe('formatJobDescription — mixed bullet markers', () => {
  it('handles - marker', () => {
    const raw = '- Build features\n- Write docs';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items).toEqual(['Build features', 'Write docs']);
  });

  it('handles * marker', () => {
    const raw = '* Own the roadmap\n* Ship quarterly';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
  });

  it('handles – (en dash) marker', () => {
    const raw = '– Lead design\n– Present to stakeholders';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items[0]).toBe('Lead design');
  });

  it('handles ▪ and ‣ markers', () => {
    const raw = '▪ First item\n‣ Second item';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items).toHaveLength(2);
  });
});

// ── numbered list ─────────────────────────────────────────────────────────────

describe('formatJobDescription — numbered list', () => {
  it('handles 1. style numbering', () => {
    const raw = '1. First step\n2. Second step\n3. Third step';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items).toEqual(['First step', 'Second step', 'Third step']);
  });

  it('handles 1) style numbering', () => {
    const raw = '1) Clone the repo\n2) Run npm install\n3) Start the server';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items[0]).toBe('Clone the repo');
  });

  it('no content loss on numbered list', () => {
    const raw = '1. Design\n2. Implement\n3. Test\n4. Deploy';
    assertNoContentLoss(raw, formatJobDescription(raw));
  });
});

// ── heading detection ─────────────────────────────────────────────────────────

describe('formatJobDescription — heading detection', () => {
  it('detects a colon-suffixed heading followed by a paragraph', () => {
    const raw = 'Requirements:\n\nFive years of experience in software engineering.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    expect(blocks[0]?.type === 'heading' && blocks[0].text).toBe('Requirements:');
    expect(blocks[1]?.type).toBe('paragraph');
    assertNoContentLoss(raw, blocks);
  });

  it('detects ALL-CAPS heading', () => {
    const raw = 'ABOUT THE ROLE\n\nWe are building the future of work.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    assertNoContentLoss(raw, blocks);
  });

  it('detects Title-Case heading followed by content', () => {
    const raw = 'What You Will Do\n\nBuild and ship features.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    assertNoContentLoss(raw, blocks);
  });

  it('does NOT promote a heading when it is the last chunk (nothing follows)', () => {
    const raw = 'This Is The End';
    const blocks = formatJobDescription(raw);
    // Single last chunk → paragraph, not heading
    expect(blocks[0]?.type).toBe('paragraph');
  });

  it('does NOT promote a line ending with . as heading', () => {
    const raw = 'We are hiring great engineers.\n\nJoin our team.';
    const blocks = formatJobDescription(raw);
    expect(blocks.every((b) => b.type !== 'heading')).toBe(true);
  });

  it('first line of a multi-line chunk is extracted as heading when it qualifies', () => {
    const raw =
      'What We Offer\nCompetitive salary and equity package with full remote flexibility.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    expect(blocks[1]?.type).toBe('paragraph');
    assertNoContentLoss(raw, blocks);
  });
});

// ── mixed content ──────────────────────────────────────────────────────────────

describe('formatJobDescription — mixed content', () => {
  it('handles a realistic job description with prose + heading + bullets', () => {
    const raw = [
      'We are a fast-growing startup looking for a Senior Engineer.',
      '',
      'Responsibilities:',
      '',
      '• Architect and build scalable backend services',
      '• Collaborate with product and design',
      '• Mentor junior engineers',
      '',
      'You will thrive here if you have strong communication skills.',
    ].join('\n');

    const blocks = formatJobDescription(raw);

    const types = blocks.map((b) => b.type);
    expect(types).toContain('paragraph');
    expect(types).toContain('heading');
    expect(types).toContain('list');
    assertNoContentLoss(raw, blocks);
  });

  it('handles bullets mixed inside a paragraph chunk', () => {
    const raw =
      'Core skills required:\n• TypeScript\n• React\nSome experience with Rust is a plus.';
    const blocks = formatJobDescription(raw);
    // Must have at least one list block for the bullets.
    expect(blocks.some((b) => b.type === 'list')).toBe(true);
    assertNoContentLoss(raw, blocks);
  });

  it('3+ consecutive blank lines collapse to a single paragraph break', () => {
    const raw = 'First paragraph.\n\n\n\nSecond paragraph.';
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(2);
    expect(blocks[0]?.type).toBe('paragraph');
    expect(blocks[1]?.type).toBe('paragraph');
  });

  it('trailing spaces within a line do not appear in output', () => {
    const raw = 'A line with trailing spaces.   \n\nAnother line.   ';
    const blocks = formatJobDescription(raw);
    for (const b of blocks) {
      if (b.type === 'paragraph') expect(b.text).toBe(b.text.trim());
      if (b.type === 'list') b.items.forEach((item) => expect(item).toBe(item.trim()));
    }
  });
});

// ── edge cases ────────────────────────────────────────────────────────────────

describe('formatJobDescription — edge cases', () => {
  it('single word input returns a paragraph', () => {
    const blocks = formatJobDescription('Engineer');
    expect(blocks).toHaveLength(1);
    expect(blocks[0]?.type).toBe('paragraph');
  });

  it('heading without trailing content remains a paragraph (not a dangling heading)', () => {
    const raw = 'Requirements:\n\n';
    const blocks = formatJobDescription(raw);
    // After normalisation the trailing empty chunk disappears,
    // leaving Requirements: as the sole block → must be paragraph (no following content).
    expect(blocks[0]?.type).toBe('paragraph');
  });

  it('long line (>60 chars) is never a heading even if title-case', () => {
    const raw =
      'This Is A Very Long Title-Case Line That Exceeds Sixty Characters And Should Not Be A Heading\n\nParagraph text.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).not.toBe('heading');
  });

  it('line ending with ? is never a heading', () => {
    const raw = 'Ready To Join Us?\n\nApply now.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).not.toBe('heading');
  });
});

// ── CRLF normalization (blocker 1) ────────────────────────────────────────────

describe('formatJobDescription — CRLF normalization', () => {
  it('two CRLF-separated chunks produce two paragraph blocks', () => {
    // Lever/Ashby descriptionPlain may arrive with Windows line-endings.
    const raw = 'First paragraph.\r\n\r\nSecond paragraph.';
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(2);
    expect(blocks[0]?.type).toBe('paragraph');
    expect(blocks[1]?.type).toBe('paragraph');
    const p0 = blocks[0];
    const p1 = blocks[1];
    expect(p0?.type === 'paragraph' && p0.text).toBe('First paragraph.');
    expect(p1?.type === 'paragraph' && p1.text).toBe('Second paragraph.');
  });

  it('CRLF bullet list is parsed correctly', () => {
    const raw = '• Build APIs\r\n• Write tests\r\n• Ship features';
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]?.type).toBe('list');
    const list = blocks[0];
    if (list?.type !== 'list') return;
    expect(list.items).toEqual(['Build APIs', 'Write tests', 'Ship features']);
  });

  it('standalone CR (\\r) is also normalised', () => {
    const raw = 'Chunk one.\r\rChunk two.';
    const blocks = formatJobDescription(raw);
    expect(blocks).toHaveLength(2);
    expect(blocks[0]?.type).toBe('paragraph');
    expect(blocks[1]?.type).toBe('paragraph');
  });
});

// ── Title-Case false-positive guard (fold 5) ──────────────────────────────────

describe('formatJobDescription — Title-Case 2-word guard', () => {
  it('a 2-word Title-Case line stays a paragraph even with following content', () => {
    // "Remote Position" or "Join Google" must NOT become headings.
    const raw = 'Remote Position\n\nWe are hiring remotely.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('paragraph');
  });

  it('a 3-word Title-Case line with following content IS a heading', () => {
    const raw = 'What You Get\n\nBenefits and perks.';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
  });
});

// ── Heading-then-bullets in one chunk (fold 8) ────────────────────────────────

describe('formatJobDescription — heading-then-bullets in one chunk (no blank line)', () => {
  it('colon heading immediately followed by bullets → heading + list', () => {
    // No blank line between heading and bullets — must NOT collapse to paragraph.
    const raw = 'Requirements:\n• 5 years TypeScript\n• Strong React skills';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    expect(blocks[0]?.type === 'heading' && blocks[0].text).toBe('Requirements:');
    expect(blocks[1]?.type).toBe('list');
    const list = blocks[1];
    if (list?.type !== 'list') return;
    expect(list.items).toEqual(['5 years TypeScript', 'Strong React skills']);
    assertNoContentLoss(raw, blocks);
  });

  it('ALL-CAPS heading immediately followed by bullets → heading + list', () => {
    const raw = 'RESPONSIBILITIES\n• Lead architecture\n• Mentor team';
    const blocks = formatJobDescription(raw);
    expect(blocks[0]?.type).toBe('heading');
    expect(blocks[1]?.type).toBe('list');
    assertNoContentLoss(raw, blocks);
  });

  it('heading-then-bullets inside a larger description → correct structure', () => {
    const raw = [
      'We are hiring.',
      '',
      'Requirements:\n• TypeScript\n• React',
      '',
      'Apply today.',
    ].join('\n');
    const blocks = formatJobDescription(raw);
    const types = blocks.map((b) => b.type);
    expect(types).toContain('heading');
    expect(types).toContain('list');
    assertNoContentLoss(raw, blocks);
  });
});
