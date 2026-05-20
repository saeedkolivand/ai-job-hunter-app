import mammoth from 'mammoth';

export async function extractDocx(filePath: string): Promise<{ text: string }> {
  const { value } = await mammoth.extractRawText({ path: filePath });
  return { text: value.trim() };
}

export async function extractDocxFromBytes(bytes: Uint8Array): Promise<{ text: string }> {
  const buffer = Buffer.isBuffer(bytes)
    ? bytes
    : Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const { value } = await mammoth.extractRawText({ buffer });
  return { text: value.trim() };
}
