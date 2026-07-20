// The exact Google-Fonts <link> set each page carried in its <head>. React 19
// hoists these into <head> of the exported HTML. `gstatic` mirrors whether the
// original page preconnected to fonts.gstatic.com (creature did not).
export function GoogleFonts({ href, gstatic = true }: { href: string; gstatic?: boolean }) {
  return (
    <>
      <link rel="preconnect" href="https://fonts.googleapis.com" />
      {gstatic ? <link rel="preconnect" href="https://fonts.gstatic.com" crossOrigin="" /> : null}
      <link href={href} rel="stylesheet" />
    </>
  );
}
