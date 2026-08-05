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
          <b>Partially conformant</b> with WCAG 2.2 AA. The most recent automated scan found no
          violations, but zero violations isn't the same as full conformance — see Known barriers
          below for what's still unaudited or out of scope.
        </p>
      </div>
      <div className="card">
        <span className="label">Desktop app</span>
        <p style={{ marginTop: '0' }}>
          <b>Partially conformant</b> with WCAG 2.2 AA. The home view has no known automated
          violations after a recent fix round, but that coverage doesn't reach past the home view —
          see Known barriers below.
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
        A first automated axe-core scan (WCAG 2a/2aa/21a/21aa/22aa rules) on 1 August 2026 found the
        problems listed below, alongside how each was fixed. A second scan later the same day, after
        the fixes, found{' '}
        <b>0 violations across all 9 landing pages and the desktop app's home view</b>. That's not a
        claim of zero barriers — the rest of this section says what's still scoped out, unaudited,
        or otherwise open.
      </p>
      <p>
        Both of those were one-off, manually run passes. Since then, a permanent gate (
        <code>apps/landing/scripts/check-a11y.mjs</code>) re-runs the same axe-core scan on every
        landing pull request and <b>blocks the merge</b> on any violation — this one doesn't stop
        repeating. Its auto-discovery found three routes neither manual pass had ever scanned (
        <code>/404</code>, <code>/_not-found</code>, <code>/mission-control</code>) and, on its
        first run, caught one real barrier: see <code>/mission-control</code> below.
      </p>
      <div className="card">
        <span className="label">Landing site</span>
        <p style={{ marginTop: '0' }}>
          0 violations across the <b>13 routes</b> the permanent gate covers (<code>/</code>,{' '}
          <code>/download</code>, <code>/how-it-works</code>, <code>/privacy</code>,{' '}
          <code>/accessibility</code>, <code>/creature</code>, <code>/world</code>,{' '}
          <code>/architecture-map</code>, <code>/agent-system</code>, <code>/tech-radar</code>,{' '}
          <code>/404</code>, <code>/_not-found</code>, <code>/mission-control</code>). One more
          public page, <code>/benchmarks/</code>, is deliberately excluded from the gate — see the
          card below, not folded into this "0 violations" figure. Four barriers found across the
          manual passes and the gate's own first run have since been fixed:
        </p>
        <ul>
          <li>
            <code>/creature</code> — the four faded doodle annotations were a low-contrast grey on
            the paper background (2.46–2.8:1); they're now a warmer grey (<code>#6b6459</code>) at
            4.98:1.
          </li>
          <li>
            <code>/world</code> — the active nav pill's white-on-red text was 3.93:1; the pill
            background is now darkened with <code>color-mix</code>, landing at 8.54:1. Worth noting:
            the accent colour cycles per section, and the original scan only ever caught the red
            state. A manual check of the others found the blue state at 1.88:1 — a worse failure the
            first pass missed entirely — since fixed to 4.86:1. We're leaving this in as an honest
            example of what a single-state automated scan can miss.
          </li>
          <li>
            <code>/architecture-map</code> — the <code>#side</code> sidebar wasn't reachable by
            keyboard. It's now <code>tabindex="0"</code> with a label, and the map's arrow-key pan
            handler skips it while it has focus, so arrow keys scroll the sidebar instead of panning
            the map underneath it. Confirmed by hand in a browser: focus lands on the sidebar, and
            ArrowDown moves its scroll position.
          </li>
          <li>
            <code>/mission-control</code> — the row-title links inherited the page's plain accent
            red at 4.03:1 on the row background; scoped to the same AA-safe token{' '}
            <code>.mc-badge.is-stale</code> already used on this exact background, landing at 5.7:1.
            Found and fixed by the permanent gate's first run — this page was never in scope for the
            two manual passes above.
          </li>
        </ul>
      </div>
      <div className="card">
        <span className="label">Landing site — excluded page</span>
        <p style={{ marginTop: '0' }}>
          <code>/benchmarks/</code> is <b>excluded from the gate, not passing it</b>. It's generated
          wholesale by a third-party CI action (
          <code>benchmark-action/github-action-benchmark</code> v1.22.1, run by the{' '}
          <code>benchmark</code> job in <code>.github/workflows/quality.yml</code>) and overwritten
          on every push-to-main perf run, so a fix committed here would be silently clobbered by the
          next run — there is no way to fix it in this repository. As of the gate's first scan it
          carries two violations: <b>color-contrast</b> (the download button, white text on{' '}
          <code>#3298dc</code>, 3.15:1, needs 4.5:1) and <b>html-has-lang</b> (the page's{' '}
          <code>html</code> element has no <code>lang</code> attribute). It's a public page on this
          domain with real, disclosed barriers, not a clean bill.
        </p>
      </div>
      <div className="card">
        <span className="label">Desktop app — home view</span>
        <p style={{ marginTop: '0' }}>
          0 violations at the WCAG A or AA level, in both the light and dark colour schemes (was 1
          critical + 1 serious across 12 elements). What changed:
        </p>
        <ul>
          <li>
            The critical <b>aria-allowed-attr</b> was an <code>aria-expanded</code> attribute on a
            roleless wrapper <code>div</code> inside the shared <code>HoverPopover</code> component.
            The invalid attribute is now removed from that wrapper everywhere the component is used,
            not just here — and where the trigger is itself a real button, it moves onto that button
            instead. Two call sites wrap a non-interactive element, so they lose the attribute
            rather than relocating it; it was never reachable by assistive technology in those cases
            either way.
          </li>
          <li>
            The 11 <b>color-contrast</b> elements now use the existing{' '}
            <code>text-muted-foreground</code> token (5.1:1 on white, 4.8:1 on <code>#f8f8f8</code>)
            instead of low-alpha <code>text-foreground/30|40|55</code>.
          </li>
        </ul>
        <p>
          <b>This fix is scoped to the home view.</b> The renderer has roughly 1,100{' '}
          <code>text-foreground/NN</code> usages, around 460 of them at the low-alpha steps (
          <code>/30</code>, <code>/35</code>, <code>/40</code>, <code>/45</code>, <code>/55</code>)
          that measure below 4.5:1 for small text. Only the home view has ever been scanned; every
          other view is unaudited and likely carries the same defect. This is the single most
          important known barrier on this page.
        </p>
        <p>
          Two <code>best-practice</code> axe rules also remain here — <b>landmark-unique</b> (1
          element) and <b>region</b> (5 elements). They're tagged best-practice, not WCAG A or AA,
          so they sit outside the standard this page targets, but "0 violations" shouldn't be read
          as a clean bill without mentioning them.
        </p>
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
