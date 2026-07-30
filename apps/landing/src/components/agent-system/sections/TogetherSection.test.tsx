// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render } from '@testing-library/react';

import {
  SAVE_JOB_PROMPT_COPY,
  SAVE_JOB_PROMPT_TEXT,
  WORK_TOGETHER_TEAMS,
} from '@/data/agent-fleet';

import { TogetherSection } from './TogetherSection';

function stubClipboard() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  return writeText;
}

afterEach(() => {
  cleanup();
  Reflect.deleteProperty(navigator, 'clipboard');
});

describe('TogetherSection', () => {
  it('renders the big prompt chip with data-copy set to the copy variant, displaying the text variant', () => {
    const { container } = render(<TogetherSection />);
    const chip = container.querySelector('.copy-cmd');
    expect(chip?.textContent).toBe(SAVE_JOB_PROMPT_TEXT);
    expect(chip?.getAttribute('data-copy')).toBe(SAVE_JOB_PROMPT_COPY);
  });

  it('copies the copy-variant text (not the displayed text) on click', () => {
    const writeText = stubClipboard();
    const { container } = render(<TogetherSection />);
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected the big-prompt copy chip');

    fireEvent.click(chip);

    expect(writeText).toHaveBeenCalledWith(SAVE_JOB_PROMPT_COPY);
  });

  it.each(['Enter', ' '])('copies on key "%s"', (key) => {
    const writeText = stubClipboard();
    const { container } = render(<TogetherSection />);
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected the big-prompt copy chip');

    fireEvent.keyDown(chip, { key });

    expect(writeText).toHaveBeenCalledWith(SAVE_JOB_PROMPT_COPY);
  });

  it.each(['Escape', 'Tab', 'a'])('does not copy on key "%s"', (key) => {
    const writeText = stubClipboard();
    const { container } = render(<TogetherSection />);
    const chip = container.querySelector('.copy-cmd');
    if (!chip) throw new Error('expected the big-prompt copy chip');

    fireEvent.keyDown(chip, { key });

    expect(writeText).not.toHaveBeenCalled();
  });

  it('renders every WORK_TOGETHER_TEAMS card from the data', () => {
    const { container } = render(<TogetherSection />);
    const cards = container.querySelectorAll('.team');
    expect(cards).toHaveLength(WORK_TOGETHER_TEAMS.length);
    WORK_TOGETHER_TEAMS.forEach((team, i) => {
      const card = cards[i];
      if (!card) throw new Error(`expected a team card at index ${i}`);
      expect(card.querySelector('h4')?.textContent).toBe(team.title);

      const rows = card.querySelectorAll('li');
      expect(rows).toHaveLength(team.items.length);

      // Counts alone would pass on a dropped <code> wrapper or a reordered
      // segment, so pin each row's flattened text AND which segments are
      // <code> — the code/text split is the whole point of the data shape.
      team.items.forEach((segments, j) => {
        const row = rows[j];
        if (!row) throw new Error(`expected row ${j} of team ${i}`);
        expect(row.textContent).toBe(segments.map((s) => ('code' in s ? s.code : s.text)).join(''));
        expect([...row.querySelectorAll('code')].map((c) => c.textContent)).toEqual(
          segments.flatMap((s) => ('code' in s ? [s.code] : []))
        );
      });
    });
  });
});
