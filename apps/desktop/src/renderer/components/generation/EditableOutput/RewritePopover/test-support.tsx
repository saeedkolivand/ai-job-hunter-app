import { vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import { rewriteSelection } from '@/lib/generate';

import { RewritePopover } from '../RewritePopover';

export const SELECTION = 'some selected text';

export function renderPopover(props: Partial<React.ComponentProps<typeof RewritePopover>> = {}) {
  return render(
    <RewritePopover
      target={{ selection: SELECTION, before: '', after: '' }}
      docType="resume"
      model="test-model"
      onAccept={vi.fn()}
      onClose={vi.fn()}
      {...props}
    />
  );
}

/** The Accept button, whose enabled state is the honesty contract under test. */
export function acceptButton(): HTMLButtonElement {
  return screen.getByRole('button', { name: /aiGenerate\.rewrite\.accept/i });
}

/** Flush `n` microtask turns inside `act`. */
export async function flush(n = 4) {
  await act(async () => {
    for (let i = 0; i < n; i++) await Promise.resolve();
  });
}

/** Run the free-instruction path with `text`, then flush the promise chain. */
export async function runInstruction(text: string) {
  fireEvent.change(screen.getByPlaceholderText('aiGenerate.rewrite.instructionPlaceholder'), {
    target: { value: text },
  });
  fireEvent.click(screen.getByRole('button', { name: 'aiGenerate.rewrite.submit' }));
  await flush();
}

/**
 * Mock that stalls until its AbortSignal fires, then rejects — mirrors a
 * real provider whose connection hangs and is finally aborted by the client.
 */
export function mockStall() {
  vi.mocked(rewriteSelection).mockImplementation(
    ({ signal }: { signal?: AbortSignal }) =>
      new Promise<string>((_, reject) => {
        signal?.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')));
      })
  );
}
