import { describe, expect, it } from 'vitest';

import { PhotoProcessingError, processPhotoFile } from './photo';

// These cover the validation guards that run *before* any canvas decode (type +
// size). The downscale/EXIF-strip path needs a real canvas + createImageBitmap,
// which the Rust `resolve_photo` suite exercises end-to-end; here we pin that bad
// input is rejected with a typed error the form can map to a localized message.
describe('processPhotoFile', () => {
  it('rejects an unsupported MIME type', async () => {
    const file = new File(['x'], 'resume.pdf', { type: 'application/pdf' });
    await expect(processPhotoFile(file)).rejects.toMatchObject({ kind: 'type' });
  });

  it('rejects an SVG (not in the raster allowlist)', async () => {
    const file = new File(['<svg/>'], 'logo.svg', { type: 'image/svg+xml' });
    await expect(processPhotoFile(file)).rejects.toMatchObject({ kind: 'type' });
  });

  it('rejects a source file over 10 MB', async () => {
    const file = new File(['x'], 'huge.png', { type: 'image/png' });
    Object.defineProperty(file, 'size', { value: 11 * 1024 * 1024 });
    await expect(processPhotoFile(file)).rejects.toMatchObject({ kind: 'size' });
  });

  it('throws a PhotoProcessingError instance', async () => {
    const file = new File(['x'], 'note.txt', { type: 'text/plain' });
    await expect(processPhotoFile(file)).rejects.toBeInstanceOf(PhotoProcessingError);
  });
});
