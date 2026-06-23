import { Briefcase, Globe } from 'lucide-react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { type TabItem, Tabs } from './Tabs';

type Section = 'overview' | 'details' | 'notes';

const ITEMS: readonly TabItem<Section>[] = [
  { value: 'overview', label: 'Overview' },
  { value: 'details', label: 'Details' },
  { value: 'notes', label: 'Notes' },
];

function Harness({
  initial = 'details' as Section,
  onChange,
}: {
  initial?: Section;
  onChange?: (v: Section) => void;
}) {
  const [value, setValue] = useState<Section>(initial);
  return (
    <Tabs
      ariaLabel="Application sections"
      items={ITEMS}
      value={value}
      onChange={(v) => {
        setValue(v);
        onChange?.(v);
      }}
    />
  );
}

describe('Tabs', () => {
  it('renders role=tablist with aria-label', () => {
    render(<Harness />);
    expect(screen.getByRole('tablist', { name: 'Application sections' })).toBeInTheDocument();
  });

  it('renders one role=tab per item', () => {
    render(<Harness />);
    expect(screen.getAllByRole('tab')).toHaveLength(3);
  });

  it('aria-selected tracks the active value', () => {
    render(<Harness initial="details" />);
    expect(screen.getByRole('tab', { name: 'Details' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tab', { name: 'Overview' })).toHaveAttribute('aria-selected', 'false');
    expect(screen.getByRole('tab', { name: 'Notes' })).toHaveAttribute('aria-selected', 'false');
  });

  it('roving tabindex: only the selected tab is tabbable', () => {
    render(<Harness initial="details" />);
    expect(screen.getByRole('tab', { name: 'Details' })).toHaveAttribute('tabindex', '0');
    expect(screen.getByRole('tab', { name: 'Overview' })).toHaveAttribute('tabindex', '-1');
    expect(screen.getByRole('tab', { name: 'Notes' })).toHaveAttribute('tabindex', '-1');
  });

  it('click calls onChange and updates aria-selected', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="overview" onChange={onChange} />);
    await user.click(screen.getByRole('tab', { name: 'Notes' }));
    expect(onChange).toHaveBeenCalledWith('notes');
    expect(screen.getByRole('tab', { name: 'Notes' })).toHaveAttribute('aria-selected', 'true');
  });

  it('ArrowRight moves selection forward and wraps', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="details" onChange={onChange} />);
    screen.getByRole('tab', { name: 'Details' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('notes');
    // wrap: notes -> overview
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('overview');
  });

  it('ArrowLeft moves selection backward and wraps', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="overview" onChange={onChange} />);
    screen.getByRole('tab', { name: 'Overview' }).focus();
    await user.keyboard('{ArrowLeft}');
    // wrap: overview -> notes (last)
    expect(onChange).toHaveBeenCalledWith('notes');
  });

  it('ArrowRight from a focused tab when value is not in items moves to the tab after the focused one', async () => {
    // value 'other' is not in ITEMS, so currentIndex === -1.
    // The first tab gets tabIndex=0 by the roving fallback, but we manually
    // focus the middle tab ('details', index 1) and press ArrowRight — the
    // result must be 'notes' (index 2), NOT 'details' (index 1 = 0 + 1).
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <Tabs
        ariaLabel="Application sections"
        items={ITEMS}
        value={'other' as Section}
        onChange={onChange}
      />
    );
    // Focus the middle tab explicitly so focusedIndex.current becomes 1.
    screen.getByRole('tab', { name: 'Details' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('notes');
  });

  it('Home jumps to first tab, End jumps to last tab', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness initial="details" onChange={onChange} />);
    screen.getByRole('tab', { name: 'Details' }).focus();
    await user.keyboard('{End}');
    expect(onChange).toHaveBeenLastCalledWith('notes');
    await user.keyboard('{Home}');
    expect(onChange).toHaveBeenLastCalledWith('overview');
  });

  it('active tab has bg-brand/15, text-foreground, and font-semibold classes (WCAG AA fix)', () => {
    render(<Harness initial="overview" />);
    const activeTab = screen.getByRole('tab', { name: 'Overview' });
    expect(activeTab.className).toContain('bg-brand/15');
    expect(activeTab.className).toContain('text-foreground');
    expect(activeTab.className).toContain('font-semibold');
  });

  it('active tab does not use text-brand (fails WCAG AA at 11px in light mode ≈4.14:1)', () => {
    render(<Harness initial="overview" />);
    const activeTab = screen.getByRole('tab', { name: 'Overview' });
    expect(activeTab.className).not.toContain('text-brand');
  });

  it('inactive tabs do not have bg-brand/15', () => {
    render(<Harness initial="overview" />);
    const inactiveTab = screen.getByRole('tab', { name: 'Details' });
    expect(inactiveTab.className).not.toContain('bg-brand/15');
  });

  it('every tab has a focus-visible ring class for keyboard visibility', () => {
    render(<Harness initial="overview" />);
    for (const tab of screen.getAllByRole('tab')) {
      expect(tab.className).toContain('focus-visible:ring-2');
    }
  });

  it('inactive tab uses text-foreground/70 (WCAG AA ≥4.5:1 in light mode, advisory in dark)', () => {
    render(<Harness initial="overview" />);
    const inactiveTab = screen.getByRole('tab', { name: 'Details' });
    expect(inactiveTab.className).toContain('text-foreground/70');
    expect(inactiveTab.className).not.toContain('text-foreground/50');
  });

  it('tab buttons meet minimum touch target height (min-h-[24px])', () => {
    render(<Harness initial="overview" />);
    for (const tab of screen.getAllByRole('tab')) {
      expect(tab.className).toContain('min-h-[24px]');
    }
  });

  it('wires explicit item.id onto the button element', () => {
    render(
      <Tabs
        ariaLabel="Sections"
        items={[{ value: 'a', label: 'A', id: 'my-tab-a' }]}
        value="a"
        onChange={() => {}}
      />
    );
    expect(screen.getByRole('tab', { name: 'A' })).toHaveAttribute('id', 'my-tab-a');
  });

  it('derives id from idBase when item.id is absent', () => {
    render(
      <Tabs
        ariaLabel="Sections"
        items={[{ value: 'foo', label: 'Foo' }]}
        value="foo"
        idBase="my-tabs"
        onChange={() => {}}
      />
    );
    expect(screen.getByRole('tab', { name: 'Foo' })).toHaveAttribute('id', 'my-tabs-foo');
  });

  it('emits aria-controls on the button when ariaControls is provided', () => {
    render(
      <Tabs
        ariaLabel="Sections"
        items={[{ value: 'a', label: 'A', ariaControls: 'panel-a' }]}
        value="a"
        onChange={() => {}}
      />
    );
    expect(screen.getByRole('tab', { name: 'A' })).toHaveAttribute('aria-controls', 'panel-a');
  });

  it('does not emit aria-controls when ariaControls is absent', () => {
    render(
      <Tabs
        ariaLabel="Sections"
        items={[{ value: 'b', label: 'B' }]}
        value="b"
        onChange={() => {}}
      />
    );
    expect(screen.getByRole('tab', { name: 'B' })).not.toHaveAttribute('aria-controls');
  });

  it('renders icon when provided', () => {
    render(
      <Tabs
        ariaLabel="With icons"
        items={[
          { value: 'a', label: 'Alpha', icon: Briefcase },
          { value: 'b', label: 'Beta', icon: Globe },
        ]}
        value="a"
        onChange={() => {}}
      />
    );
    // Icons have aria-hidden, labels still visible
    expect(screen.getByRole('tab', { name: 'Alpha' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Beta' })).toBeInTheDocument();
  });
});
