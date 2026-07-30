// "Browser extension" block of /privacy, split out of PrivacyBody.tsx
// purely for file size — verbatim ported markup, no props. See
// PrivacyBody.tsx for the shared conversion notes. Nothing scripts this page
// (privacy-0.js is a console easter egg), but scripts/check-parity.mjs pins
// its copy and anchor hrefs (ADR 0018).
export function Extension() {
  return (
    <>
      {/* Browser extension */}
      <h2 id="extension">
        Browser extension{' '}
        <a className="anchor" href="#extension" aria-label="Link to Browser extension">
          #
        </a>
      </h2>
      <p>
        The <b>AI Job Hunter — Job Importer</b> extension (Chrome and Firefox) exists to do one
        thing: take the job posting you're looking at and hand it to the desktop app running on the
        same machine. It is inert unless that app is running and you've paired it.
      </p>

      <div className="card">
        <span className="label">What it sends — and where</span>
        <ul>
          <li>
            <b>Loopback only.</b> The extension connects to your own desktop app over native
            messaging (preferred) or a loopback WebSocket at{' '}
            <code>
              ws://127.0.0.1:{'<'}port{'>'}
            </code>{' '}
            (fallback). It has <b>no remote backend</b> and contacts <b>no third-party server</b>.
            Its only host permission is <code>127.0.0.1</code> (loopback); it grants no access to
            any public or LAN address.
          </li>
          <li>
            <b>Import this job</b> sends the current tab's URL to the local app. The extension also
            captures the page's rendered DOM when possible (for logged-in boards that a headless
            server-side fetch cannot reach), and sends that HTML only to the local app.{' '}
            <b>Nothing is captured in the background or on page load.</b> Capture happens only when
            you click Import.
          </li>
          <li>
            <b>Fill this form (assisted autofill: opt-in, off by default).</b> If you turn on{' '}
            <i>Assisted form autofill</i> in the desktop app (
            <i>Settings → Accounts → Browser extension</i>), clicking <b>Fill this form</b> asks the
            desktop for your saved contact details (name, email, phone, location,
            LinkedIn/GitHub/website) over the same loopback connection and fills matching{' '}
            <b>empty</b> fields on the current page, then shows an in-page summary. Your details are
            your <b>own data</b>, come from your <b>own paired desktop</b>, are used only for that
            one fill, are <b>never stored in the browser</b>, and{' '}
            <b>never leave your computer except into the page you chose to fill</b>. It{' '}
            <b>never submits the form for you</b>. When the toggle is off, the desktop declines the
            request. This is why the extension collects no data and its Firefox data-collection
            declaration is <code>["none"]</code>.
          </li>
        </ul>
      </div>

      <div className="card">
        <span className="label">Permissions, and why each is there</span>
        <ul>
          <li>
            <code>activeTab</code> — read the URL and the DOM of the tab you clicked,{' '}
            <b>only on that click</b>. No standing access to any site.
          </li>
          <li>
            <code>storage</code> — store the pairing token locally so you only pair once.
          </li>
          <li>
            <code>scripting</code> — MV3 requires this to inject the DOM capture into the active tab
            on import click; its reach stays limited to the active tab.
          </li>
          <li>
            <code>nativeMessaging</code> — connect to the AI Job Hunter desktop host (
            <code>app.aijobhunter.bridge</code>) using the browser's native-messaging channel. This
            is the primary transport to the local app and is immune to Firefox HTTPS-Only Mode
            silently upgrading <code>ws://</code> connections. Falls back to the loopback WebSocket
            if the native host is not registered.
          </li>
        </ul>
        <p className="note">
          No broad host access (
          <code>
            {'<'}all_urls{'>'}
          </code>
          ), no <code>tabs</code> permission, no <code>webRequest</code>, no remotely-hosted code,
          no <code>eval</code>. Everything is bundled at build time.
        </p>
      </div>

      <div className="card">
        <span className="label">What it stores</span>
        <p style={{ marginTop: '0' }}>
          The only value the extension persists is the <b>pairing token</b> — a one-time secret you
          copy from the app's Settings — kept in <code>chrome.storage.local</code>. It is used
          solely to authenticate to your local desktop app and is never sent to any remote server.{' '}
          <b>No telemetry, no analytics, no external API.</b>
        </p>
      </div>
    </>
  );
}
