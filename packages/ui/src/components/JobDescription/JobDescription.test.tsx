/**
 * JobDescription — markdown rendering + security invariants.
 *
 * Tests the REAL component (no stub). react-markdown without rehype-raw:
 *  - escapes raw HTML by default, so <script> / <img onerror> cannot reach DOM.
 *  - links map to <span class="text-brand">, NOT <a href=…>.
 *
 * noUncheckedIndexedAccess: no array indexing needed here.
 */

import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { JobDescription } from './index';

const FIXTURE = [
  '## Requirements',
  '',
  '**Excellent communication** skills required.',
  '',
  '- TypeScript',
  '- React',
  '- Rust',
  '',
  '[Apply](https://x.io/p/1)',
  '',
  '<script>alert(1)</script>',
  '',
  '<img src=x onerror=alert(1)>',
].join('\n');

describe('JobDescription — markdown elements', () => {
  it('renders an h2 markdown heading as an <h3> element (h2→h3 mapping)', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    // Component maps h2 → <h3>
    expect(container.querySelector('h3')).toBeInTheDocument();
    expect(container.querySelector('h3')?.textContent).toBe('Requirements');
  });

  it('renders **bold** as a <strong> element', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    const strong = container.querySelector('strong');
    expect(strong).toBeInTheDocument();
    expect(strong?.textContent).toBe('Excellent communication');
  });

  it('renders bullet list items as <li> elements', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    const items = container.querySelectorAll('li');
    expect(items.length).toBeGreaterThanOrEqual(3);
    const texts = Array.from(items).map((li) => li.textContent);
    expect(texts).toContain('TypeScript');
    expect(texts).toContain('React');
    expect(texts).toContain('Rust');
  });
});

describe('JobDescription — inert-link security invariant', () => {
  it('renders a markdown link as a <span> with text-brand class, NOT an <a> with href', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    // Must find a span (not anchor) containing the link text.
    expect(screen.getByText('Apply')).toBeInTheDocument();
    const span = screen.getByText('Apply');
    expect(span.tagName.toLowerCase()).toBe('span');
    expect(span.className).toContain('text-brand');
    // No live <a href> must exist in the rendered output.
    expect(container.querySelector('a[href]')).toBeNull();
  });
});

describe('JobDescription — XSS-suppression invariant', () => {
  it('does NOT produce a <script> element for raw <script>alert(1)</script> input', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    // react-markdown without rehype-raw escapes raw HTML — no script element.
    expect(container.querySelector('script')).toBeNull();
  });

  it('does NOT produce an <img> with onerror for raw <img src=x onerror=alert(1)>', () => {
    const { container } = render(<JobDescription markdown={FIXTURE} />);
    // No img element must be present; react-markdown escapes raw HTML tags.
    expect(container.querySelector('img')).toBeNull();
  });
});
