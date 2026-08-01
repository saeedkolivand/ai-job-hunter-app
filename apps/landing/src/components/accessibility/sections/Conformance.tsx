// "Conformance status" + "Known barriers" blocks of /accessibility, split out
// of AccessibilityBody.tsx purely for file size. No props.
export function Conformance() {
  return (
    <>
      {/* Conformance status */}
      <h2 id="conformance">
        Conformance status{' '}
        <a className="anchor" href="#conformance" aria-label="Link to Conformance status">
          #
        </a>
      </h2>
      <div className="card">
        <span className="label">Landing site</span>
        <p style={{ marginTop: '0' }}>
          <b>Partially conformant</b> with WCAG 2.2 AA — most pages pass; the exceptions are listed
          below.
        </p>
      </div>
      <div className="card">
        <span className="label">Desktop app</span>
        <p style={{ marginTop: '0' }}>
          <b>Partially conformant</b> with WCAG 2.2 AA — the known issues are listed below;
          everything else is unverified beyond that.
        </p>
      </div>
      <div className="card">
        <span className="label">Browser extension</span>
        <p style={{ marginTop: '0' }}>
          <b>Not assessed.</b> It injects a small import button and an autofill panel into
          third-party job-board pages; we haven't run an automated or manual accessibility scan
          against it. Treat it as unverified, not as passing.
        </p>
      </div>

      {/* Known barriers */}
      <h2 id="barriers">
        Known barriers{' '}
        <a className="anchor" href="#barriers" aria-label="Link to Known barriers">
          #
        </a>
      </h2>
      <p>
        Found by an automated axe-core scan (WCAG 2a/2aa/21a/21aa/22aa rules), a scripted keyboard
        tab-through of every built landing page, and a code review of the architecture map's
        keyboard handling, all on 1 August 2026. This is what we know is broken — not a claim that
        nothing else is.
      </p>
      <div className="card">
        <span className="label">Landing site</span>
        <ul>
          <li>
            <code>/creature</code> — 1 serious <b>colour-contrast</b> violation, 4 elements (
            <code>#d-todo</code>, <code>.strike</code>, <code>#d-page</code>, <code>.thint</code>).
            It's a decorative short-film page; no information lives only there.
          </li>
          <li>
            <code>/world</code> — 1 serious <b>colour-contrast</b> violation, 1 element (
            <code>.sw-nav__item.is-active</code>). Also decorative — a scroll-driven diorama with no
            accessible equivalent, and no information that lives only there.
          </li>
          <li>
            <code>/architecture-map</code> — 1 serious <b>scrollable-region-focusable</b> violation:
            the <code>#side</code> sidebar's scroll container isn't reachable by keyboard.
            Everything else on that page is — arrow keys pan, Shift moves faster, <code>+</code>/
            <code>-</code> zoom, <code>0</code>/<code>f</code> fit, <code>?</code> opens a help
            overlay with a focus trap, Escape closes it.
          </li>
          <li>
            <code>/</code>, <code>/download</code>, <code>/how-it-works</code>,{' '}
            <code>/privacy</code>, and <code>/agent-system</code> — no violations found by the scan.
          </li>
        </ul>
      </div>
      <div className="card">
        <span className="label">Desktop app — home view</span>
        <ul>
          <li>
            <b>aria-allowed-attr</b> — critical, 1 element.
          </li>
          <li>
            <b>color-contrast</b> — serious, 11 elements.
          </li>
        </ul>
      </div>
      <div className="card">
        <span className="label">Desktop app — exported PDFs</span>
        <p style={{ marginTop: '0' }}>
          Exported résumé and cover-letter PDFs carry document metadata (title, author, language)
          but are <b>not fully tagged to PDF/UA-1</b> — proper tagging (reading order, semantic
          structure for a screen reader) is a stated future goal, not something shipped today. Don't
          assume a screen reader gets a clean read of an exported PDF; the underlying content stays
          available and editable inside the app's own résumé and cover-letter editor, independent of
          how any one export renders.
        </p>
      </div>
      <div className="flag">
        Automated tooling like axe-core catches roughly a third of WCAG issues — the rest, including
        most keyboard-trap and cognitive/language problems, need a person to find. We haven't run a
        third-party audit, and we haven't tested with assistive-technology users. Treat this list as
        a floor, not a ceiling.
      </div>
    </>
  );
}
