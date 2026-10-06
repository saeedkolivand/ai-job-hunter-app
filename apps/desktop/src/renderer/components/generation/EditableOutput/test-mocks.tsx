/**
 * Mock factories + spies for the EditableOutput tests. `vi.mock` is hoisted per
 * test file, so each file wires these in with
 * `vi.mock('<pkg>', async (importOriginal) => (await import('./test-mocks')).<factory>(await importOriginal()))`.
 *
 * The @ajh/ui mock:
 *   - replaces RichTextEditor with a test-double (see below)
 *   - stubs useFocusTrap (needed by every EditableOutput test)
 *   - forwards all other exports unchanged
 *
 * Test-double contract:
 *   (a) renders [data-testid="rich-text-editor"] so tests can assert tab wiring
 *   (b) exposes a [data-testid="rte-select-trigger"] button that fires
 *       onSelectionChange(true) — simulates the user highlighting text
 *   (c) wires the ref to the spy functions below
 */
import { forwardRef, useImperativeHandle } from 'react';
import { type Mock, vi } from 'vitest';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';
import type { RichTextEditorHandle, RichTextEditorProps } from '@ajh/ui';

import type * as Generate from '@/lib/generate';

export const mockReplaceSelection = vi.fn<(text: string) => void>();
export const mockGetSelectionText = vi.fn<() => string>(() => '');
export const mockGetSelectionContext = vi.fn<
  () => { selection: string; before: string; after: string }
>(() => ({ selection: '', before: '', after: '' }));
export const mockEditorFocus = vi.fn<() => void>();

/** rewriteSelection is the only async side-effect we need to control. */
export const mockRewriteSelection: Mock = vi.fn();

export function uiMock(actual: typeof AjhUi): Record<string, unknown> {
  const RichTextEditorDouble = forwardRef<RichTextEditorHandle, RichTextEditorProps>(
    function RichTextEditorDouble({ onSelectionChange, value }, ref) {
      useImperativeHandle(ref, (): RichTextEditorHandle => ({
        getSelectionText: mockGetSelectionText,
        getSelectionContext: mockGetSelectionContext,
        replaceSelection: mockReplaceSelection,
        focus: mockEditorFocus,
      }));

      return (
        <div data-testid={TEST_IDS.generation.richTextEditor}>
          <span data-testid={TEST_IDS.generation.rteValue}>{value}</span>
          <actual.Button
            data-testid={TEST_IDS.generation.rteSelectTrigger}
            onClick={() => onSelectionChange?.(true)}
          >
            simulate selection
          </actual.Button>
          <actual.Button
            data-testid={TEST_IDS.generation.rteDeselectTrigger}
            onClick={() => onSelectionChange?.(false)}
          >
            deselect
          </actual.Button>
        </div>
      );
    }
  );

  return {
    ...actual,
    useFocusTrap: () => ({ current: null }),
    RichTextEditor: RichTextEditorDouble,
  };
}

export function generateMock(actual: typeof Generate): Record<string, unknown> {
  return {
    ...actual,
    rewriteSelection: (...args: Parameters<typeof mockRewriteSelection>) =>
      mockRewriteSelection(...args),
  };
}
