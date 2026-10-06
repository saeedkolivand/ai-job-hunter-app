/**
 * Harness for the GenerationOutput tests: the `vi.mock` calls (hoisted above this module's
 * imports), mutable stub state, props builder and render helpers.
 *
 * Tests import the component ONLY from here — never from '../GenerationOutput' directly — so the
 * mocks are registered before it loads regardless of import sorting.
 */
import React, { useState } from 'react';
import { type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import type * as AjhUi from '@ajh/ui';

import { GenerationOutput as GenerationOutputImpl } from '../GenerationOutput';

// Plain binding (not `export { x }` of an import — vite-node leaves those undefined here).
export const GenerationOutput = GenerationOutputImpl;

// Echo every key verbatim — no i18next runtime needed in jsdom.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ModelSelector uses useAppClient (requires AppClientProvider) — stub so tests
// that reach the summary tab don't need a full provider tree. `useSelectedProvider`
// reads a mutable module-level var (not a plain arrow) so the CLI-agent egress
// tests below can select a CLI-agent provider; every other test relies on the
// default ('ollama' — not a CLI agent, so the score strip never discloses egress).
vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => <div data-testid="model-selector-stub" />,
  useSelectedProvider: () => stub.provider,
}));

// ExternalLink uses useAppClient (requires AppClientProvider) — stub it with a
// plain anchor so tests that reach the Job-ad source tab don't need a provider.
vi.mock('@/components/ui/ExternalLink', () => ({
  ExternalLink: ({
    href,
    children,
    ...rest
  }: { href: string; children: React.ReactNode } & React.HTMLAttributes<HTMLAnchorElement>) => (
    <a href={href} {...rest}>
      {children}
    </a>
  ),
}));

// useJobAdTextMatchScore — shared by JobAdView's Score tab AND the résumé
// result's score strip (GenerationScoreStrip), via useAppClient/QueryClient —
// stubbed so tests that reach either don't need a provider tree. A mutable
// `stubbedScore` (not a plain arrow) so the score-strip tests below can drive
// it — same pattern as JobAdView/test-support.tsx. `mockUseJobAdTextMatchScore`
// (the `mock`-prefixed name) is Vitest's documented exception to the "no
// out-of-scope refs in a hoisted factory" rule. Reset before EVERY test —
// most never touch it and rely on this default (undefined data, not
// loading), which is what makes the strip render its honest "not scored"
// placeholder rather than a stale value leaking across tests.
export const stub: {
  score: { data?: unknown; isLoading?: boolean; isError?: boolean; refetch?: () => void };
  provider: string;
} = { score: { data: undefined, isLoading: false }, provider: 'ollama' };

/** Call in `beforeEach`. */
export function resetHarness() {
  stub.score = { data: undefined, isLoading: false };
  stub.provider = 'ollama';
}

export const mockUseJobAdTextMatchScore = vi.fn((..._args: unknown[]) => stub.score);

vi.mock('@/services', () => ({
  useJobAdTextMatchScore: (...args: unknown[]) => mockUseJobAdTextMatchScore(...args),
}));

// EditableOutput mock — exposes onChange/onBlur/isPending + renders previewSlot.
// Uses divs (not raw <textarea>/<button>) to stay clear of the @ajh/ui ESLint rule.
// The mock is intentionally richer than the original so edit/debounce/preview tests
// can drive the component's committed-text logic without the real editor tree.
vi.mock('@/components/generation/EditableOutput', () => ({
  EditableOutput: ({
    value,
    onChange,
    onBlur,
    isPending,
    previewSlot,
  }: {
    value: string;
    onChange?: (v: string) => void;
    onBlur?: () => void;
    isPending?: boolean;
    previewSlot?: React.ReactNode;
  }) => (
    <div data-testid={TEST_IDS.documents.editableOutput}>
      {value}
      <div
        role="textbox"
        data-testid={TEST_IDS.documents.editableInput}
        contentEditable
        suppressContentEditableWarning
        onInput={(e) => onChange?.((e.target as HTMLElement).textContent ?? '')}
        onBlur={onBlur}
      />
      {isPending && <div data-testid={TEST_IDS.generation.pendingCommit}>updating</div>}
      {previewSlot && <div data-testid={TEST_IDS.documents.previewSlot}>{previewSlot}</div>}
    </div>
  ),
}));

// PdfPreview mock — renders its `text` and `locale` props into a testid (the
// latter as a data attribute) so tests can inspect the committed text AND the
// market/locale GenerationOutput actually forwards, without launching the real
// Typst/PDF pipeline.
vi.mock('@/components/generation/PdfPreview', () => ({
  PdfPreview: ({ text, locale }: { text: string; locale?: string }) => (
    <div data-testid={TEST_IDS.documents.pdfPreview} data-locale={locale ?? ''}>
      {text}
    </div>
  ),
}));

// Dropdown mock — preserves all other @ajh/ui exports unchanged via
// importOriginal; only Dropdown is replaced with a plain <select>-like
// div structure that drives onChange when an option div is clicked.
vi.mock('@ajh/ui', async (importOriginal) => {
  const real = await importOriginal<typeof AjhUi>();
  return {
    ...real,
    Dropdown: ({
      options,
      value,
      onChange,
      id,
    }: {
      options: Array<{ value: string; label: string }>;
      value: string;
      onChange: (v: string) => void;
      id?: string;
    }) => (
      <div data-testid={id ?? 'dropdown'} data-value={value}>
        {options.map((o) => (
          <div
            key={o.value}
            role="option"
            aria-selected={o.value === value}
            data-optvalue={o.value}
            onClick={() => onChange(o.value)}
          >
            {o.label}
          </div>
        ))}
      </div>
    ),
  };
});

// ── Default props fixture ─────────────────────────────────────────────────────

export const noop = () => undefined;

export function makeProps(overrides: Partial<Parameters<typeof GenerationOutput>[0]> = {}) {
  return {
    target: 'both' as const,
    hasResume: true,
    activeOut: 'resume' as const,
    setActiveOut: vi.fn() as Mock,
    templateId: 'classic' as const,
    atsMode: false,
    accent: undefined,
    letterLayoutId: undefined,
    onTemplateChange: vi.fn() as Mock,
    onAtsModeChange: vi.fn() as Mock,
    onAccentChange: vi.fn() as Mock,
    onLetterLayoutChange: vi.fn() as Mock,
    output: 'Generated resume content',
    onEdit: noop,
    editable: false,
    meta: null,
    copied: false,
    onCopy: noop,
    exportOpen: false,
    setExportOpen: vi.fn() as Mock,
    onExport: vi.fn() as Mock,
    jobDesc: 'Full job description text',
    onJobDescChange: vi.fn() as Mock,
    hasDesc: true,
    fetchingDesc: false,
    jobUrl: 'https://example.com/job',
    jobAdSummary: {
      summary: '',
      generating: false,
      error: null,
      generate: vi.fn() as Mock,
      language: 'en',
      setLanguage: vi.fn() as Mock,
    },
    ...overrides,
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Click the top-level "Job ad" tab (its label is the echoed i18n key). */
export async function clickJobAdTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('tab', { name: 'autopilot.apply.tabs.jobAd' }));
}

/** Click the JobAdView "Job ad" source SUB-TAB (a SegmentedControl radio). */
export async function clickSourceSubTab(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('radio', { name: 'autopilot.apply.tabs.jobAd' }));
}

/** Click a template option by its id inside the picker. */
export async function pickTemplate(user: ReturnType<typeof userEvent.setup>, templateId: string) {
  await user.click(screen.getByRole('option', { name: new RegExp(templateId, 'i') }));
}

// ── Stateful wrapper for edit/save/preview tests ───────────────────────────────
// The component is fully controlled: onEdit informs the parent, the parent must
// pass the new value back down as `output`. This wrapper simulates that round-trip.

export function ControlledWrapper(initialProps: Parameters<typeof GenerationOutput>[0]) {
  const [output, setOutput] = useState(initialProps.output);
  const handleEdit = (text: string) => {
    initialProps.onEdit(text);
    setOutput(text);
  };
  return <GenerationOutput {...initialProps} output={output} onEdit={handleEdit} />;
}

/** Renders the component with the default props plus `overrides`. */
export const renderOutput = (overrides: Parameters<typeof makeProps>[0] = {}) =>
  render(<GenerationOutput {...makeProps(overrides)} />);
