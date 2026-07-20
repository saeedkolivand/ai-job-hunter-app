// A page's verbatim <style> block (extracted from the original HTML). Each
// landing route is its own document with globally-scoped element selectors
// (`body{}`, `h1{}`, `a{}`) that differ per page, so the CSS is inlined per
// route rather than imported globally (which would collide across routes).
export function PageStyle({ css }: { css: string }) {
  return <style dangerouslySetInnerHTML={{ __html: css }} />;
}
