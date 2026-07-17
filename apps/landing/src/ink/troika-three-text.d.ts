// troika-three-text ships no type declarations (drei bundles its own runtime
// use of Text/preloadFont). We only import preloadFont, to warm atlases before
// the GL Experience mounts (see ink/text.ts preloadAllFonts) -- declare just
// that surface.
declare module "troika-three-text" {
  export function preloadFont(
    options: { font?: string; characters?: string | string[]; sdfGlyphSize?: number },
    callback: (result: unknown) => void,
  ): void;
}
