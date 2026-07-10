import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import type * as Generate from '@/lib/generate';

import { PdfPreview } from './index';

// Control the real-SVG render (the only side-effect) and keep the rest of the
// generate barrel intact (types, etc.).
const mockRender = vi.fn();
vi.mock('@/lib/generate', async (importOriginal) => {
  const actual = await importOriginal<typeof Generate>();
  return { ...actual, renderDocumentPreview: (...args: unknown[]) => mockRender(...args) };
});

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// jsdom doesn't implement URL.createObjectURL / revokeObjectURL.
// Mock them: createObjectURL returns a predictable blob: URL; revokeObjectURL is a spy.
let blobCounter = 0;
const revokeObjectURL = vi.fn();
beforeEach(() => {
  blobCounter = 0;
  revokeObjectURL.mockReset();
  Object.defineProperty(URL, 'createObjectURL', {
    configurable: true,
    value: (_blob: Blob) => `blob:mock/${++blobCounter}`,
  });
  Object.defineProperty(URL, 'revokeObjectURL', {
    configurable: true,
    value: revokeObjectURL,
  });
});

const PROPS = { docType: 'resume' as const, templateId: 'classic' as const };

beforeEach(() => {
  vi.useFakeTimers();
  mockRender.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('PdfPreview (#24)', () => {
  it('renders SVG pages as <img> elements with blob URLs after the debounce settles', async () => {
    mockRender.mockResolvedValue(['<svg>page1</svg>', '<svg>page2</svg>']);
    render(<PdfPreview text="A real resume body" {...PROPS} />);

    // Before the debounce elapses nothing is rendered yet.
    expect(mockRender).not.toHaveBeenCalled();
    expect(screen.queryByRole('img')).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    expect(mockRender).toHaveBeenCalledTimes(1);
    const imgs = screen.getAllByRole('img');
    expect(imgs).toHaveLength(2);
    // Blob URLs from the mock: first two createObjectURL calls in this test.
    expect(imgs[0]).toHaveAttribute('src', 'blob:mock/1');
    expect(imgs[1]).toHaveAttribute('src', 'blob:mock/2');
    // Descriptive alt text
    expect(imgs[0]).toHaveAttribute('alt', 'aiGenerate.pdfPreview.title — page 1');
    expect(imgs[1]).toHaveAttribute('alt', 'aiGenerate.pdfPreview.title — page 2');
    // No iframe
    expect(screen.queryByRole('iframe')).toBeNull();
  });

  it('does not render while paused (still generating)', async () => {
    render(<PdfPreview text="A real resume body" paused {...PROPS} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(mockRender).not.toHaveBeenCalled();
    expect(screen.getByText('aiGenerate.pdfPreview.empty')).toBeInTheDocument();
  });

  it('does not render for empty/whitespace text', async () => {
    render(<PdfPreview text="   " {...PROPS} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(mockRender).not.toHaveBeenCalled();
  });

  it('shows an error state when the render fails', async () => {
    mockRender.mockRejectedValue(new Error('render boom'));
    render(<PdfPreview text="A real resume body" {...PROPS} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(screen.getByText('aiGenerate.pdfPreview.failed')).toBeInTheDocument();
    expect(screen.queryByRole('img')).toBeNull();
  });

  it('debounces rapid edits into a single render of the latest text', async () => {
    mockRender.mockResolvedValue(['<svg>page</svg>']);
    const { rerender } = render(<PdfPreview text="v1" {...PROPS} />);
    rerender(<PdfPreview text="v2" {...PROPS} />);
    rerender(<PdfPreview text="v3" {...PROPS} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    expect(mockRender).toHaveBeenCalledTimes(1);
    expect(mockRender).toHaveBeenCalledWith(
      'v3',
      'resume',
      undefined,
      'classic',
      false,
      undefined,
      undefined,
      undefined
    );
  });

  it('revokes old blob URLs when a new render batch resolves', async () => {
    mockRender.mockResolvedValue(['<svg>page</svg>']);
    const { rerender } = render(<PdfPreview text="v1" {...PROPS} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    // First batch: blob:mock/1 created, nothing revoked yet.
    expect(revokeObjectURL).not.toHaveBeenCalled();

    // Trigger a second render.
    rerender(<PdfPreview text="v2" {...PROPS} />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    // Old URL revoked before new batch set.
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:mock/1');
    // New img uses the next blob URL.
    const imgs = screen.getAllByRole('img');
    expect(imgs[0]).toHaveAttribute('src', 'blob:mock/2');
  });

  it('revokes all blob URLs on unmount', async () => {
    mockRender.mockResolvedValue(['<svg>p1</svg>', '<svg>p2</svg>']);
    const { unmount } = render(<PdfPreview text="A resume" {...PROPS} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    unmount();
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:mock/1');
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:mock/2');
  });

  it('clears stale pages immediately on doc-switch and renders without the 500ms debounce', async () => {
    // First render: résumé settles after the debounce.
    mockRender.mockResolvedValue(['<svg>resume-page</svg>']);
    const { rerender } = render(<PdfPreview text="resume body" {...PROPS} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    // Résumé pages are visible.
    expect(screen.getAllByRole('img')).toHaveLength(1);

    // Switch doc type: cover-letter replaces résumé.
    mockRender.mockResolvedValue(['<svg>letter-page</svg>']);
    rerender(<PdfPreview text="cover letter body" docType="cover-letter" templateId="classic" />);

    // Pages must be cleared synchronously — before any timer advances.
    // No <img> elements should be present; the loader is shown instead.
    expect(screen.queryByRole('img')).toBeNull();

    // The old blob URL was revoked immediately on the switch.
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:mock/1');

    // Doc-switch uses delay=0, so advancing by 0ms (flushing microtasks) is
    // enough — the full 500ms debounce must NOT be required.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    // New cover-letter page renders without waiting the full debounce period.
    const imgs = screen.getAllByRole('img');
    expect(imgs).toHaveLength(1);
    expect(imgs[0]).toHaveAttribute('alt', 'aiGenerate.pdfPreview.title — page 1');
    // mockRender was called twice total: once for résumé, once for cover-letter.
    expect(mockRender).toHaveBeenCalledTimes(2);
    expect(mockRender).toHaveBeenLastCalledWith(
      'cover letter body',
      'cover-letter',
      undefined,
      'classic',
      false,
      undefined,
      undefined,
      undefined
    );
  });
});
