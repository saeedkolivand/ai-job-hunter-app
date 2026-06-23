import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Dropdown, type DropdownOption } from './Dropdown';

const OPTIONS: DropdownOption[] = [
  { value: 'a', label: 'Apple' },
  { value: 'b', label: 'Banana', meta: 'fruit' },
  { value: 'c', label: 'Cherry', section: 'Stone fruit' },
];

const LANG_OPTIONS: DropdownOption[] = [
  { value: 'en', label: 'English' },
  { value: 'de', label: 'German' },
  { value: 'fr', label: 'French' },
];

describe('Dropdown', () => {
  it('shows the placeholder when nothing is selected', () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} placeholder="Pick one" />);
    expect(screen.getByText('Pick one')).toBeInTheDocument();
  });

  it('shows the selected option label', () => {
    render(<Dropdown options={OPTIONS} value="b" onChange={() => {}} />);
    expect(screen.getByText('Banana')).toBeInTheDocument();
  });

  it('opens on click and selects an option', async () => {
    const onChange = vi.fn();
    render(<Dropdown options={OPTIONS} value="" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.click(screen.getByText('Cherry'));
    expect(onChange).toHaveBeenCalledWith('c');
  });

  it('filters options via the search box', async () => {
    const many: DropdownOption[] = Array.from({ length: 8 }, (_, i) => ({
      value: String(i),
      label: `Option ${i}`,
    }));
    render(<Dropdown options={many} value="" onChange={() => {}} searchable />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.type(screen.getByPlaceholderText('Search…'), 'Option 3');
    expect(screen.getByText('Option 3')).toBeInTheDocument();
    expect(screen.queryByText('Option 5')).not.toBeInTheDocument();
  });

  it('shows a "No results" message when nothing matches', async () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} searchable />);
    await userEvent.click(screen.getByRole('button'));
    await userEvent.type(screen.getByPlaceholderText('Search…'), 'zzz');
    expect(screen.getByText('No results')).toBeInTheDocument();
  });

  it('does not open when disabled', async () => {
    render(<Dropdown options={OPTIONS} value="" onChange={() => {}} disabled />);
    await userEvent.click(screen.getByRole('button')).catch(() => {});
    expect(screen.queryByText('Apple')).not.toBeInTheDocument();
  });

  // ── Additional coverage (keyboard nav, listbox role, auto-search) ─────────

  it('opens as a listbox and selects an option by click', async () => {
    const onChange = vi.fn();
    render(<Dropdown options={LANG_OPTIONS} value="" onChange={onChange} />);
    await userEvent.click(screen.getByRole('button'));
    const listbox = await screen.findByRole('listbox');
    expect(listbox).toBeInTheDocument();
    await userEvent.click(screen.getByRole('option', { name: 'French' }));
    expect(onChange).toHaveBeenCalledWith('fr');
  });

  it('supports keyboard navigation (ArrowDown + Enter)', async () => {
    const onChange = vi.fn();
    render(<Dropdown options={LANG_OPTIONS} value="" onChange={onChange} />);
    const trigger = screen.getByRole('button');
    trigger.focus();
    await userEvent.keyboard('{ArrowDown}'); // opens
    await userEvent.keyboard('{ArrowDown}{Enter}');
    expect(onChange).toHaveBeenCalled();
  });

  it('renders a search box when there are many options', async () => {
    const many = Array.from({ length: 10 }, (_, i) => ({ value: `v${i}`, label: `Item ${i}` }));
    render(<Dropdown options={many} value="" onChange={() => {}} />);
    await userEvent.click(screen.getByRole('button'));
    const listbox = await screen.findByRole('listbox');
    expect(listbox).toBeInTheDocument();
    // Search box auto-shown at threshold >= 8
    expect(screen.getByPlaceholderText('Search…')).toBeInTheDocument();
  });

  // ── New coverage flagged by testing-reviewer ──────────────────────────────

  describe('keyboard: Escape', () => {
    it('closes the open listbox and returns focus to the trigger', async () => {
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} />);
      const trigger = screen.getByRole('button');
      trigger.focus();
      await userEvent.keyboard('{ArrowDown}'); // open
      await screen.findByRole('listbox');
      await userEvent.keyboard('{Escape}');
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
      expect(trigger).toHaveFocus();
    });
  });

  describe('keyboard: ArrowUp', () => {
    it('moves highlight upward — ArrowDown to idx1 then ArrowUp selects idx0', async () => {
      const onChange = vi.fn();
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={onChange} />);
      const trigger = screen.getByRole('button');
      trigger.focus();
      await userEvent.keyboard('{ArrowDown}'); // open; highlight syncs to current value (none → -1 or 0)
      await screen.findByRole('listbox');
      await userEvent.keyboard('{ArrowDown}'); // highlight 0 → 1
      await userEvent.keyboard('{ArrowUp}'); // highlight 1 → 0
      await userEvent.keyboard('{Enter}');
      expect(onChange).toHaveBeenCalledWith('en'); // first option value
    });
  });

  describe('keyboard: Space', () => {
    it('opens the listbox when the trigger is focused and Space is pressed', async () => {
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} />);
      const trigger = screen.getByRole('button');
      trigger.focus();
      await userEvent.keyboard(' ');
      await screen.findByRole('listbox');
    });
  });

  describe('keyboard: Tab', () => {
    it('closes the listbox without firing onChange', async () => {
      const onChange = vi.fn();
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={onChange} />);
      const trigger = screen.getByRole('button');
      trigger.focus();
      await userEvent.keyboard('{ArrowDown}'); // open
      await screen.findByRole('listbox');
      // Fire Tab via keyDown on the trigger (userEvent.tab() moves focus away
      // which also collapses; we assert onChange was never called either way)
      await userEvent.keyboard('{Tab}');
      expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe('keyboard: ArrowDown-from-closed then Enter selects first option', () => {
    it('calls onChange with the exact value of the first option', async () => {
      const onChange = vi.fn();
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={onChange} />);
      const trigger = screen.getByRole('button');
      trigger.focus();
      await userEvent.keyboard('{ArrowDown}'); // opens; highlight set to findIndex of current value → -1 (no match)
      await screen.findByRole('listbox');
      // After open with no prior value the highlight is -1; one ArrowDown brings it to 0
      await userEvent.keyboard('{ArrowDown}');
      await userEvent.keyboard('{Enter}');
      expect(onChange).toHaveBeenCalledWith('en');
    });
  });

  describe('auto-search threshold boundary', () => {
    it('does NOT show a search box for 7 options', async () => {
      const seven = Array.from({ length: 7 }, (_, i) => ({ value: `v${i}`, label: `Item ${i}` }));
      render(<Dropdown options={seven} value="" onChange={() => {}} />);
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      expect(screen.queryByPlaceholderText('Search…')).not.toBeInTheDocument();
    });

    it('shows a search box for exactly 8 options', async () => {
      const eight = Array.from({ length: 8 }, (_, i) => ({ value: `v${i}`, label: `Item ${i}` }));
      render(<Dropdown options={eight} value="" onChange={() => {}} />);
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      expect(screen.getByPlaceholderText('Search…')).toBeInTheDocument();
    });
  });

  describe('section header', () => {
    it('renders the section header text when options carry a section', async () => {
      render(<Dropdown options={OPTIONS} value="" onChange={() => {}} />);
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      expect(screen.getByText('Stone fruit')).toBeInTheDocument();
    });
  });

  describe('meta text', () => {
    it('renders the meta text next to its option', async () => {
      render(<Dropdown options={OPTIONS} value="" onChange={() => {}} />);
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      expect(screen.getByText('fruit')).toBeInTheDocument();
    });
  });

  describe('icon rendering', () => {
    it('renders an icon inside an option row when the option has an icon', async () => {
      const withIcon: DropdownOption[] = [
        { value: 'x', label: 'X-ray', icon: <span data-testid="opt-icon">★</span> },
        { value: 'y', label: 'Yellow' },
      ];
      render(<Dropdown options={withIcon} value="" onChange={() => {}} />);
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      expect(screen.getByTestId('opt-icon')).toBeInTheDocument();
    });

    it('renders the trigger icon when the icon prop is passed', () => {
      render(
        <Dropdown
          options={LANG_OPTIONS}
          value=""
          onChange={() => {}}
          icon={<span data-testid="trigger-icon">◆</span>}
        />
      );
      expect(screen.getByTestId('trigger-icon')).toBeInTheDocument();
    });
  });

  describe('className passthrough', () => {
    it('merges className onto the trigger button, overriding the default height', () => {
      render(
        <Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} className="h-9 shadow-none" />
      );
      const trigger = screen.getByRole('button');
      // className is appended last inside cn() so twMerge resolves h-9 over the default h-8
      expect(trigger.className).toContain('h-9');
      expect(trigger.className).not.toMatch(/\bh-8\b/);
    });
  });

  describe('tone="field"', () => {
    it('keeps a brand border when open and does not retain the rest border-white class', async () => {
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} tone="field" />);
      const trigger = screen.getByRole('button');
      // Open the dropdown
      await userEvent.click(trigger);
      await screen.findByRole('listbox');
      // Must have brand border in open state
      expect(trigger.className).toMatch(/border-brand/);
      // Must NOT retain the rest-state white border class once open
      expect(trigger.className).not.toMatch(/border-white/);
    });

    it('applies text-foreground/40 to the chevron (matching form-field icon opacity)', () => {
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} tone="field" />);
      // The ChevronDown svg is the last svg inside the trigger (SVGAnimatedString — use getAttribute)
      const trigger = screen.getByRole('button');
      const chevron = trigger.querySelector('svg:last-of-type');
      expect(chevron).not.toBeNull();
      if (!chevron) return;
      const cls = chevron.getAttribute('class') ?? '';
      // text-foreground/40 must be on the chevron, not the dimmer /30 (default) or brand (primary)
      expect(cls).toContain('text-foreground/40');
      expect(cls).not.toContain('text-foreground/30');
    });

    it('keeps the default chevron at text-foreground/30 when tone is default', () => {
      render(<Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} tone="default" />);
      const trigger = screen.getByRole('button');
      const chevron = trigger.querySelector('svg:last-of-type');
      expect(chevron).not.toBeNull();
      if (!chevron) return;
      const cls = chevron.getAttribute('class') ?? '';
      expect(cls).toContain('text-foreground/30');
      expect(cls).not.toContain('text-foreground/40');
    });
  });

  describe('listClassName override', () => {
    it('applies the custom class to the list container', async () => {
      render(
        <Dropdown options={LANG_OPTIONS} value="" onChange={() => {}} listClassName="max-h-40" />
      );
      await userEvent.click(screen.getByRole('button'));
      await screen.findByRole('listbox');
      // The list container is the div that wraps the option rows
      const container = document.querySelector('.max-h-40');
      expect(container).toBeInTheDocument();
    });
  });
});
