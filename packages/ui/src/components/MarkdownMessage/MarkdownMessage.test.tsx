import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

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
    const link = screen.getByRole('link', { name: '0.49.0' });
    expect(link).not.toHaveAttribute('href'); // never navigates the webview itself
    fireEvent.click(link);
    expect(onLinkClick).toHaveBeenCalledWith('https://example.com/compare/v0.48.0...v0.49.0');
    // Surrounding heading text is preserved alongside the link.
    expect(screen.getByText(/2026-06-02/)).toBeInTheDocument();
  });
});
