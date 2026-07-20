/* ============================ NAV ============================ */
      const nav = document.getElementById('nav');
      const prefersReducedMotion = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      nav.addEventListener('click', (e) => {
        const b = e.target.closest('button');
        if (!b) return;
        document
          .querySelectorAll('#nav button')
          .forEach((x) => {
            x.classList.toggle('active', x === b);
            x.setAttribute('aria-current', x === b ? 'true' : 'false');
          });
        const v = b.dataset.view;
        document
          .querySelectorAll('section.view')
          .forEach((s) => s.classList.toggle('active', s.id === 'view-' + v));
        window.scrollTo({ top: 0, behavior: prefersReducedMotion() ? 'auto' : 'smooth' });
      });

      /* ============================ OVERVIEW NODE DETAILS ============================ */
      const NODE = {
        main: {
          t: 'main.tsx — app bootstrap',
          layer: 'ui',
          d: 'The webview entry point. Restores the saved theme, builds the TanStack router, creates the Tauri AppClient, and mounts the provider tree in this order: <b>AppClientProvider → PerformanceModeProvider → QueryClientProvider → RouterProvider</b>, then navigates to <code>/</code>.',
          f: ['apps/desktop/src/main.tsx'],
        },
        routes: {
          t: 'Routes / Pages',
          layer: 'ui',
          d: 'File‑based routes. <code>__root.tsx</code> renders the chrome (Titlebar, Sidebar, StatusBar) plus the onboarding gate. Pages: Dashboard <code>/</code>, <code>/jobs</code>, <code>/ai-generate</code>, <code>/analyze</code>, <code>/documents</code>, <code>/autopilot</code>, <code>/settings</code>, <code>/ai</code>, <code>/monitoring</code>, <code>/support</code>.',
          f: ['apps/desktop/src/renderer/routes/__root.tsx', 'apps/desktop/src/renderer/routes/'],
        },
        features: {
          t: 'Features & components',
          layer: 'ui',
          d: 'Each route owns a feature folder (<code>features/jobs</code>, <code>features/ai-generate</code>, <code>features/onboarding</code>…). Shared primitives come from the <code>@ajh/ui</code> design‑system package.',
          f: ['apps/desktop/src/renderer/features/', 'packages/ui/src/index.ts'],
        },
        hooks: {
          t: 'Service hooks (React Query)',
          layer: 'ui',
          d: 'The ONLY way the UI touches the backend. Each hook calls <code>useAppClient()</code> then wraps an IPC call in <code>useQuery</code>/<code>useMutation</code> — giving caching, <code>staleTime</code>, and automatic cache invalidation. Event hooks (e.g. <code>useJobEvents</code>, <code>useAIStream</code>) subscribe to Tauri events.',
          f: [
            'apps/desktop/src/renderer/services/',
            'apps/desktop/src/renderer/services/query-client/',
          ],
        },
        stores: {
          t: 'Zustand stores',
          layer: 'ui',
          d: 'Two stores. <b>Preferences</b> (persisted to localStorage, versioned + migrated): name, language, active AI provider/model, default resume, performance mode, onboarding flag. <b>Session</b> (in‑memory): per‑route UI state like form text, active tab, wizard step.',
          f: [
            'apps/desktop/src/renderer/store/preferences-store/',
            'apps/desktop/src/renderer/store/session-store/',
          ],
        },
        machines: {
          t: 'State machines',
          layer: 'ui',
          d: "Small transition tables for multi‑step flows so impossible states can't happen. e.g. AI generation is <code>idle → configuring → extracting → generating → done</code> (+ error). Driven by <code>useMachine</code>.",
          f: [
            'apps/desktop/src/renderer/lib/machines/',
            'apps/desktop/src/renderer/hooks/use-machine/',
          ],
        },
        appclient: {
          t: 'createTauriInvokeClient()',
          layer: 'ipc',
          d: 'Assembles 23 namespace objects (<code>ai</code>, <code>jobs</code>, <code>scrape</code>, <code>apply</code>, <code>documents</code>…) into a single <code>AppClient</code> object. Its shape is identical to the old Electron bridge, so every service hook works unchanged.',
          f: ['apps/desktop/src/tauri-client/index.ts', 'apps/desktop/src/tauri-client/namespaces/'],
        },
        invoke: {
          t: "invoke('command', args)",
          layer: 'ipc',
          d: "Each namespace method is a thin wrapper, e.g. <code>scrape.boards(req) ⇒ invoke('scrape_boards', { req })</code>. Tauri serializes the args to JSON, sends them across the native bridge, and resolves with the command's return value (or rejects with the error string).",
          f: ['apps/desktop/src/tauri-client/namespaces/scrape/scrape.ts'],
        },
        listen: {
          t: "listen('channel') — event streams",
          layer: 'ipc',
          d: 'For progress/streaming the renderer subscribes to Tauri events: <span class="chan">ai:stream</span> (tokens), <span class="chan">jobs:event</span> (scrape stream + completion), <span class="chan">autopilot.step</span>, <span class="chan">updater:status</span>. <code>asyncUnsub()</code> turns Tauri\'s async <code>listen()</code> into a synchronous cleanup fn.',
          f: ['apps/desktop/src/tauri-client/utils.ts'],
        },
        contracts: {
          t: 'Contracts + Zod schemas',
          layer: 'ipc',
          d: 'The shared, UI‑free, Node‑free contract package. Zod schemas validate every request shape, and TS types are derived via <code>z.infer</code>. The same schemas generate the Rust request structs (single source of truth).',
          f: ['packages/shared/src/ipc/contracts/', 'packages/shared/src/schemas/index.ts'],
        },
        commands: {
          t: '#[tauri::command] handlers',
          layer: 'rust',
          d: '~120 commands registered in <code>main.rs</code> via <code>generate_handler![]</code>. Each receives an <code>AppHandle</code> + a deserialized request struct, reads shared state via <code>app.state::&lt;T&gt;()</code>, does the work, and returns <code>serde_json::Value</code> (or an <code>AppError</code> that serializes to a string).',
          f: ['apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/commands/'],
        },
        provider: {
          t: 'AI provider layer',
          layer: 'rust',
          d: 'Strictly typed: a <code>ProviderId</code> enum (Ollama, OpenAi, OpenAiCompatible, Anthropic, Gemini, ClaudeCode, Codex, GeminiCli) with <b>no silent fallback</b>. <code>resolve(id, base_url)</code> returns a <code>Box&lt;dyn AiProvider&gt;</code>; the trait\'s <code>chat_stream</code> emits <span class="chan">ai:stream</span> deltas. Cloud = HTTP; CLI agents = spawned subprocess.',
          f: [
            'apps/desktop/src-tauri/src/commands/ai_provider/mod.rs',
            'apps/desktop/src-tauri/src/commands/ai_provider/cli_agent/mod.rs',
          ],
        },
        scraper: {
          t: 'Scraper engine + board registry',
          layer: 'rust',
          d: '<code>ScraperEngine</code> bounds concurrency with a resizable semaphore and tracks cancellation tokens per job. A static <code>SCRAPERS</code> registry (24 boards) maps an id to a <code>&dyn Scraper</code>. Each scraper is HTTP‑mode or Browser‑mode and emits items through an <code>on_item</code> callback.',
          f: [
            'apps/desktop/src-tauri/src/scraping/engine/mod.rs',
            'apps/desktop/src-tauri/src/scraping/boards/mod.rs',
          ],
        },
        autopilot: {
          t: 'Autopilot engine',
          layer: 'rust',
          d: 'A saved hunt runs on a schedule: it scrapes the configured boards, embeds &amp; cosine‑ranks new postings against your résumé, registers a <code>JobTracker</code> job and emits <span class="chan">autopilot.step</span> per stage. New matches raise an OS notification + the tray “New jobs: N”. <b>Nothing is auto‑submitted</b> — the user tailors &amp; applies.',
          f: [
            'apps/desktop/src-tauri/src/autopilot/mod.rs',
            'apps/desktop/src-tauri/src/tray/mod.rs',
          ],
        },
        jobtracker: {
          t: 'JobTracker',
          layer: 'rust',
          d: "In‑memory map of every async job's status (queued/running/completed/failed/cancelled). Long commands register a job, return its <code>jobId</code> at once, then update it as events flow. The UI can poll <code>jobs_get</code> or cancel via <code>jobs_cancel</code>.",
          f: ['apps/desktop/src-tauri/src/jobs/mod.rs'],
        },
        pipeline: {
          t: 'Pipeline (cover letter)',
          layer: 'rust',
          d: "A composable <code>Stage</code> pipeline used for higher‑quality generation: <b>research</b> the company → <b>generate</b> a draft (uses the provider's non‑streaming <code>complete()</code>) → <b>validate</b> there's no fabricated info not in your resume → retry up to 2×. Backed by a small KV cache.",
          f: [
            'apps/desktop/src-tauri/src/pipeline/mod.rs',
            'apps/desktop/src-tauri/src/cover_letter/mod.rs',
          ],
        },
        'stores-rs': {
          t: 'Stores (SQLite / JSON / keyring)',
          layer: 'rust',
          d: 'Managed state created in <code>setup()</code>: <code>DocumentStore</code> (documents.db + vector embeddings), <code>AiGenerationStore</code>, <code>JobPreferencesStore</code> (SQLite); autopilots/interactions/credential‑meta as JSON; API keys &amp; board passwords in the OS keyring.',
          f: [
            'apps/desktop/src-tauri/src/documents/mod.rs',
            'apps/desktop/src-tauri/src/platform/config.rs',
            'apps/desktop/src-tauri/src/credentials/mod.rs',
          ],
        },
        ollama: {
          t: 'Ollama (local LLM)',
          layer: 'ext',
          d: 'A local server on <code>localhost:11434</code>. Keyless. The app probes <code>/api/tags</code> for installed models, streams from <code>/api/generate</code>, and can <code>/api/pull</code> new models with progress events. Also the default embeddings provider.',
          f: ['apps/desktop/src-tauri/src/commands/ai_provider/ollama.rs'],
        },
        cloud: {
          t: 'Cloud APIs (OpenAI / Anthropic / Gemini)',
          layer: 'ext',
          d: 'HTTP providers. The API key is resolved per request from the keyring; requests are built per‑provider (OpenAI chat/completions SSE, Anthropic messages, Gemini). “OpenAI‑compatible” covers LM Studio, vLLM, OpenRouter, Groq, Together, DeepSeek, Azure via a custom base URL.',
          f: [
            'apps/desktop/src-tauri/src/commands/ai_provider/openai.rs',
            'apps/desktop/src-tauri/src/commands/ai_provider/anthropic.rs',
          ],
        },
        cli: {
          t: 'CLI agents (Claude Code / Codex / Gemini CLI)',
          layer: 'ext',
          d: 'Run headless as a child process — keyless, using your own logged‑in CLI. A registry (<code>cli_agent::all()</code>) defines each agent\'s binary, env override, models, argv, and stdout parser. Output is streamed line‑by‑line and re‑emitted as <span class="chan">ai:stream</span> deltas.',
          f: ['apps/desktop/src-tauri/src/commands/ai_provider/cli_agent/claude_code.rs'],
        },
        boards: {
          t: 'Job boards',
          layer: 'ext',
          d: 'LinkedIn is the only browser‑mode scraper: the app drives Chromium through <code>chromiumoxide</code> using a persistent profile so your login survives. Indeed, Glassdoor, Xing, Workday, and StepStone are reached via the <b>Adzuna/JSearch aggregator API</b> — no Chromium needed. The rest (Greenhouse, Lever, Ashby, Personio, Recruitee, and more) are plain‑HTTP ATS/feed scrapers.',
          f: ['apps/desktop/src-tauri/src/scraping/boards/'],
        },
        disk: {
          t: 'Disk & OS keyring',
          layer: 'ext',
          d: 'Everything lives under one data dir (resolved once, exported as <code>AJH_DATA_DIR</code>): the SQLite DBs, JSON records, board Chromium profiles, and logs. Secrets never touch disk in plaintext — they go to the OS keyring.',
          f: [
            'apps/desktop/src-tauri/src/platform/config.rs',
            'apps/desktop/src-tauri/src/net/http.rs',
          ],
        },
      };
      const layerName = { ui: 'Renderer', ipc: 'IPC bridge', rust: 'Rust core', ext: 'External' };
      function renderNode(id) {
        const n = NODE[id];
        const box = document.getElementById('nodeDetail');
        box.innerHTML =
          '<span class="badge ' +
          n.layer +
          '" style="color:var(--' +
          n.layer +
          ')">' +
          layerName[n.layer] +
          '</span><h4 style="margin-top:10px">' +
          n.t +
          '</h4><p class="muted">' +
          n.d +
          '</p>' +
          '<div class="files">' +
          n.f.map((f) => '<span class="path">' + f + '</span>').join('') +
          '</div>';
      }
      document.querySelectorAll('[data-nodes] .node').forEach((el) => {
        el.setAttribute('role', 'button');
        el.setAttribute('tabindex', '0');
        const activate = () => {
          document.querySelectorAll('.node').forEach((x) => {
            x.classList.remove('sel');
            x.setAttribute('aria-pressed', 'false');
          });
          el.classList.add('sel');
          el.setAttribute('aria-pressed', 'true');
          renderNode(el.dataset.node);
        };
        el.addEventListener('click', activate);
        el.addEventListener('keydown', (e) => {
          if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); activate(); }
        });
      });

      /* ============================ GENERIC STEP PLAYER ============================ */
      function buildPlayer(mount, steps) {
        const lanes = [
          ['ui', 'Renderer', 'React'],
          ['ipc', 'IPC', 'AppClient'],
          ['rust', 'Rust', 'Tauri core'],
          ['ext', 'External', 'models·boards·disk'],
        ];
        const el = document.createElement('div');
        el.className = 'player';
        el.innerHTML =
          '<div class="strip">' +
          lanes
            .map(
              (l) =>
                '<div class="lane ' +
                l[0] +
                '" data-lane="' +
                l[0] +
                '">' +
                l[1] +
                '<small>' +
                l[2] +
                '</small></div>'
            )
            .join('') +
          '</div>' +
          '<div class="stage" data-stage aria-live="polite" aria-atomic="true"></div>' +
          '<div class="controls">' +
          '<button data-prev>‹ Back</button>' +
          '<div class="progress"><i data-bar></i></div>' +
          '<span class="stepcount" data-count></span>' +
          '<button class="primary" data-next>Next ›</button>' +
          '</div>';
        mount.innerHTML = '';
        mount.appendChild(el);
        let i = 0;
        const stage = el.querySelector('[data-stage]'),
          bar = el.querySelector('[data-bar]'),
          count = el.querySelector('[data-count]'),
          prev = el.querySelector('[data-prev]'),
          next = el.querySelector('[data-next]');
        function draw() {
          const s = steps[i];
          el.querySelectorAll('.lane').forEach((l) =>
            l.classList.toggle('on', l.dataset.lane === s.layer)
          );
          stage.innerHTML =
            '<span class="badge ' +
            s.layer +
            '">' +
            layerName[s.layer] +
            '</span>' +
            '<h4>' +
            s.title +
            '</h4>' +
            '<p>' +
            s.body +
            '</p>' +
            (s.code ? '<pre>' + s.code + '</pre>' : '') +
            (s.files
              ? '<div class="files" style="margin-top:14px">' +
                s.files.map((f) => '<span class="path">' + f + '</span>').join('') +
                '</div>'
              : '');
          bar.style.width = ((i + 1) / steps.length) * 100 + '%';
          count.textContent = i + 1 + ' / ' + steps.length;
          prev.disabled = i === 0;
          next.disabled = i === steps.length - 1;
        }
        prev.onclick = () => {
          if (i > 0) {
            i--;
            draw();
          }
        };
        next.onclick = () => {
          if (i < steps.length - 1) {
            i++;
            draw();
          }
        };
        el.tabIndex = 0;
        el.setAttribute('aria-label', 'Step player — use arrow keys to navigate');
        el.addEventListener('keydown', (e) => {
          if (e.key === 'ArrowRight') { e.preventDefault(); next.onclick(); }
          if (e.key === 'ArrowLeft') { e.preventDefault(); prev.onclick(); }
        });
        draw();
        return {
          reset: () => {
            i = 0;
            draw();
          },
        };
      }

      /* ============================ BOOT STEPS ============================ */
      const BOOT = [
        {
          layer: 'rust',
          title: 'OS launches the Tauri binary',
          body: 'The native Rust process starts first. It initializes the OS keyring, registers Tauri plugins (log, dialog, opener, updater, clipboard), and enters the <code>setup()</code> closure that runs once.',
          files: ['apps/desktop/src-tauri/src/main.rs'],
        },
        {
          layer: 'rust',
          title: 'setup(): resolve the data directory',
          body: '<code>resolve_and_export_data_dir()</code> decides the single on‑disk home for the app and exports it as the <code>AJH_DATA_DIR</code> env var, so any worker resolves the same path.',
          files: ['apps/desktop/src-tauri/src/platform/config.rs'],
        },
        {
          layer: 'rust',
          title: 'setup(): build & manage shared state',
          body: 'All stores and registries are constructed and handed to Tauri via <code>app.manage()</code> so every command can reach them: <code>DocumentStore</code>, <code>AiGenerationStore</code>, <code>JobPreferencesStore</code>, <code>CredentialStore</code>, <code>JobTracker</code>, <code>PostingsCache</code>, <code>InteractionStore</code>, and the <code>ScraperEngine</code>.',
          files: ['apps/desktop/src-tauri/src/main.rs'],
        },
        {
          layer: 'rust',
          title: 'setup(): start background tasks',
          body: 'Two timers spin up: the <b>updater</b> checks 10s after launch then every 4h, and the <b>autopilot scheduler</b> wakes every 60s to run any due autopilots.',
          files: [
            'apps/desktop/src-tauri/src/updater/',
            'apps/desktop/src-tauri/src/autopilot_scheduler.rs',
          ],
        },
        {
          layer: 'ui',
          title: 'Webview loads main.tsx',
          body: "In parallel, the webview boots the renderer. It restores the saved theme, registers window controls, and creates the TanStack router with <code>defaultPreload:'intent'</code>.",
          files: ['apps/desktop/src/main.tsx'],
        },
        {
          layer: 'ipc',
          title: 'Create the AppClient',
          body: '<code>createTauriInvokeClient()</code> stitches the 23 namespace modules into one typed client object and hands it to <code>AppClientProvider</code> at the top of the tree.',
          code: '<span class="f">&lt;AppClientProvider</span> client={createTauriInvokeClient()}<span class="f">&gt;</span>\\n  <span class="f">&lt;PerformanceModeProvider&gt;</span>\\n    <span class="f">&lt;QueryClientProvider</span> client={queryClient}<span class="f">&gt;</span>\\n      <span class="f">&lt;RouterProvider</span> router={router} <span class="f">/&gt;</span>',
          files: ['apps/desktop/src/tauri-client/index.ts'],
        },
        {
          layer: 'ui',
          title: 'Provider tree mounts, route → /',
          body: 'React renders the provider stack and the router navigates to <code>/</code>. <code>__root.tsx</code> paints the Titlebar, Sidebar, StatusBar and the page Outlet.',
          files: ['apps/desktop/src/renderer/routes/__root.tsx'],
        },
        {
          layer: 'ipc',
          title: 'First health poll',
          body: "A service hook fires <code>invoke('system_health')</code> and keeps polling every 5s. The result reports AI readiness, data stores, worker counts, and which CLI agents are detected.",
          code: 'useSystemHealth() ⇒ invoke(<span class="s">\'system_health\'</span>)\\n<span class="c">// → { ai:{ready,model}, data:{sqlite,vector}, workers, cliAgents }</span>',
          files: ['apps/desktop/src/renderer/services/use-system/'],
        },
        {
          layer: 'ui',
          title: 'Onboarding gate',
          body: "<code>__root.tsx</code> reads <code>useOnboardingCompleted()</code>. If it's the first run, a full‑screen <code>OnboardingWizard</code> (z‑200) covers the app: <b>Welcome</b> (name + language) → <b>Resume</b> (import + OCR) → <b>AI selection</b> (Local / Cloud / CLI tabs) → <b>Browser</b> check, then a spotlight tour.",
          files: ['apps/desktop/src/renderer/features/onboarding/'],
        },
        {
          layer: 'ui',
          title: 'Ready',
          body: 'Once <code>setOnboardingComplete()</code> fires (or it was already done), the wizard unmounts and the Dashboard is interactive. The app is fully local — nothing was sent anywhere except the health checks you configured.',
          files: ['apps/desktop/src/renderer/features/dashboard/'],
        },
      ];

      /* ============================ FLOW STEPS ============================ */
      const FLOWS = {
        onboarding: { label: '① Boot + onboarding', steps: BOOT },
        generate: {
          label: '② Generate résumé (AI streaming)',
          steps: [
            {
              layer: 'ui',
              title: 'User sets up a generation',
              body: 'On <code>/ai-generate</code> the user pastes their résumé + the job ad, picks a target (résumé / cover / both), a mode (ATS / creative) and the model. The state machine sits in <code>idle</code>.',
              files: ['apps/desktop/src/renderer/features/ai-generate/'],
            },
            {
              layer: 'ui',
              title: 'Click “Generate” → machine advances',
              body: 'The machine moves <code>idle → configuring</code>. The <code>useGeneration</code> hook first asks the model to <b>extract metadata</b> (candidate name, job title, key skills) before the full write.',
              code: 'send(<span class="s">\'SUBMIT\'</span>) <span class="c">// idle → configuring</span>\\nawait extractMetadata(resume, jobAd, model)',
              files: ['apps/desktop/src/renderer/services/use-ai/'],
            },
            {
              layer: 'ipc',
              title: 'invoke ai_generate',
              body: "The hook calls <code>api.ai.generate(req)</code>, a wrapper over <code>invoke('ai_generate', { req })</code>. The request carries provider, model, messages, temperature, maxTokens and an optional baseUrl.",
              code: 'generate: (req) =&gt; invoke(<span class="s">\'ai_generate\'</span>, { req })',
              files: ['apps/desktop/src/tauri-client/namespaces/ai/ai.ts'],
            },
            {
              layer: 'rust',
              title: 'Command validates & dispatches',
              body: "The Rust handler parses the provider (<code>ProviderId::parse</code> — a hard error if it's missing/unknown, <b>never</b> a fallback), validates the model, registers a job in the <code>JobTracker</code>, spawns an async task, and <b>returns <code>{ jobId }</code> immediately</b>.",
              code: '<span class="k">let</span> id = ProviderId::parse(req.provider)?;  <span class="c">// no silent fallback</span>\\nid.validate_model(&req.model)?;\\n<span class="k">let</span> provider = resolve(id, req.base_url);\\n<span class="c">// spawn → return { jobId } now</span>',
              files: [
                'apps/desktop/src-tauri/src/commands/ai.rs',
                'apps/desktop/src-tauri/src/commands/ai_provider/mod.rs',
              ],
            },
            {
              layer: 'ext',
              title: 'Provider talks to the model',
              body: "<code>provider.chat_stream()</code> runs. For a <b>cloud/local</b> provider that's an HTTP request (OpenAI SSE / Anthropic / Gemini / Ollama). For a <b>CLI agent</b> it spawns the headless binary and reads its stdout. API keys are resolved from the keyring per request.",
              files: [
                'apps/desktop/src-tauri/src/commands/ai_provider/openai.rs',
                'apps/desktop/src-tauri/src/commands/ai_provider/cli_agent/mod.rs',
              ],
            },
            {
              layer: 'rust',
              title: 'Each token is emitted as an event',
              body: 'As deltas arrive, the provider emits them on the <span class="chan">ai:stream</span> channel, tagged with the <code>jobId</code>. Cancellation is checked per chunk against the JobTracker.',
              code: 'app.emit(<span class="s">"ai:stream"</span>, json!({\\n  <span class="s">"jobId"</span>: id, <span class="s">"delta"</span>: text, <span class="s">"done"</span>: <span class="k">false</span>\\n}));',
              files: ['apps/desktop/src-tauri/src/commands/ai_provider/anthropic.rs'],
            },
            {
              layer: 'ui',
              title: 'UI streams the text live',
              body: "<code>useAIStream</code> (a <code>listen('ai:stream')</code> subscription) filters by jobId and appends each delta to a buffer that <code>StreamingText</code> renders — so the résumé appears token‑by‑token. A <code>thinking</code> flag routes reasoning to a separate panel.",
              code: 'api.ai.onStream(c =&gt; {\\n  <span class="k">if</span> (c.jobId !== myJob) <span class="k">return</span>;\\n  setBuffer(b =&gt; b + c.delta);\\n});',
              files: ['apps/desktop/src/tauri-client/namespaces/ai/ai.ts'],
            },
            {
              layer: 'ui',
              title: 'Done → optionally save',
              body: 'On the terminal <code>done:true</code> event the machine reaches <code>done</code> (generating the cover letter next if target was “both”). The user can export to PDF/DOCX or save to the library.',
              code: 'invoke(<span class="s">\'ai_generations_save\'</span>, { req }) <span class="c">// → ai_generations.db</span>',
              files: [
                'apps/desktop/src/tauri-client/namespaces/aiGenerations/aiGenerations.ts',
                'apps/desktop/src-tauri/src/ai_generations/mod.rs',
              ],
            },
          ],
        },
        scrape: {
          label: '③ Scrape jobs (live results)',
          steps: [
            {
              layer: 'ui',
              title: 'User fills the scrape form',
              body: 'On <code>/jobs</code> the user picks a board and enters query, location, pages and a date filter, then clicks “Start scraping”.',
              files: ['apps/desktop/src/renderer/features/jobs/'],
            },
            {
              layer: 'ipc',
              title: 'invoke scrape_boards',
              body: "<code>useScrapeBoards().mutateAsync(req)</code> → <code>invoke('scrape_boards', { req })</code>. The UI sets <code>scraping=true</code>, clears the live list, and waits for the jobId. Multi-board: up to the full board catalog per run, fanned out ≤3 concurrent.",
              code: 'boards: (req) =&gt; invoke(<span class="s">\'scrape_boards\'</span>, { req })',
              files: ['apps/desktop/src/tauri-client/namespaces/scrape/scrape.ts'],
            },
            {
              layer: 'rust',
              title: 'Command spawns a scrape job',
              body: 'The handler registers a <code>JobTracker</code> entry, builds a <code>BoardSearchInput</code>, grabs the shared <code>ScraperEngine</code>, spawns the work and returns <code>{ jobId }</code> right away.',
              files: ['apps/desktop/src-tauri/src/commands/scrape.rs'],
            },
            {
              layer: 'rust',
              title: 'Engine: throttle + look up the board',
              body: '<code>ScraperEngine</code> acquires a semaphore permit (default <b>2</b> concurrent scrapes, tunable by performance mode), registers a <code>CancellationToken</code> under the jobId, and resolves the scraper from the registry: <code>boards::get(id)</code>.',
              code: '<span class="k">static</span> SCRAPERS: &[&<span class="k">dyn</span> Scraper] = &[ <span class="c">/* ~24 boards */</span> ];\\nboards::get(board)? <span class="c">// &dyn Scraper</span>',
              files: [
                'apps/desktop/src-tauri/src/scraping/engine/mod.rs',
                'apps/desktop/src-tauri/src/scraping/boards/mod.rs',
              ],
            },
            {
              layer: 'ext',
              title: 'Scraper fetches postings',
              body: 'The board runs in HTTP mode (shared <code>reqwest</code> client) or Browser mode (Chromium). For every posting it finds, it invokes the <code>on_item</code> callback — and reports 0→1 progress via <code>on_progress</code>.',
              files: [
                'apps/desktop/src-tauri/src/scraping/boards/',
                'apps/desktop/src-tauri/src/net/http.rs',
              ],
            },
            {
              layer: 'rust',
              title: 'Each item streamed as an event',
              body: 'The <code>on_item</code> callback pushes the posting into the <code>PostingsCache</code> and emits it on <span class="chan">jobs:event</span> with <code>type:\'job.stream\'</code>. On finish it emits <code>job.completed</code> or <code>job.failed</code>.',
              code: 'app.emit(<span class="s">"jobs:event"</span>, json!({\\n  <span class="s">"type"</span>:<span class="s">"job.stream"</span>, <span class="s">"jobId"</span>:id, <span class="s">"data"</span>: posting\\n}));',
              files: [
                'apps/desktop/src-tauri/src/commands/scrape.rs',
                'apps/desktop/src-tauri/src/postings/mod.rs',
              ],
            },
            {
              layer: 'ui',
              title: 'Live postings appear',
              body: "<code>useJobEvents</code> (<code>listen('jobs:event')</code>) prepends each streamed posting to the list (capped at 500) and invalidates the jobs query cache. On <code>job.completed</code> it flips <code>scraping=false</code>.",
              files: [
                'apps/desktop/src/tauri-client/namespaces/jobs/jobs.ts',
                'apps/desktop/src/renderer/services/use-jobs/',
              ],
            },
            {
              layer: 'ui',
              title: 'Watchdog fallback',
              body: "In case an event is ever dropped, a <code>useEffect</code> polls <code>invoke('jobs_get', jobId)</code> every 2.5s and reconciles the UI once a terminal status is known. From here the user can queue matches into an Autopilot (next flow).",
              code: 'invoke(<span class="s">\'jobs_get\'</span>, { jobId }) <span class="c">// every 2.5s while running</span>',
              files: ['apps/desktop/src/tauri-client/namespaces/jobs/jobs.ts'],
            },
          ],
        },
        autopilot: {
          label: '④ Autopilot (find → rank → notify)',
          steps: [
            {
              layer: 'ui',
              title: 'User saves an autopilot',
              body: 'On <code>/autopilot</code> the user defines a saved hunt — boards, filters and a schedule — in the workflow builder. It runs unattended and surfaces matches for the user to tailor &amp; apply.',
              files: ['apps/desktop/src/renderer/features/autopilot/'],
            },
            {
              layer: 'ipc',
              title: 'invoke autopilot_run',
              body: "<code>useAutopilot().run(id)</code> → <code>invoke('autopilot_run', { autopilotId })</code>. List/create/update/pause/resume share the same namespace.",
              code: 'run: (autopilotId) =&gt; invoke(<span class="s">\'autopilot_run\'</span>, { autopilotId })',
              files: ['apps/desktop/src/tauri-client/namespaces/autopilot/autopilot.ts'],
            },
            {
              layer: 'rust',
              title: 'Engine runs the hunt',
              body: 'The handler scrapes the configured boards, embeds &amp; cosine‑ranks postings against the résumé, keeps the top matches, registers a <code>JobTracker</code> job and emits <span class="chan">autopilot.step</span> per stage (scrape → rank → notify).',
              code: 'app.emit(<span class="s">"autopilot.step"</span>, json!({\\n  <span class="s">"autopilotId"</span>:id, <span class="s">"step"</span>:step, <span class="s">"detail"</span>:detail\\n}));',
              files: [
                'apps/desktop/src-tauri/src/commands/autopilot.rs',
                'apps/desktop/src-tauri/src/autopilot/mod.rs',
              ],
            },
            {
              layer: 'rust',
              title: 'Notify on new matches',
              body: 'New postings raise a permission‑gated OS notification and update the system tray (“New jobs: N” + “Pause all autopilots”). <b>Nothing is auto‑submitted</b> — the app finds &amp; notifies, the user decides.',
              files: ['apps/desktop/src-tauri/src/tray/mod.rs'],
            },
            {
              layer: 'ext',
              title: 'Deep‑link focus',
              body: "Clicking the notification opens <code>ajh://autopilot/&lt;id&gt;</code>, validated against a strict allowlist, which focuses the app on that autopilot's results.",
              code: '<span class="chan">autopilot:focus</span> <span class="c">// ajh://autopilot/&lt;id&gt;</span>',
              files: ['apps/desktop/src-tauri/src/deeplink/mod.rs'],
            },
            {
              layer: 'ui',
              title: 'User tailors & applies',
              body: 'The user reviews the ranked matches, generates a tailored résumé/cover letter (the Generate flow), and applies on the board themselves. The run is recorded for the Autopilot history.',
              files: [
                'apps/desktop/src/renderer/features/autopilot/',
                'apps/desktop/src-tauri/src/postings/mod.rs',
              ],
            },
          ],
        },
      };

      /* build flow tabs + player */
      const flowTabs = document.getElementById('flowTabs');
      const flowMount = document.getElementById('flowPlayer');
      let flowCtl = null;
      Object.entries(FLOWS).forEach(([key, f], idx) => {
        const t = document.createElement('div');
        t.className = 'tab' + (idx === 0 ? ' active' : '');
        t.textContent = f.label;
        t.dataset.flow = key;
        t.setAttribute('role', 'button');
        t.setAttribute('tabindex', '0');
        t.setAttribute('aria-pressed', idx === 0 ? 'true' : 'false');
        flowTabs.appendChild(t);
      });
      const activateTab = (t) => {
        if (!t) return;
        flowTabs.querySelectorAll('.tab').forEach((x) => {
          x.classList.toggle('active', x === t);
          x.setAttribute('aria-pressed', x === t ? 'true' : 'false');
        });
        flowCtl = buildPlayer(flowMount, FLOWS[t.dataset.flow].steps);
      };
      flowTabs.addEventListener('click', (e) => activateTab(e.target.closest('.tab')));
      flowTabs.addEventListener('keydown', (e) => {
        const t = e.target.closest('.tab');
        if (t && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); activateTab(t); }
      });

      /* ============================ IPC TABLE ============================ */
      const IPC = [
        ['ai', 'generate', 'inv', 'ai_generate'],
        ['ai', 'generatePipeline', 'inv', 'generate_pipeline'],
        ['ai', 'listModels', 'inv', 'ai_list_models'],
        ['ai', 'pullModel', 'inv', 'ai_pull_model'],
        ['ai', 'unloadModel', 'inv', 'ai_unload_model'],
        ['ai', 'embed', 'inv', 'ai_embed'],
        ['ai', 'onStream', 'evt', 'ai:stream'],
        ['ai', 'setProviderKey', 'inv', 'ai_set_provider_key'],
        ['ai', 'removeProviderKey', 'inv', 'ai_remove_provider_key'],
        ['ai', 'hasProviderKey', 'inv', 'ai_has_provider_key'],
        ['ai', 'testProviderKey', 'inv', 'ai_test_provider_key'],
        ['ai', 'listProviderModels', 'inv', 'ai_list_provider_models'],
        ['ai', 'embeddingStatus', 'inv', 'ai_embedding_status'],
        ['ai', 'setEmbeddingConfig', 'inv', 'ai_set_embedding_config'],
        ['ai', 'reembedAll', 'inv', 'ai_reembed_all'],
        ['aiGenerations', 'list', 'inv', 'ai_generations_list'],
        ['aiGenerations', 'save', 'inv', 'ai_generations_save'],
        ['aiGenerations', 'remove', 'inv', 'ai_generations_remove'],
        ['autopilot', 'list', 'inv', 'autopilot_list'],
        ['autopilot', 'get', 'inv', 'autopilot_get'],
        ['autopilot', 'create', 'inv', 'autopilot_create'],
        ['autopilot', 'update', 'inv', 'autopilot_update'],
        ['autopilot', 'remove', 'inv', 'autopilot_remove'],
        ['autopilot', 'run', 'inv', 'autopilot_run'],
        ['autopilot', 'pause', 'inv', 'autopilot_pause'],
        ['autopilot', 'resume', 'inv', 'autopilot_resume'],
        ['autopilot', 'onStep', 'evt', 'autopilot.step'],
        ['boards', 'connect', 'inv', 'boards_connect'],
        ['boards', 'disconnect', 'inv', 'boards_disconnect'],
        ['boards', 'getStatus', 'inv', 'boards_get_status'],
        ['conversations', 'getOrCreateConversation', 'inv', 'conversations_get_or_create'],
        ['conversations', 'loadMessages', 'inv', 'conversations_load_messages'],
        ['conversations', 'saveMessage', 'inv', 'conversations_save_message'],
        ['conversations', 'saveAllMessages', 'inv', 'conversations_save_all_messages'],
        ['credentials', 'available', 'inv', 'credentials_available'],
        ['credentials', 'list', 'inv', 'credentials_list'],
        ['credentials', 'set', 'inv', 'credentials_set'],
        ['credentials', 'remove', 'inv', 'credentials_remove'],
        ['data', 'export', 'inv', 'data_export'],
        ['data', 'import', 'inv', 'data_import'],
        ['dialog', 'openFiles', 'inv', 'dialog_open_files'],
        ['documents', 'list', 'inv', 'documents_list'],
        ['documents', 'import', 'inv', 'documents_import'],
        ['documents', 'remove', 'inv', 'documents_remove'],
        ['documents', 'setDefault', 'inv', 'documents_set_default'],
        ['documents', 'exportDocument', 'inv', 'documents_export_document'],
        ['documents', 'exportAndSave', 'inv', 'documents_export_and_save'],
        ['geocode', 'suggest', 'inv', 'geocode_suggest'],
        ['jobPreferences', 'get', 'inv', 'job_preferences_get'],
        ['jobPreferences', 'set', 'inv', 'job_preferences_set'],
        ['jobs', 'list', 'inv', 'jobs_list'],
        ['jobs', 'get', 'inv', 'jobs_get'],
        ['jobs', 'cancel', 'inv', 'jobs_cancel'],
        ['jobs', 'retry', 'inv', 'jobs_retry'],
        ['jobs', 'onEvent', 'evt', 'jobs:event'],
        ['linkedin', 'connect', 'inv', 'boards_login_with_browser'],
        ['linkedin', 'disconnect', 'inv', 'boards_logout'],
        ['linkedin', 'getStatus', 'inv', 'boards_get_status'],
        ['linkedin', 'importProfileFromUrl', 'inv', 'profile_import_from_url'],
        ['match', 'resume', 'inv', 'match_resume'],
        ['privacy', 'signOutAll', 'inv', 'privacy_sign_out_all'],
        ['privacy', 'clearInteractions', 'inv', 'privacy_clear_interactions'],
        ['privacy', 'resetApp', 'inv', 'privacy_reset_app'],
        ['resume', 'extractText', 'inv', 'resume_extract_text'],
        ['scrape', 'boards', 'inv', 'scrape_boards'],
        ['scrape', 'url', 'inv', 'scrape_url'],
        ['scrape', 'resolveUrl', 'inv', 'scrape_resolve_url'],
        ['scrape', 'persistJob', 'inv', 'scrape_persist_job'],
        ['scrape', 'listPostings', 'inv', 'scrape_list_postings'],
        ['scrape', 'clearPostings', 'inv', 'scrape_clear_postings'],
        ['scrape', 'listInteractions', 'inv', 'scrape_list_interactions'],
        ['support', 'exportDiagnostics', 'inv', 'support_export_diagnostics'],
        ['support', 'reloadAiRuntime', 'inv', 'support_reload_ai_runtime'],
        ['support', 'unloadAllModels', 'inv', 'support_unload_all_models'],
        ['support', 'resetModelConfiguration', 'inv', 'support_reset_model_configuration'],
        ['support', 'rebuildVectorIndexes', 'inv', 'support_rebuild_vector_indexes'],
        ['support', 'clearEmbeddingsCache', 'inv', 'support_clear_embeddings_cache'],
        ['support', 'resetVectorDatabase', 'inv', 'support_reset_vector_database'],
        ['support', 'clearOcrCache', 'inv', 'support_clear_ocr_cache'],
        ['support', 'reindexAllDocuments', 'inv', 'support_reindex_all_documents'],
        ['support', 'resetAllSessions', 'inv', 'support_reset_all_sessions'],
        ['support', 'clearScrapingQueue', 'inv', 'support_clear_scraping_queue'],
        ['support', 'copyEnvironmentDetails', 'inv', 'support_copy_environment_details'],
        ['support', 'copyAppVersion', 'inv', 'support_copy_app_version'],
        ['support', 'copySystemInfo', 'inv', 'support_copy_system_info'],
        ['system', 'health', 'inv', 'system_health'],
        ['system', 'getVersion', 'inv', 'system_get_version'],
        ['system', 'getLocale', 'inv', 'system_get_locale'],
        ['system', 'setLocale', 'inv', 'system_set_locale'],
        ['system', 'getPlatform', 'inv', 'system_get_platform'],
        ['system', 'openExternal', 'inv', 'system_open_external'],
        ['system', 'setPerformanceMode', 'inv', 'system_set_performance_mode'],
        ['system', 'getMetrics', 'inv', 'system_get_metrics'],
        ['system', 'checkBrowser', 'inv', 'system_check_browser'],
        ['system', 'openDevtools', 'inv', 'system_open_devtools'],
        ['updater', 'check', 'inv', 'updater_check'],
        ['updater', 'download', 'inv', 'updater_download'],
        ['updater', 'install', 'inv', 'updater_install'],
        ['updater', 'onStatus', 'evt', 'updater:status'],
      ];
      const ipcBody = document.getElementById('ipcBody'),
        ipcCount = document.getElementById('ipcCount'),
        ipcFilter = document.getElementById('ipcFilter');
      function renderIpc(q = '') {
        q = q.trim().toLowerCase();
        const rows = IPC.filter(
          (r) => !q || (r[0] + ' ' + r[1] + ' ' + r[3]).toLowerCase().includes(q)
        );
        ipcBody.innerHTML = rows
          .map((r) => {
            const kind =
              r[2] === 'evt'
                ? '<span class="tag evt">listen</span>'
                : '<span class="tag inv">invoke</span>';
            const target =
              r[2] === 'evt'
                ? '<span class="chan">' + r[3] + '</span>'
                : '<span class="cmd">' + r[3] + '</span>';
            return (
              '<tr><td><b>' +
              r[0] +
              '</b></td><td class="mono" style="font-size:12.5px">' +
              r[1] +
              '</td><td>' +
              kind +
              '</td><td>' +
              target +
              '</td></tr>'
            );
          })
          .join('');
        ipcCount.textContent = rows.length + ' of ' + IPC.length + ' endpoints';
      }
      ipcFilter.addEventListener('input', (e) => renderIpc(e.target.value));
      renderIpc();

      /* ============================ SUBSYSTEMS ============================ */
      const SUBS = [
        {
          h: 'AI provider layer — one trait, many backends',
          b: `
    <p>Routing is by the <code>ProviderId</code> enum — <b>stringly‑typed checks are banned</b> and there is
    <b>no silent fallback to Ollama</b>. <code>resolve(id, base_url)</code> returns a
    <code>Box&lt;dyn AiProvider&gt;</code>; CLI agents are routed first via <code>cli_agent::backend_for()</code>.</p>
    <ul>
      <li><b>Trait surface:</b> <code>chat_stream</code> (emits <span class="chan">ai:stream</span>),
        <code>complete</code> (one‑shot, used by server‑side pipelines), <code>embed</code>,
        <code>list_models</code>, <code>test_key</code>, plus a per‑model <code>capabilities()</code> matrix
        (streaming / temperature / reasoning / tools / JSON / embeddings, and which token field to use).</li>
      <li><b>Cloud / local</b> (Ollama, OpenAI, OpenAI‑compatible, Anthropic, Gemini): HTTP via the shared
        <code>reqwest</code> client; key from the keyring per request.</li>
      <li><b>CLI agents</b> (Claude Code, Codex, Gemini CLI): keyless — spawned as a headless child process,
        stdout parsed line‑by‑line and re‑emitted as stream deltas.</li>
      <li><b>Forgiving validation:</b> <code>validate_model</code> only rejects an <i>unambiguous</i>
        cross‑provider mistake (a <code>gpt‑*</code> model on Anthropic); brand‑new model names are accepted so
        releases need no code change.</li>
    </ul>
    <div class="pillrow"><span>ollama</span><span>openai</span><span>openai‑compatible</span><span>anthropic</span><span>gemini</span><span>claude‑code</span><span>codex</span><span>gemini‑cli</span></div>`,
          f: [
            'apps/desktop/src-tauri/src/commands/ai_provider/mod.rs',
            'apps/desktop/src-tauri/src/commands/ai_provider/cli_agent/mod.rs',
          ],
        },
        {
          h: 'Scraping — engine + board registry (24 boards)',
          b: `
    <p>The <code>ScraperEngine</code> bounds concurrency with a resizable semaphore (default 2) and keeps a
    cancellation token per running job. Boards live in a static <code>SCRAPERS</code> registry.</p>
    <ul>
      <li><b>Three modes:</b> <i>Browser</i> (Chromium via <code>chromiumoxide</code>) for <b>LinkedIn</b> only; <i>Aggregator API</i> (Adzuna primary, JSearch paid fallback) for the walled boards — Indeed, Glassdoor, Xing, Workday, StepStone; and <i>HTTP</i> for the rest (Greenhouse, Lever, Ashby, Personio, Recruitee, SmartRecruiters, Remotive, RemoteOK, WeWorkRemotely, YCombinator, ArbeitNow, BerlinStartupJobs, GermanTechJobs, Arbeitsagentur).</li>
      <li><b>Streaming:</b> each posting is pushed through an <code>on_item</code> callback → cached + emitted on
        <span class="chan">jobs:event</span>. Progress flows through <code>on_progress</code>.</li>
      <li><b>Add a board:</b> implement the <code>Scraper</code> trait in one module + add one line to
        <code>SCRAPERS</code>. Nothing else changes.</li>
    </ul>`,
          f: [
            'apps/desktop/src-tauri/src/scraping/engine/mod.rs',
            'apps/desktop/src-tauri/src/scraping/boards/mod.rs',
            'apps/desktop/src-tauri/src/scraping/types/mod.rs',
          ],
        },
        {
          h: 'Autopilot — find → rank → notify',
          b: `
    <p>A saved <b>autopilot</b> runs on a schedule. The engine scrapes the configured boards, embeds and
    cosine‑ranks new postings against your résumé, and keeps the top matches — then <b>notifies</b> you.
    <b>Nothing is auto‑submitted</b>; you tailor &amp; apply.</p>
    <ul>
      <li><b>Run:</b> the scheduler ticks and runs any due autopilot end‑to‑end, emitting
        <span class="chan">autopilot.step</span> per stage.</li>
      <li><b>Notify:</b> new matches raise an OS notification and update the tray (“New jobs: N” /
        “Pause all autopilots”).</li>
      <li><b>Focus:</b> clicking the notification opens <code>ajh://autopilot/&lt;id&gt;</code> (allow‑listed
        deep link) and focuses that autopilot's results.</li>
    </ul>`,
          f: [
            'apps/desktop/src-tauri/src/autopilot/mod.rs',
            'apps/desktop/src-tauri/src/tray/mod.rs',
            'apps/desktop/src-tauri/src/deeplink/mod.rs',
          ],
        },
        {
          h: 'Documents & embeddings',
          b: `
    <p><code>DocumentStore</code> (SQLite, <span class="path">documents.db</span>) holds résumés/cover letters plus a
    <code>vectors</code> table. Every vector is tagged with its <b>embedding space</b>
    <code>(provider, model, dim)</code> so incompatible vectors can never be silently compared — a mismatch is an
    error, not a 0.0 score.</p>
    <ul>
      <li><b>Ranking</b> uses cosine similarity between the résumé vector and each posting vector.</li>
      <li>Changing the embedding config marks vectors stale → re‑embed on demand (<code>ai_reembed_all</code>).</li>
    </ul>`,
          f: ['apps/desktop/src-tauri/src/documents/mod.rs'],
        },
        {
          h: 'Persistence map — where everything lives',
          b: `
    <p>One data directory, resolved once and exported as <code>AJH_DATA_DIR</code>. Repo‑relative names only:</p>
    <ul>
      <li><b>SQLite:</b> <span class="path">documents.db</span>, <span class="path">ai_generations.db</span>,
        <span class="path">job_preferences.db</span></li>
      <li><b>JSON:</b> <span class="path">autopilots.json</span>, <span class="path">interactions.json</span>,
        <span class="path">credential-meta.json</span></li>
      <li><b>Profiles:</b> <span class="path">board_profiles/&lt;board&gt;/</span> (Chromium)</li>
      <li><b>Secrets:</b> OS keyring (API keys, board passwords) — never plaintext on disk</li>
      <li><b>Logs:</b> rotating files via <code>tauri_plugin_log</code></li>
    </ul>`,
          f: [
            'apps/desktop/src-tauri/src/platform/config.rs',
            'apps/desktop/src-tauri/src/credentials/mod.rs',
          ],
        },
        {
          h: 'Errors & observability',
          b: `
    <p><code>AppError</code> is a typed enum (Config, Network, Provider, Storage, Parse, NotFound, Validation,
    Cancelled, Message) with a <code>code()</code> and <code>retriable()</code>. It <b>serializes to a plain
    string</b>, so on the JS side a failed <code>invoke</code> is just a rejected promise with a readable
    message. Every subsystem wraps work in a <code>Span</code> that logs a <code>→</code> line at start and a
    <code>←</code> line with <code>duration=…ms ok=…</code> at the end.</p>`,
          f: ['apps/desktop/src-tauri/src/error.rs', 'apps/desktop/src-tauri/src/observability.rs'],
        },
      ];
      document.getElementById('subs').innerHTML = SUBS.map(
        (s) =>
          '<details class="acc"><summary>' +
          s.h +
          '</summary><div class="body">' +
          s.b +
          '<div class="files" style="margin-top:12px">' +
          s.f.map((f) => '<span class="path">' + f + '</span>').join('') +
          '</div></div></details>'
      ).join('');

      /* ============================ CHEAT SHEET ============================ */
      const QA = [
        {
          q: 'Why Tauri instead of Electron?',
          a: `Tauri uses the OS\'s native webview instead of bundling Chromium, so the
    binary is far smaller and lighter on memory, and the “backend” is real <b>Rust</b> — ideal for CPU‑heavy work
    like scraping, browser automation, PDF/DOCX rendering and local embeddings. The renderer stayed structurally
    identical to the previous Electron client (same <code>AppClient</code> shape), so the migration touched the
    transport, not the UI.`,
        },
        {
          q: 'How does the renderer talk to Rust?',
          a: `Only through service hooks. A hook calls <code>useAppClient()</code>
    to get the <code>AppClient</code>, then a namespace method like <code>api.scrape.boards(req)</code> which is a
    one‑line wrapper over <code>invoke(\'scrape_boards\', { req })</code>. Tauri serializes args to JSON, routes to
    the matching <code>#[tauri::command]</code>, and resolves the promise with the return value. The UI never
    touches <code>invoke</code> directly — that\'s enforced by ESLint.`,
        },
        {
          q: 'How is streaming implemented, and why return a jobId?',
          a: `Long operations (AI generation, scraping, autopilot runs)
    can\'t block a single request/response. So the command registers the work in the <code>JobTracker</code> and
    returns <code>{ jobId }</code> <b>immediately</b>. Progress then flows back as Tauri <b>events</b> —
    <span class="chan">ai:stream</span>, <span class="chan">jobs:event</span>, <span class="chan">autopilot.step</span> —
    which the renderer subscribes to with <code>listen()</code> and filters by <code>jobId</code>. That\'s how the
    résumé text and live postings appear incrementally.`,
        },
        {
          q: 'How do you keep types in sync across the TS ↔ Rust boundary?',
          a: `Zod schemas in
    <span class="path">packages/shared/src/schemas</span> are the single source of truth. <code>pnpm gen:ipc</code>
    generates the matching Rust request structs into <span class="path">apps/desktop/src-tauri/src/ipc_contracts/</span>,
    and CI runs it with <code>--check</code> to fail on drift. TS types come from <code>z.infer</code>. Tauri\'s
    macro deserializes the JSON payload straight into the typed struct via serde.`,
        },
        {
          q: 'How do you add a new AI provider / job board?',
          a: `Each is a registry. A new provider = a client
    module + one <code>ProviderId</code> arm + one <code>resolve()</code> arm (CLI agents don\'t even touch
    <code>resolve</code> — just the <code>cli_agent</code> registry). A new board = implement <code>Scraper</code>
    and add one line to <code>SCRAPERS</code>. This “zero‑edit‑elsewhere” extensibility is a deliberate design value of the project.`,
        },
        {
          q: 'How are secrets and personal data handled (local‑first)?',
          a: `Everything stays on the machine. Résumés,
    generations, preferences and embeddings live in local SQLite/JSON under one data dir; API keys and board
    passwords go to the <b>OS keyring</b>, never to disk in plaintext. The only outbound traffic is what the user
    configures: chosen cloud AI APIs and the job boards being scraped. Privacy commands
    (<code>privacy_reset_app</code>, <code>privacy_clear_interactions</code>) wipe it.`,
        },
        {
          q: 'How is concurrency and cancellation managed?',
          a: `The <code>ScraperEngine</code> uses a resizable semaphore to
    cap concurrent scrapes (tuned by performance mode). Every long job gets a <code>CancellationToken</code> stored
    by <code>jobId</code>; background loops check it each chunk so <code>jobs_cancel</code> stops work promptly.
    The <code>JobTracker</code> is the authoritative status map, and the UI also runs a 2.5s watchdog poll
    (<code>jobs_get</code>) so a dropped event can\'t leave the UI stuck.`,
        },
        {
          q: 'What does the Autopilot actually do?',
          a: `A scheduler ticks every 60s and runs any due autopilot end‑to‑end:
    scrape the configured board → embed and cosine‑rank postings against your résumé → keep the top N → generate a
    validated cover letter through the <code>Stage</code> pipeline (research → draft → leakage check → retry) →
    notify you (OS notification + tray) so you can tailor &amp; apply. It chains the other flows in this doc.`,
        },
        {
          q: 'Why a state machine for AI generation?',
          a: `Generation has real phases (extract metadata, then stream the
    document, maybe twice for “both”) where only certain transitions are valid. A tiny transition table
    (<code>idle → configuring → extracting → generating → done</code> + <code>error</code>) makes illegal UI states
    impossible and keeps the busy/error flags in one place, instead of a tangle of booleans.`,
        },
      ];
      document.getElementById('qa').innerHTML = QA.map(
        (x) =>
          '<details class="acc qa"><summary>' +
          x.q +
          '</summary><div class="body">' +
          x.a +
          '</div></details>'
      ).join('');

      /* ============================ INIT PLAYERS ============================ */
      buildPlayer(document.getElementById('bootPlayer'), BOOT);
      flowCtl = buildPlayer(flowMount, FLOWS.onboarding.steps);
