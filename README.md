<p align="center">
  <img src="docs/banner.png" alt="AI Job Hunter" width="100%">
</p>

# AI Job Hunter

> Your local-first, AI-native desktop assistant for intelligent job searching, resume generation, and automated applications — run it fully offline with Ollama, or plug in your own OpenAI, Anthropic, or Gemini key.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![TypeScript](https://img.shields.io/badge/TypeScript-6.0-blue)](https://www.typescriptlang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-purple)](https://tauri.app)

---

## What It Does

AI Job Hunter is a desktop application built with Tauri that brings the full power of AI-driven job hunting to your local machine. It scrapes 18+ job boards, semantically matches postings to your resume, generates tailored cover letters and resumes using your AI provider of choice, and can autonomously apply to jobs on your behalf — all while keeping your data and credentials stored locally on your device.

### AI Provider Flexibility

Choose how you want to run the AI — you can switch providers at any time in Settings → AI:

| Provider              | Models                                           | Notes                                        |
| --------------------- | ------------------------------------------------ | -------------------------------------------- |
| **Ollama** (local)    | mistral, llama3.2, neural-chat, any Ollama model | No API key needed; fully offline             |
| **OpenAI**            | GPT-4o, GPT-4 Turbo, GPT-3.5 Turbo               | Requires API key                             |
| **Anthropic**         | Claude 3.5 Sonnet, Claude 3 Opus                 | Requires API key; supports extended thinking |
| **Google Gemini**     | Gemini 1.5 Pro, Gemini 1.5 Flash                 | Requires API key                             |
| **OpenAI-compatible** | Any (LM Studio, remote Ollama, etc.)             | Custom base URL                              |

API keys are stored encrypted in the OS keychain — never in plain text or config files.

### Why It Exists

Modern job searching is repetitive, time-consuming, and emotionally draining. AI Job Hunter automates the mechanical parts — scraping, matching, writing — so you can focus on the interviews that matter. All application data (jobs, resumes, interactions) stays in a local SQLite database on your machine; the only outbound calls are to the AI provider you explicitly configure.

---

## Key Features

- **Multi-board scraping** — LinkedIn, Indeed, StepStone, Xing, Greenhouse, Lever, Workday, and 11 more boards in one pass
- **AI resume & cover letter generation** — Streaming generation with 9 professional templates, DOCX/PDF export, ATS-safe formatting
- **Semantic job matching** — Hybrid vector + keyword search powered by LanceDB; scores each posting against your resume
- **Resume analysis** — ATS scoring, skill gap detection, language mismatch warnings, and improvement recommendations
- **Autopilot** — Define a workflow (board, schedule, resume, message) and let the app apply to jobs automatically
- **Document processing** — Import PDF, DOCX, TXT, images (OCR via Tesseract); extract and chunk for embedding
- **Multi-provider AI** — Ollama (local), OpenAI, Anthropic (extended thinking), Gemini, LM Studio
- **Multilingual** — UI and generation in 11 languages: en, de, fr, es, it, tr, pt, ru, zh, ja, ko
- **Privacy-first** — Credentials in OS keychain; all data in local SQLite/LanceDB; zero telemetry

---

## Tech Stack

| Layer               | Technology                                 |
| ------------------- | ------------------------------------------ |
| Desktop shell       | Tauri 2.x (Rust)                           |
| UI framework        | React 19, TypeScript 6                     |
| Routing             | TanStack Router 1.x (file-based)           |
| Server state        | TanStack Query 5.x                         |
| Client state        | Zustand 5                                  |
| Styling             | TailwindCSS v4 + CSS custom properties     |
| Animations          | motion/react (Framer Motion)               |
| Build system        | Vite 8 + Turbo (monorepo)                  |
| Package manager     | pnpm 11 (workspaces)                       |
| Local AI            | Ollama 0.6                                 |
| Relational DB       | SQLite via better-sqlite3 + Drizzle ORM    |
| Vector DB           | LanceDB                                    |
| Scraping            | Playwright + Cheerio                       |
| File processing     | pdfjs-dist, mammoth, Tesseract.js          |
| Document generation | docx-rs + printpdf (Rust) · jsPDF (client) |
| Validation          | Zod 4                                      |
| Logging             | Pino 10                                    |

---

## Installation

### Running a released build (macOS)

Download the latest `.dmg` from the [Releases](https://github.com/saeedkolivand/ai-job-hunter-assistant-app/releases) page and drag the app into your Applications folder.

Because the app is not notarized by Apple, macOS Gatekeeper may refuse to open it (e.g. "app is damaged and can't be opened"). Clear the quarantine attribute once after installing:

```bash
xattr -cr /Applications/AI\ Job\ Hunter\ Assistant.app
```

Then launch the app normally.

### Build from source

#### Prerequisites

| Requirement    | Version | Notes                                           |
| -------------- | ------- | ----------------------------------------------- |
| Node.js        | 20+     | LTS recommended                                 |
| pnpm           | 11+     | `npm install -g pnpm`                           |
| Rust toolchain | stable  | `rustup install stable`                         |
| Ollama         | latest  | [ollama.com](https://ollama.com) — for local AI |

#### Clone and install

```bash
git clone https://github.com/saeedkolivand/ai-job-hunter-assistant-app.git
cd ai-job-hunter-assistant-app
pnpm install
```

#### Pull an AI model

```bash
ollama pull mistral
# or for better quality:
ollama pull llama3.2
```

#### Start in development mode

```bash
pnpm dev
```

This launches the full Tauri desktop app with hot-module reloading on the React side.

---

## Usage Examples

### Generate a cover letter

```
1. Open the app → AI Generate
2. Paste your resume text, or upload a PDF/DOCX/TXT file
3. Paste the job ad text, or upload a job description file
4. Click Continue → app detects languages, role, company, top requirements
5. Choose a template and style, then click Generate → watch streaming output
6. Export as DOCX, PDF, or TXT
```

### Scrape job boards

```
1. Navigate to Jobs → Scrape
2. Select boards (e.g. LinkedIn + Greenhouse)
3. Enter search query, location, date filter
4. Start scrape → results stream into the jobs table
```

### Semantic search

```typescript
// Via IPC service hook
const { data } = useSearch({
  query: 'senior TypeScript engineer remote',
  collection: 'jobs',
  topK: 20,
  semanticWeight: 0.7,
});
```

### Set up Autopilot

```
1. Navigate to Autopilot → New Workflow
2. Step 1 – Target: board + salary + remote preference
3. Step 2 – Schedule: daily at 09:00
4. Step 3 – Action: attach resume, custom message
5. Save & Enable
```

---

## Configuration

The app uses the OS keychain for secrets — no `.env` files needed. API keys and credentials are stored via Settings → AI and encrypted using Tauri's keychain plugin.

| Setting          | Location            | Description                          |
| ---------------- | ------------------- | ------------------------------------ |
| AI provider      | Settings → AI       | Ollama / OpenAI / Anthropic / Gemini |
| API keys         | Settings → AI       | Stored in OS keychain                |
| Performance mode | Settings → General  | Low / Balanced / Performance         |
| Language         | Settings → General  | UI and generation locale             |
| Browser          | Settings → Scraping | Path to system browser               |

---

## Project Structure

```
ai-job-hunter-assistant-app/
├── apps/
│   └── tauri/                    # Main desktop app (Rust core + React renderer)
│       ├── src-tauri/            # Rust core (commands, scraping, DB)
│       └── src/renderer/         # React frontend
│           ├── features/         # Feature-scoped components
│           ├── routes/           # TanStack Router pages
│           ├── services/         # React Query IPC hooks
│           ├── lib/              # Utilities (motion, i18n, machine)
│           ├── store/            # Zustand stores
│           └── providers/        # React context providers
├── packages/
│   ├── shared/                   # IPC contracts, Zod schemas, shared types
│   ├── ui/                       # @ajh/ui — React component library
│   ├── core/                     # EventBus, JobQueue, Logger, RuntimeManager
│   ├── ai/                       # Ollama client + AI runtime
│   ├── data/                     # DB, matching, file processing
│   ├── prompts/                  # AI prompt templates
│   └── workers/                  # Web Workers (OCR, embeddings, chunking)
├── docs/                         # All project documentation
├── turbo.json                    # Turbo build configuration
├── pnpm-workspace.yaml           # pnpm workspaces
└── package.json                  # Root scripts
```

---

## Scripts

```bash
pnpm dev              # Start Tauri dev app (full stack)
pnpm dev:frontend     # Frontend-only Vite dev server
pnpm build            # Build all packages (Turbo)
pnpm build:packages   # Build packages only (excludes Tauri)
pnpm package          # Package desktop installers
pnpm typecheck        # TypeScript check across monorepo
pnpm test             # Run Vitest suite
pnpm lint             # ESLint across monorepo
pnpm lint:fix         # ESLint auto-fix
pnpm lint:strict      # Lint with --max-warnings 0 (CI mode)
pnpm format           # Prettier format
pnpm clean            # Clean build artifacts
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for branching strategy, commit conventions, code style, and PR guidelines.

Quick rules:

- All changes go through PRs — never push directly to `main`
- Use Conventional Commits (`feat:`, `fix:`, `chore:`, etc.)
- Run `pnpm lint:fix && pnpm typecheck` before pushing
- ESLint errors block commits — no `// eslint-disable` suppressions

---

## Documentation

| Document                                                   | Description                                            |
| ---------------------------------------------------------- | ------------------------------------------------------ |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)               | System design, data flow, Mermaid diagrams             |
| [docs/DESIGN_SYSTEM.md](docs/DESIGN_SYSTEM.md)             | Tokens, components, motion, theming                    |
| [docs/PATTERNS.md](docs/PATTERNS.md)                       | IPC, state machines, AI streaming, search patterns     |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)                 | Full local dev environment setup                       |
| [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)                   | Building and releasing installers                      |
| [docs/API.md](docs/API.md)                                 | All 21 IPC namespaces documented                       |
| [docs/ARCHITECTURE_STATUS.md](docs/ARCHITECTURE_STATUS.md) | Implementation status tracker                          |
| [docs/DESIGN_DECISIONS.md](docs/DESIGN_DECISIONS.md)       | Architecture decisions, patterns, and design rationale |
| [CONTRIBUTING.md](CONTRIBUTING.md)                         | Code style, branching, PR process                      |

---

## License

MIT — see [LICENSE](LICENSE) for details.
