import { describe, expect, it, vi } from 'vitest';
import { createEvent, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { MarkdownMessage } from './MarkdownMessage';

describe('MarkdownMessage', () => {
  it('renders headings of each level', () => {
    render(<MarkdownMessage content={'# H1\n## H2\n### H3'} />);
    expect(screen.getByText('H1')).toBeInTheDocument();
    expect(screen.getByText('H2')).toBeInTheDocument();
    expect(screen.getByText('H3')).toBeInTheDocument();
  });

  it('renders a fenced code block with a language label', () => {
    const { container } = render(<MarkdownMessage content={'```ts\nconst x = 1;\n```'} />);
    expect(container.querySelector('pre code')?.textContent).toContain('const x = 1;');
    expect(screen.getByText('ts')).toBeInTheDocument();
  });

  it('renders unordered and ordered lists', () => {
    const { container } = render(
      <MarkdownMessage content={'- one\n- two\n\n1. first\n2. second'} />
    );
    expect(container.querySelector('ul')).toBeTruthy();
    expect(container.querySelector('ol')).toBeTruthy();
    expect(screen.getByText('one')).toBeInTheDocument();
    expect(screen.getByText('first')).toBeInTheDocument();
  });

  it('renders a blockquote and a horizontal rule', () => {
    const { container } = render(<MarkdownMessage content={'> quoted line\n\n---'} />);
    expect(container.querySelector('blockquote')).toBeTruthy();
    expect(container.querySelector('hr')).toBeTruthy();
  });

  it('renders inline emphasis: bold, italic and code', () => {
    const { container } = render(
      <MarkdownMessage content={'A **bold** and *italic* and `code` run.'} />
    );
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect(container.querySelector('em')?.textContent).toBe('italic');
    expect(container.querySelector('code')?.textContent).toBe('code');
  });

  it('renders plain paragraphs', () => {
    render(<MarkdownMessage content={'just a normal sentence'} />);
    expect(screen.getByText('just a normal sentence')).toBeInTheDocument();
  });

  it('renders a [label](url) link as plain label text when no handler is given (no raw href)', () => {
    const { container } = render(
      <MarkdownMessage content={'see ([#225](https://example.com/issues/225)) for details'} />
    );
    // Label is shown, the URL is not, and no navigable anchor is emitted.
    expect(screen.getByText('#225')).toBeInTheDocument();
    expect(container.querySelector('a[href]')).toBeNull();
    expect(container.textContent).not.toContain('https://example.com');
  });

  it('calls onLinkClick with the URL when a link is activated (changelog/external open)', () => {
    const onLinkClick = vi.fn();
    render(
      <MarkdownMessage
        content={'## [0.49.0](https://example.com/compare/v0.48.0...v0.49.0) (2026-06-02)'}
        onLinkClick={onLinkClick}
      />
    );
    // A real anchor with its destination in `href` — that is what makes it a
    // link to a screen reader AND what lets it wrap inline with the prose.
    const link = screen.getByRole('link', { name: '0.49.0' });
    expect(link).toHaveAttribute('href', 'https://example.com/compare/v0.48.0...v0.49.0');
    fireEvent.click(link);
    expect(onLinkClick).toHaveBeenCalledWith('https://example.com/compare/v0.48.0...v0.49.0');
    // Surrounding heading text is preserved alongside the link.
    expect(screen.getByText(/2026-06-02/)).toBeInTheDocument();
  });

  it('never navigates the webview itself — the click default is prevented', () => {
    render(
      <MarkdownMessage
        content={'see [#225](https://example.com/issues/225) for details'}
        onLinkClick={vi.fn()}
      />
    );
    const link = screen.getByRole('link', { name: '#225' });
    // The host opens the URL; the anchor must not also follow it, or the whole
    // app would be replaced by the page inside the Tauri webview.
    const event = createEvent.click(link, { bubbles: true, cancelable: true });
    fireEvent(link, event);
    expect(event.defaultPrevented).toBe(true);
  });

  it('refuses a non-http(s) scheme: it renders as label text, never as an href', () => {
    const onLinkClick = vi.fn();
    const { container } = render(
      <MarkdownMessage
        content={'click [here](javascript:alert(1)) now'}
        onLinkClick={onLinkClick}
      />
    );
    // `preventDefault` only intercepts a PRIMARY click; a middle-click fires
    // `auxclick` and would navigate. So an unvetted scheme never reaches href.
    expect(container.querySelector('a')).toBeNull();
    expect(screen.getByText('here')).toBeInTheDocument();
  });

  it('renders an inline link inside the same <p> as the surrounding prose', () => {
    const { container } = render(
      <MarkdownMessage
        content={'see [#225](https://example.com/issues/225) for details'}
        onLinkClick={vi.fn()}
      />
    );
    // A `Button` computes to inline-block and broke the line box around it; an
    // anchor is inline, so it wraps with the sentence it sits in.
    const paragraph = container.querySelector('p');
    expect(paragraph?.querySelector('a')).not.toBeNull();
    expect(paragraph?.textContent).toBe('see #225 for details');
  });
});

describe('MarkdownMessage — link keyboard operability', () => {
  it('tab reaches the link and Enter activates it (a real control, not a fake anchor)', async () => {
    const onLinkClick = vi.fn();
    render(
      <MarkdownMessage
        content={'see [#225](https://example.com/issues/225) for details'}
        onLinkClick={onLinkClick}
      />
    );
    // A real `href` is what puts the anchor in the tab order natively — no
    // hand-rolled tabIndex + Enter/Space handler, which is what an anchor
    // WITHOUT an href would have needed.
    await userEvent.tab();
    const link = screen.getByRole('link', { name: '#225' });
    expect(link).toHaveFocus();
    await userEvent.keyboard('{Enter}');
    expect(onLinkClick).toHaveBeenCalledWith('https://example.com/issues/225');
  });
});
