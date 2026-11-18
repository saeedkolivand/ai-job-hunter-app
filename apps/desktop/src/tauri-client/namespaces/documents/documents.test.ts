import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { BaseExportRequest } from '@ajh/shared/ipc';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { documents } from './documents';

beforeEach(() => {
  invoke.mockReset().mockResolvedValue('/out/cv.pdf');
});

const request = { text: 'cv', format: 'pdf' } as BaseExportRequest;

describe('documents.exportAndSave', () => {
  // The Rust command binds `generationId`; any other key deserializes to None and
  // silently records no template.
  it('sends the generation id under the `generationId` invoke key', async () => {
    await expect(documents.exportAndSave(request, 'gen-1')).resolves.toBe('/out/cv.pdf');
    expect(invoke).toHaveBeenCalledWith('documents_export_and_save', {
      request,
      generationId: 'gen-1',
    });
    expect(Object.keys(invoke.mock.calls[0]?.[1] as object)).toEqual(['request', 'generationId']);
  });

  it('leaves the id undefined when none is given', async () => {
    await documents.exportAndSave(request);
    expect((invoke.mock.calls[0]?.[1] as { generationId?: string }).generationId).toBeUndefined();
  });
});
