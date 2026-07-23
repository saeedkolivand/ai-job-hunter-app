import { BackLink } from '@/components/BackLink';
import { SiteFooter } from '@/components/SiteFooter';

// Body markup for /privacy, converted 1:1 from the deleted
// src/content/privacy/body.html. The rendered DOM must stay byte-identical —
// public/scripts/privacy-0.js and scripts/check-parity.mjs both bind to it by
// id/class (ADR 0018). The root <div style={{display:'contents'}}> replaces
// the old RawHtml wrapper so the serialized DOM is unchanged.
export function PrivacyBody() {
  return (
    <div style={{ display: 'contents' }}>
      <main className="wrap">
        <BackLink />

        <h1>Privacy Policy</h1>
        <p className="updated">Last updated: 30 June 2026</p>

        <p className="lede">
          AI Job Hunter is a <b>local-first desktop app</b> with an optional{' '}
          <b>companion browser extension</b>. We don't track you — honestly, we can barely track
          ourselves. There are no accounts to sign up for, nothing here phones home for analytics,
          and most of what the app does never leaves your computer. The parts that <i>do</i> leave
          your computer are spelled out plainly below, because store reviewers read this against the
          actual code, and so should you.
        </p>

        {/* Short version */}
        <h2 id="short">
          The short version{' '}
          <a className="anchor" href="#short" aria-label="Link to The short version">
            #
          </a>
        </h2>
        <div className="card tldr">
          <ul>
            <li>
              <b>No accounts.</b> Nothing to register. We never see your data because there is no
              "we" server to see it.
            </li>
            <li>
              <b>No analytics, no telemetry, no tracking.</b> No Sentry, no Google Analytics, no
              PostHog — nothing.
            </li>
            <li>
              <b>The browser extension is loopback-only.</b> It talks to <i>your own</i> running
              desktop app on <code>127.0.0.1</code> and nowhere else.
            </li>
            <li>
              <b>
                The desktop app talks to a third-party AI provider only when you set one up and run
                an AI feature
              </b>{' '}
              — using <i>your own</i> API key (or your own local model). That content is governed by
              that provider's privacy policy.
            </li>
            <li>
              <b>Your data lives on your machine</b> — résumés, job history and settings are stored
              locally in your OS application-data directory. Secrets (API keys, board passwords) go
              in your operating system's keychain, not plain files.
            </li>
          </ul>
        </div>

        {/* Browser extension */}
        <h2 id="extension">
          Browser extension{' '}
          <a className="anchor" href="#extension" aria-label="Link to Browser extension">
            #
          </a>
        </h2>
        <p>
          The <b>AI Job Hunter — Job Importer</b> extension (Chrome and Firefox) exists to do one
          thing: take the job posting you're looking at and hand it to the desktop app running on
          the same machine. It is inert unless that app is running and you've paired it.
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
              <b>Import this job</b> sends the current tab's URL to the local app. The extension
              also captures the page's rendered DOM when possible (for logged-in boards that a
              headless server-side fetch cannot reach), and sends that HTML only to the local app.{' '}
              <b>Nothing is captured in the background or on page load.</b> Capture happens only
              when you click Import.
            </li>
            <li>
              <b>Fill this form (assisted autofill: opt-in, off by default).</b> If you turn on{' '}
              <i>Assisted form autofill</i> in the desktop app (
              <i>Settings → Accounts → Browser extension</i>), clicking <b>Fill this form</b> asks
              the desktop for your saved contact details (name, email, phone, location,
              LinkedIn/GitHub/website) over the same loopback connection and fills matching{' '}
              <b>empty</b> fields on the current page, then shows an in-page summary. Your details
              are your <b>own data</b>, come from your <b>own paired desktop</b>, are used only for
              that one fill, are <b>never stored in the browser</b>, and{' '}
              <b>never leave your computer except into the page you chose to fill</b>. It{' '}
              <b>never submits the form for you</b>. When the toggle is off, the desktop declines
              the request. This is why the extension collects no data and its Firefox
              data-collection declaration is <code>["none"]</code>.
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
              <code>scripting</code> — MV3 requires this to inject the DOM capture into the active
              tab on import click; its reach stays limited to the active tab.
            </li>
            <li>
              <code>nativeMessaging</code> — connect to the AI Job Hunter desktop host (
              <code>app.aijobhunter.bridge</code>) using the browser's native-messaging channel.
              This is the primary transport to the local app and is immune to Firefox HTTPS-Only
              Mode silently upgrading <code>ws://</code> connections. Falls back to the loopback
              WebSocket if the native host is not registered.
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
            The only value the extension persists is the <b>pairing token</b> — a one-time secret
            you copy from the app's Settings — kept in <code>chrome.storage.local</code>. It is used
            solely to authenticate to your local desktop app and is never sent to any remote server.{' '}
            <b>No telemetry, no analytics, no external API.</b>
          </p>
        </div>

        {/* Desktop app */}
        <h2 id="desktop">
          Desktop app{' '}
          <a className="anchor" href="#desktop" aria-label="Link to Desktop app">
            #
          </a>
        </h2>
        <p>
          The desktop app is local-first: it stores your data on your machine and does its work
          there. But it is an AI job-hunting tool, so some features do reach out over the network —
          by design, and only when you ask. Here is exactly what goes where.
        </p>

        <div className="card">
          <span className="label">AI providers — your text, your key, their servers</span>
          <p style={{ marginTop: '0' }}>
            When you run an AI feature (tailoring a résumé, analysing a job, writing a cover
            letter), the app sends the relevant <b>résumé / job-posting / cover-letter text</b> to
            the AI provider <b>you choose and configure</b>, authenticated with{' '}
            <b>your own API key</b>.
          </p>
          <ul>
            <li>
              <b>Local models</b> — <b>Ollama</b> runs models on your own machine (default; keyless,
              nothing leaves the computer).
            </li>
            <li>
              <b>Cloud providers</b> — <b>OpenAI</b>, <b>Anthropic</b>, <b>Google Gemini</b>,{' '}
              <b>Ollama Cloud</b>, and any <b>OpenAI-compatible</b> endpoint (LM Studio, OpenRouter,
              Groq, Together, DeepSeek, Azure, etc.) via a base URL you set.
            </li>
            <li>
              <b>Local CLI agents</b> — Claude Code, Codex, and the Gemini CLI, run as child
              processes under your own logged-in CLI.
            </li>
          </ul>
          <p>
            When you pick a cloud provider, the content you generate over is sent to{' '}
            <b>that provider</b> and is governed by{' '}
            <b>that provider's own privacy policy and terms</b> — not by us. We are not a party to
            that exchange; the request goes straight from your machine to the provider you chose,
            under your key. If you stay on a local model (Ollama), that text never leaves your
            computer.
          </p>
        </div>

        <div className="card">
          <span className="label">Embeddings</span>
          <p style={{ marginTop: '0' }}>
            To rank job matches the app computes <b>embeddings</b> for your résumé and the postings.
            By default these are computed <b>locally with Ollama</b> (<code>nomic-embed-text</code>
            ), so the text stays on your machine. If you explicitly configure a cloud provider for
            embeddings, the same "your text → your chosen provider, under your key" disclosure above
            applies.
          </p>
        </div>

        <div className="card">
          <span className="label">Job scraping — fetching job-board pages</span>
          <p style={{ marginTop: '0' }}>
            To find and import jobs, the app makes{' '}
            <b>outbound requests to the job boards you search</b>. Most boards (e.g. Greenhouse,
            Lever, Ashby, Personio) are fetched over plain HTTP. LinkedIn job listings are also
            fetched over HTTP like the other boards; the only local-Chromium use is an optional
            login window that saves your LinkedIn session cookie to a per-board profile on your
            machine (to enrich authenticated searches) — not the scrape transport. The walled
            aggregator boards (Indeed, Glassdoor, StepStone, Xing, Workday) are reached via the
            Adzuna/JSearch aggregator API using your own API key — no browser required.{' '}
            <b>Adzuna</b> requests go directly to Adzuna's API. <b>JSearch</b> requests go through{' '}
            <b>RapidAPI</b> (the API gateway for JSearch) using your RapidAPI key. We run no proxy
            or intermediary; there is no AI Job Hunter server in the path. Adzuna, JSearch, and
            RapidAPI requests are subject to their respective terms of service and privacy policies.
          </p>
          <p>
            <b>LinkedIn via Apify (opt-in, off by default).</b> Enabling the "Include LinkedIn
            (Apify)" toggle <em>and</em> providing an Apify token activates an additional source
            that sends a search request to <b>Apify's API</b>, which then queries public LinkedIn
            job listings on your behalf. Both conditions must be met: a token stored <em>and</em>{' '}
            the toggle on. This ensures the feature never runs silently (e.g. during a scheduled
            autopilot run). What leaves your machine: the{' '}
            <b>search keywords, location, and date-range window</b>, used to build a LinkedIn jobs
            search URL. No résumé, profile data, or other personal information is included; your
            Apify token travels in an authentication header only. Requests go directly to Apify's
            API. No AI Job Hunter server is in the path. Results are billed{' '}
            <b>pay-per-result to your own Apify account</b>. Apify requests are subject to{' '}
            <a href="https://apify.com/privacy-policy" rel="noopener noreferrer">
              Apify's terms of service and privacy policy
            </a>
            .
          </p>
        </div>

        <div className="card">
          <span className="label">Where your data is stored</span>
          <p style={{ marginTop: '0' }}>
            Résumés, job and application history, embedding vectors, and settings are stored{' '}
            <b>locally on your machine</b>, in the operating system's standard per-application data
            directory. Secrets — AI provider API keys and saved board passwords — are kept in your{' '}
            <b>operating system's keychain / credential store</b>, never in plain-text files. None
            of this is uploaded anywhere by the app.
          </p>
        </div>

        <div className="flag">
          <b>No telemetry. No analytics.</b> The desktop app contains no analytics or telemetry SDK
          (no Sentry, Google Analytics, PostHog, Segment, Mixpanel, Amplitude, Datadog — none). AI
          Job Hunter checks for application updates by contacting its update server; this transmits
          only your current app version and operating system / architecture — no personal data.
          Other than that, the only network calls are the AI-provider and job-board requests
          described above, which you trigger; nothing about your usage is collected or sent to us.
        </div>

        {/* Your control */}
        <h2 id="control">
          Your control{' '}
          <a className="anchor" href="#control" aria-label="Link to Your control">
            #
          </a>
        </h2>
        <ul>
          <li>
            <b>Rotate the pairing token</b> any time from the app's{' '}
            <i>Settings → Browser extension</i>. Re-pairing invalidates the old token.
          </li>
          <li>
            <b>Uninstall the extension</b> to remove its stored pairing token — it lives only in the
            browser's extension storage.
          </li>
          <li>
            <b>Delete your local app data</b> by removing the app's data directory and clearing the
            saved secrets from your OS keychain. Because there's no server-side copy, deleting
            locally deletes it everywhere.
          </li>
          <li>
            <b>Stay fully local</b> by choosing a local model (Ollama) for AI and embeddings — then
            nothing leaves your machine except the job-board fetches you initiate.
          </li>
        </ul>

        {/* Changes */}
        <h2 id="changes">
          Changes to this policy{' '}
          <a className="anchor" href="#changes" aria-label="Link to Changes to this policy">
            #
          </a>
        </h2>
        <p>
          If this policy changes, we'll bump the <b>"Last updated"</b> date at the top of this page
          and publish the revised version here. Material changes will be reflected in the app's
          store listings. There are no accounts, so there's no mailing list to notify — checking
          this page is the source of truth.
        </p>

        {/* Contact */}
        <h2 id="contact">
          Contact{' '}
          <a className="anchor" href="#contact" aria-label="Link to Contact">
            #
          </a>
        </h2>
        <p>
          Questions about privacy, or a data request? Email{' '}
          <a href="mailto:contact@aijobhunter.app">contact@aijobhunter.app</a>.
        </p>

        <hr className="scrawl" />

        <SiteFooter current="privacy" />
      </main>
    </div>
  );
}
