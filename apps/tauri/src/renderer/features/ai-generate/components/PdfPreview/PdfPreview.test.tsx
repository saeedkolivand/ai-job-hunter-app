import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import type * as Generate from '@/lib/generate';

import { PdfPreview } from './index';

// Control the real-PDF render (the only side-effect) and keep the rest of the
// generate barrel intact (types, etc.).
const mockRender = vi.fn();
vi.mock('@/lib/generate', async (importOriginal) => {
  const actual = await importOriginal<typeof Generate>();
  return { ...actual, renderPdfPreview: (...args: unknown[]) => mockRender(...args) };
});

vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const PROPS = { docType: 'resume' as const, templateId: 'classic' as const };

beforeEach(() => {
  vi.useFakeTimers();
  mockRender.mockReset();
  // jsdom does not implement these — provide spies the component can call.
  URL.createObjectURL = vi.fn(() => 'blob:mock-url');
  URL.revokeObjectURL = vi.fn();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('PdfPreview (#24)', () => {
  it('renders the real PDF in an iframe after the debounce settles', async () => {
    mockRender.mockResolvedValue(new Uint8Array([1, 2, 3]));
    render(<PdfPreview text="A real resume body" {...PROPS} />);

    // Before the debounce elapses nothing is rendered yet.
    expect(mockRender).not.toHaveBeenCalled();
    expect(screen.queryByTitle('aiGenerate.pdfPreview.title')).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    expect(mockRender).toHaveBeenCalledTimes(1);
    const iframe = screen.getByTitle('aiGenerate.pdfPreview.title');
    expect(iframe).toHaveAttribute('src', 'blob:mock-url');
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
    expect(screen.queryByTitle('aiGenerate.pdfPreview.title')).toBeNull();
  });

  it('debounces rapid edits into a single render of the latest text', async () => {
    mockRender.mockResolvedValue(new Uint8Array([1]));
    const { rerender } = render(<PdfPreview text="v1" {...PROPS} />);
    rerender(<PdfPreview text="v2" {...PROPS} />);
    rerender(<PdfPreview text="v3" {...PROPS} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    expect(mockRender).toHaveBeenCalledTimes(1);
    expect(mockRender).toHaveBeenCalledWith('v3', 'resume', undefined, 'classic', false, undefined);
  });
});
