// "What's in place" + "How this was assessed" blocks of /accessibility, split
// out of AccessibilityBody.tsx purely for file size. No props.
export function InPlace() {
  return (
    <>
      {/* What's in place */}
      <h2 id="in-place">
        What's in place{' '}
        <a className="anchor" href="#in-place" aria-label="Link to What's in place">
          #
        </a>
      </h2>
      <ul>
        <li>
          <b>Reduced motion is honoured on every page.</b> 9 landing stylesheets carry their own{' '}
          <code>prefers-reduced-motion: reduce</code> block, as do the two scripted experiences (
          <code>app/world/scrub-engine.js</code> and <code>components/agent-system/hooks.ts</code>
          ). One more file animates without carrying its own block —{' '}
          <code>how-it-works-shell.css</code>'s back-link transition — but it's always loaded on{' '}
          <code>/how-it-works</code> alongside <code>how-it-works.css</code>, whose block covers the
          whole page. The remaining stylesheets are token and base files with no animation in them.
        </li>
        <li>
          <b>Visible focus rings.</b> Every interactive element gets a <code>:focus-visible</code>{' '}
          outline, defined independently in each page's own stylesheet — this page reuses{' '}
          <code>privacy.css</code>, and shares the <code>marketing-base.css</code> footer baseline
          with <a href="/privacy">/privacy</a> and <a href="/download">/download</a>; the other
          pages define theirs separately. Ink-on-paper contrast is documented at ~12.6:1, well above
          the WCAG non-text-contrast floor of 3:1.
        </li>
        <li>
          <b>Link colour</b> (<code>--red</code>, <code>#b02b2a</code>) is documented at 5.25:1 on
          the page background — AA for 13px text.
        </li>
        <li>
          <b>Semantic markup</b> — headings and lists, not styled divs pretending to be them.
        </li>
        <li>
          <b>The architecture map has real keyboard support</b> — arrow-key panning, zoom, fit, a
          help overlay with a focus trap, and (fixed 1 August 2026) a keyboard-focusable sidebar
          that scrolls on its own arrow keys instead of panning the map underneath it.
        </li>
        <li>
          <b>A permanent, blocking accessibility gate for the landing site.</b>{' '}
          <code>apps/landing/scripts/check-a11y.mjs</code> re-runs the same axe-core WCAG A/AA scan
          on every landing pull request in CI and <b>fails the build</b> on any violation — unlike
          the three desktop checks below, which stay advisory. It auto-discovers every built route
          instead of scanning a hand-maintained list, currently covering 12 routes (all clean) plus
          one page it deliberately excludes with a stated reason (see Known barriers above).
        </li>
      </ul>
      <p>
        Three checks exist in CI for the desktop app, and all three are{' '}
        <b>advisory, not blocking</b> today: an <code>eslint.a11y.config.mjs</code> jsx-a11y lint
        pass runs warn-only, kept separate from the strict lint that does block; an axe scan in{' '}
        <code>apps/desktop/e2e/a11y.spec.ts</code> logs findings instead of failing the build; and{' '}
        <code>.lighthouserc.json</code> sets an accessibility floor of 0.9 as a warning. None of the
        three currently gate a merge.
      </p>

      {/* How this was assessed */}
      <h2 id="assessment">
        How this was assessed{' '}
        <a className="anchor" href="#assessment" aria-label="Link to How this was assessed">
          #
        </a>
      </h2>
      <p>
        Self-assessed on 1 August 2026, in two passes. First, an automated axe-core scan of the
        built landing site and the desktop renderer's home view, a scripted keyboard tab-through of
        every built landing page (a visible focus indicator on every stop it reached and no keyboard
        trap — it did not reach every focusable element on every page, most likely elements sitting
        in inactive tab/scene panels on <code>/how-it-works</code> and <code>/creature</code>, but
        that wasn't confirmed), and a code review of the architecture map's keyboard handling. Then,
        after fixing what that pass found, a second automated re-scan of the same surfaces —
        independently re-verified against the fix code rather than taken on trust, and run against
        the desktop app in both the light and dark colour schemes — plus a manual browser check that
        the architecture map's sidebar actually receives keyboard focus and scrolls. There has been{' '}
        <b>no third-party audit</b> and <b>no testing with assistive-technology users</b>. We'll say
        so again if either of those changes.
      </p>
    </>
  );
}
