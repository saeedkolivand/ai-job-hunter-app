import { describe, expect, it, vi } from 'vitest';

import { mountTabs } from './tabs';

function mount() {
  const host = document.createElement('div');
  const onSelect = vi.fn();
  const view = mountTabs(
    host,
    [
      { id: 'job', label: 'Job' },
      { id: 'answers', label: 'Answers', count: 3 },
    ],
    { onSelect }
  );
  return { host, onSelect, view };
}

describe('mountTabs', () => {
  it('renders one tab button per spec, with a count badge only when > 0', () => {
    const { host } = mount();
    const buttons = host.querySelectorAll<HTMLButtonElement>('.tab');
    expect(buttons).toHaveLength(2);
    expect(buttons[0]?.textContent).toBe('Job');
    expect(buttons[1]?.textContent).toBe('Answers (3)');
  });

  it('renders one hidden panel per tab, in order, right after the bar', () => {
    const { host } = mount();
    const panels = host.querySelectorAll<HTMLElement>('.tab-body-section');
    expect(panels).toHaveLength(2);
    expect(panels[0]?.dataset.section).toBe('job');
    expect(panels[0]?.hidden).toBe(true);
  });

  it('calls onSelect with the clicked tab id', () => {
    const { host, onSelect } = mount();
    host.querySelector<HTMLButtonElement>('[data-tab="answers"]')!.click();
    expect(onSelect).toHaveBeenCalledWith('answers');
  });

  it('setActive marks exactly one tab/panel active and toggles aria-selected', () => {
    const { host, view } = mount();
    view.setActive('answers');

    const job = host.querySelector<HTMLButtonElement>('[data-tab="job"]')!;
    const answers = host.querySelector<HTMLButtonElement>('[data-tab="answers"]')!;
    expect(job.classList.contains('active')).toBe(false);
    expect(job.getAttribute('aria-selected')).toBe('false');
    expect(answers.classList.contains('active')).toBe(true);
    expect(answers.getAttribute('aria-selected')).toBe('true');

    expect(host.querySelector<HTMLElement>('[data-section="job"]')!.hidden).toBe(true);
    expect(host.querySelector<HTMLElement>('[data-section="answers"]')!.hidden).toBe(false);
  });

  it('setCount updates the badge without a full rebuild', () => {
    const { host, view } = mount();
    view.setCount('answers', 5);
    expect(host.querySelector('[data-tab="answers"]')!.textContent).toBe('Answers (5)');
    view.setCount('answers', 0);
    expect(host.querySelector('[data-tab="answers"]')!.textContent).toBe('Answers');
  });

  it('panel(id) returns the same node rendered into the DOM', () => {
    const { host, view } = mount();
    expect(view.panel('job')).toBe(host.querySelector('[data-section="job"]'));
  });

  it('panel(id) throws for an unknown id', () => {
    const { view } = mount();
    expect(() => view.panel('nope')).toThrow();
  });
});
