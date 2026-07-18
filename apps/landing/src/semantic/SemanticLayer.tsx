// The Semantic layer: the full story as crawlable semantic HTML. It is the
// scroll-height authority and always in the DOM -- fully visible in fallback,
// visibility:hidden + inert (NEVER display:none) when GL is live. This is a
// SERVER component (no 'use client'); it is prerendered into the static export
// so SEO + machine-readable copy exist with zero JS. The client <Experience>
// receives it as a prop and toggles its visibility.
//
// ASCII-only source (Turbopack sourcemap rule): separators are plain ASCII.

import { CREDITS, MENU_LINKS, SCENES, SITE, type StoryLink } from "@/content/story";

import { JsonLd } from "./JsonLd";

function LinkAnchor({ link }: { link: StoryLink }) {
  if (link.external) {
    return (
      <a href={link.href} target="_blank" rel="noopener noreferrer">
        {link.label}
      </a>
    );
  }
  return <a href={link.href}>{link.label}</a>;
}

export function SemanticLayer() {
  return (
    <div id="semantic-layer" className="semantic-layer">
      <JsonLd />

      <header className="semantic-head">
        <h1>
          {SITE.name} -- {SITE.tagline}
        </h1>
        <p>{SITE.description}</p>
        <nav aria-label="Menu">
          <ul>
            {MENU_LINKS.map((l) => (
              <li key={l.href}>
                <LinkAnchor link={l} />
              </li>
            ))}
          </ul>
        </nav>
      </header>

      <main id="story-content" className="semantic-main">
        {SCENES.map((s) => (
          <section key={s.id} id={s.id} aria-labelledby={`${s.id}-heading`}>
            <p className="act-label">
              {s.act} - {s.timecode}
            </p>
            <h2 id={`${s.id}-heading`}>{s.heading}</h2>
            {s.copy.map((para, i) => (
              <p key={i}>{para}</p>
            ))}
            {s.links.length > 0 && (
              <ul className="scene-links">
                {s.links.map((l) => (
                  <li key={l.href + l.label}>
                    <LinkAnchor link={l} />
                  </li>
                ))}
              </ul>
            )}
          </section>
        ))}

        <footer id="credits" className="semantic-credits" aria-labelledby="credits-heading">
          <h2 id="credits-heading">{CREDITS.tagline}</h2>
          <p>{CREDITS.honest}</p>
          <p>{CREDITS.privacy}</p>

          <h3>What it actually does</h3>
          <ul className="feature-list">
            {CREDITS.features.map((f) => (
              <li key={f.title}>
                <strong>{f.title}</strong> -- {f.body}
              </li>
            ))}
          </ul>
          <ul className="extension-links">
            {CREDITS.extensionLinks.map((l) => (
              <li key={l.href}>
                <LinkAnchor link={l} />
              </li>
            ))}
          </ul>

          <ul className="link-roll">
            {CREDITS.links.map((l) => (
              <li key={l.href + l.label}>
                <LinkAnchor link={l} />
              </li>
            ))}
          </ul>

          <p className="mac-note">{CREDITS.macNote}</p>
          <p className="built-with">{CREDITS.builtWith}</p>
          <p className="byline">{CREDITS.byline}</p>

          <nav aria-label="Footer">
            <ul>
              <li>home</li>
              {CREDITS.footNav.map((l) => (
                <li key={l.href + l.label}>
                  <LinkAnchor link={l} />
                </li>
              ))}
            </ul>
          </nav>
        </footer>
      </main>
    </div>
  );
}
