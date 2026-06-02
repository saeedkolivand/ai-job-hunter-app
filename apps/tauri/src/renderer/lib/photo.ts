/**
 * Client-side photo preprocessing for the contact-profile header image.
 *
 * Decodes a user-picked image, centre-crops it to a small square thumbnail, and
 * re-encodes it to a JPEG `data:` URL. Re-encoding through a canvas drops every
 * piece of metadata (EXIF orientation, GPS, camera make) — only the pixels
 * survive — and the size caps below keep the persisted profile small. The Rust
 * side (`resolve_photo`) re-validates and re-encodes again before it ever reaches
 * the renderer, so this is the UX layer, not the security boundary.
 */

/** Source MIME types we accept (mirrors the Rust `resolve_photo` allowlist). */
const ACCEPTED_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];
/** Reject obviously-oversized source files before we attempt to decode them. */
const MAX_SOURCE_BYTES = 10 * 1024 * 1024; // 10 MB
/** Refuse source images larger than this per side (decode-bomb guard). */
const MAX_SOURCE_EDGE = 8192;
/** Output side length in pixels — a header thumbnail never needs more. */
const OUTPUT_EDGE = 512;
/** JPEG quality for the re-encoded thumbnail. */
const OUTPUT_QUALITY = 0.85;

/** Why {@link processPhotoFile} rejected an image — maps to a localized message. */
export type PhotoErrorKind = 'type' | 'size' | 'decode';

export class PhotoProcessingError extends Error {
  constructor(readonly kind: PhotoErrorKind) {
    super(kind);
    this.name = 'PhotoProcessingError';
  }
}

/**
 * Validate, centre-crop, downscale, and EXIF-strip a picked image file into a
 * bounded `data:image/jpeg;base64,…` URL suitable for `ContactProfile.photo`.
 * Throws a {@link PhotoProcessingError} the caller maps to a localized message.
 */
export async function processPhotoFile(file: File): Promise<string> {
  if (!ACCEPTED_TYPES.includes(file.type)) throw new PhotoProcessingError('type');
  if (file.size > MAX_SOURCE_BYTES) throw new PhotoProcessingError('size');

  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new PhotoProcessingError('decode');
  }

  try {
    if (bitmap.width > MAX_SOURCE_EDGE || bitmap.height > MAX_SOURCE_EDGE) {
      throw new PhotoProcessingError('size');
    }

    // Centre-crop to a square, then downscale to the output edge. A square reads
    // well in both the circular (Portrait) and boxed (Lebenslauf) photo frames.
    const side = Math.min(bitmap.width, bitmap.height);
    const sx = (bitmap.width - side) / 2;
    const sy = (bitmap.height - side) / 2;
    const out = Math.min(OUTPUT_EDGE, side);

    const canvas = document.createElement('canvas');
    canvas.width = out;
    canvas.height = out;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new PhotoProcessingError('decode');
    // Flatten any transparency onto white so JPEG never renders alpha as black.
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0, 0, out, out);
    ctx.drawImage(bitmap, sx, sy, side, side, 0, 0, out, out);

    const dataUrl = canvas.toDataURL('image/jpeg', OUTPUT_QUALITY);
    if (!dataUrl.startsWith('data:image/jpeg')) throw new PhotoProcessingError('decode');
    return dataUrl;
  } finally {
    bitmap.close();
  }
}
