// Injects a page's verbatim body markup. `display:contents` makes the wrapper
// generate no box, so the injected children behave as direct children of
// <body> — preserving layouts that depend on it (how-it-works' body grid, the
// centered `.wrap` pages). The content is first-party, build-time-inlined static
// HTML (no user input), so dangerouslySetInnerHTML carries no injection surface.
export function RawHtml({ html }: { html: string }) {
  return <div style={{ display: 'contents' }} dangerouslySetInnerHTML={{ __html: html }} />;
}
