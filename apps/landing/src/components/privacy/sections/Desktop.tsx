// "Desktop app" block of /privacy, split out of PrivacyBody.tsx purely for
// file size — verbatim ported markup, no props. See PrivacyBody.tsx for the
// shared conversion notes. Nothing scripts this page (privacy-0.js is a
// console easter egg), but scripts/check-parity.mjs pins its copy and anchor
// hrefs (ADR 0018).
export function Desktop() {
  return (
    <>
      {/* Desktop app */}
      <h2 id="desktop">
        Desktop app{' '}
        <a className="anchor" href="#desktop" aria-label="Link to Desktop app">
          #
        </a>
      </h2>
      <p>
        The desktop app is local-first: it stores your data on your machine and does its work there.
        But it is an AI job-hunting tool, so some features do reach out over the network — by
        design, and only when you ask. Here is exactly what goes where.
      </p>

      <div className="card">
        <span className="label">AI providers — your text, your key, their servers</span>
        <p style={{ marginTop: '0' }}>
          When you run an AI feature (tailoring a résumé, analysing a job, writing a cover letter),
          the app sends the relevant <b>résumé / job-posting / cover-letter text</b> to the AI
          provider <b>you choose and configure</b>, authenticated with <b>your own API key</b>.
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
            <b>Local CLI agents</b> — Claude Code, Codex, and the Gemini CLI, run as child processes
            under your own logged-in CLI.
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
          fetched over HTTP like the other boards; the only local-Chromium use is an optional login
          window that saves your LinkedIn session cookie to a per-board profile on your machine (to
          enrich authenticated searches) — not the scrape transport. The walled aggregator boards
          (Indeed, Glassdoor, StepStone, Xing, Workday) are reached via the Adzuna/JSearch
          aggregator API using your own API key — no browser required. <b>Adzuna</b> requests go
          directly to Adzuna's API. <b>JSearch</b> requests go through <b>RapidAPI</b> (the API
          gateway for JSearch) using your RapidAPI key. We run no proxy or intermediary; there is no
          AI Job Hunter server in the path. Adzuna, JSearch, and RapidAPI requests are subject to
          their respective terms of service and privacy policies.
        </p>
        <p>
          <b>LinkedIn via Apify (opt-in, off by default).</b> Enabling the "Include LinkedIn
          (Apify)" toggle <em>and</em> providing an Apify token activates an additional source that
          sends a search request to <b>Apify's API</b>, which then queries public LinkedIn job
          listings on your behalf. Both conditions must be met: a token stored <em>and</em> the
          toggle on. This ensures the feature never runs silently (e.g. during a scheduled autopilot
          run). What leaves your machine: the{' '}
          <b>search keywords, location, and date-range window</b>, used to build a LinkedIn jobs
          search URL. No résumé, profile data, or other personal information is included; your Apify
          token travels in an authentication header only. Requests go directly to Apify's API. No AI
          Job Hunter server is in the path. Results are billed{' '}
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
          <b>operating system's keychain / credential store</b>, never in plain-text files. None of
          this is uploaded anywhere by the app.
        </p>
      </div>

      <div className="flag">
        <b>Crash reporting — on by default, and switchable off.</b> The desktop app sends crash and
        error reports to <b>Sentry</b> (Functional Software, Inc.), our only data processor, so that
        failures can be found and fixed.
        <br />
        <br />
        <b>What is sent:</b> the error and its stack trace, your operating system and architecture,
        and the app version. <b>What is never sent:</b> your résumés, cover letters, job data,
        prompts, generated documents, credentials, or anything you typed. Before any report leaves
        your machine, file paths, links, host names, e-mail addresses and credential-shaped values
        are replaced with placeholders, and the machine name is not attached.
        <br />
        <br />
        <b>Your choice:</b> you are asked during first-run setup, and{' '}
        <i>nothing is sent until you have been asked</i>. You can turn it off there, or later under{' '}
        <i>Settings → Privacy</i>. Reports are kept for 30 days. To have any report deleted, e-mail
        us and we will remove it.
        <br />
        <br />
        <b>No behavioural analytics.</b> There is no Google Analytics, PostHog, Segment, Mixpanel,
        Amplitude or Datadog, and no advertising or cross-site tracking. Nothing records which
        features you use or what you search for. AI Job Hunter also checks for updates by contacting
        its update server; this transmits only your current app version and operating system /
        architecture. Other than that, the only network calls are the AI-provider and job-board
        requests described above, which you trigger.
      </div>
    </>
  );
}
