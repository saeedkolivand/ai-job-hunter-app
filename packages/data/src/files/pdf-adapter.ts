/**
 * Adapter for pdf-parse to handle ESM import issues with TypeScript
 */

let pdfModule: any = null;

async function getPdfModule() {
  if (!pdfModule) {
    pdfModule = await import('pdf-parse');
  }
  return pdfModule;
}

export async function parsePdf(buffer: Buffer): Promise<{ numpages: number; text: string }> {
  const module = await getPdfModule();
  // pdf-parse 2.x exports PDFParse class - use getText() method
  const { PDFParse } = module;
  const parser = new PDFParse({ data: buffer });
  const result = await parser.getText();
  return {
    numpages: result.numpages,
    text: result.text,
  };
}
