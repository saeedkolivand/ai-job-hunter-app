# AI Job Hunter — Desktop

A local-first, AI-native desktop application for intelligent job hunting, semantic resume matching, and multilingual document understanding.

> **Single Electron application. No backend. No server. Everything runs locally.**

## Features

- **AI-Powered Job Generation**: Generate tailored cover letters, emails, and responses using local LLMs
- **Semantic Resume Matching**: ATS scoring combined with semantic vector similarity for better job matching
- **Multi-Board Scraping**: Scrape job postings from multiple job boards automatically
- **Document Understanding**: Extract and analyze resumes, job descriptions, and cover letters (PDF, DOCX, TXT, MD)
- **Multilingual Support**: Process and generate content in multiple languages
- **Local AI Inference**: All AI processing runs locally via Ollama - no API keys, no cloud dependency
- **Vector Search**: Semantic search through job postings and documents using LanceDB
- **Credential Management**: Securely store job board credentials (encrypted OS keychain)
- **Job Application Automation**: Apply to jobs automatically with configurable workflows

## Quick Start

Get the app running in 3 simple steps:

### 1. Install Prerequisites

- **Node.js** 20.11.0+ [Download](https://nodejs.org/)
- **pnpm** 9.0.0+ (if not installed, run: `npm install -g pnpm`)
- **Ollama** [Download](https://ollama.com) - Required for AI features

### 2. Install Dependencies

```bash
# Clone the repository
git clone https://github.com/saeedkolivand/ai-job-hunter-assistant-app.git
cd ai-job-hunter-assistant-app

# Install dependencies
pnpm install
```

### 3. Start Ollama and Run the App

```bash
# In one terminal, start Ollama
ollama serve

# In another terminal, pull a model (first time only)
ollama pull llama3.2

# Run the app
pnpm dev
```

**Important for WSL users**: Run `pnpm dev` from Windows PowerShell/CMD, not from WSL, as Electron requires a display server.

## Tech Stack

### Core

- **Electron** 42.1.0 - Desktop application framework
- **React** 19.2.6 - UI framework
- **TypeScript** 6.0.3 - Type safety
- **Vite** 8.0.13 - Fast build tool
- **pnpm** 11.1.2 - Package manager (workspace)

### UI

- **TailwindCSS** v4 - Styling
- **shadcn/ui** - Component library
- **motion/react** - Animations
- **Lucide** - Icons
- **TanStack Router** - File-based routing
- **TanStack Query** - Server state management
- **Zustand** - Client state management
- **Zod** - Schema validation

### AI & Data

- **Ollama** - Local LLM inference
- **LanceDB** - Vector database
- **SQLite** (better-sqlite3) - Relational database
- **Drizzle ORM** - Database ORM

### File Processing

- **pdfjs-dist** - PDF parsing
- **mammoth** - DOCX parsing
- **tesseract.js** - OCR

### Scraping

- **Playwright** - Browser automation
- **cheerio** - HTML parsing
- **undici** - HTTP client

### Infrastructure

- **Pino** - Logging
- **Vitest** - Unit testing
- **Playwright** - E2E testing

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Renderer Process                         │
│  React UI + TanStack Router + Zustand + TanStack Query          │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                    Preload Bridge (IPC)
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                        Main Process (Thin)                       │
│  Lifecycle • Windows • Menus • IPC Routing • Tray               │
└──────────────────────────┬──────────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────────┐
│                      Application Core                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Event Bus   │  │ Job Queue    │  │  Task Scheduler      │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Runtime Manager                              │  │
│  └──────────────────────┬───────────────────────────────────┘  │
│                         │                                       │
│  ┌──────────────────────┴───────────────────────────────────┐  │
│  │              State Coordinator                            │  │
│  └──────────────────────────────────────────────────────────┘  │
└──────────────────────────┬──────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
        ┌────────────┐          ┌────────────┐
        │ AI Runtime │          │Data Runtime│
        │  Ollama    │          │  SQLite    │
        │  Embeddings│          │  LanceDB   │
        │  Chat      │          │  Scraping  │
        └────────────┘          │  Matching  │
                                └────────────┘
```

**Design Philosophy**: The Main process is intentionally thin - it handles lifecycle, IPC routing, and orchestration only. All heavy work (AI, OCR, scraping, embeddings, indexing) goes through the Core's job system into specialized runtimes and worker pools.

## Workspace Structure

```
ai-job-hunter-assistant-app/
├── apps/
│   └── desktop/              # Electron application
│       ├── src/
│       │   ├── main/         # Main process (IPC, windows, bootstrap)
│       │   ├── preload/      # Preload bridge (contextBridge)
│       │   └── renderer/     # React UI (routes, components, stores)
│       └── electron.vite.config.ts
├── packages/
│   ├── shared/              # Shared types, IPC contracts, Zod schemas
│   ├── core/                # Event Bus, Job Queue, Scheduler, Runtime Manager
│   ├── ai/                  # Ollama runtime (chat, embeddings, model lifecycle)
│   ├── data/                # Data runtime (SQLite, LanceDB, scraping, matching)
│   └── workers/             # Worker thread pool (OCR, embeddings, chunking)
├── electron-builder.yml      # Electron builder configuration
├── package.json             # Root package.json (workspace scripts)
├── pnpm-workspace.yaml      # pnpm workspace configuration
└── tsconfig.base.json       # Base TypeScript configuration
```

## Prerequisites

- **Node.js**: 20.11.0+ (required by engines)
- **pnpm**: 9.0.0+ (required by engines)
- **Ollama**: Download from [ollama.com](https://ollama.com)
  - Start Ollama: `ollama serve`
  - Pull a model: `ollama pull llama3.2` or `ollama pull mistral`

## Installation

```bash
# Clone the repository
git clone https://github.com/saeedkolivand/ai-job-hunter-assistant-app.git
cd ai-job-hunter-assistant-app

# Install dependencies
pnpm install
```

## Running the Application

### Development Mode

```bash
pnpm dev
```

This launches Electron with hot module replacement (HMR) for the renderer process.

**Important**: If running from WSL, you must run from Windows PowerShell/CMD instead, as Electron requires a display server.

### Building for Production

```bash
pnpm build          # Build all packages
pnpm package       # Create platform-specific installers
```

Installers are created in `apps/desktop/dist/`:

- **Windows**: NSIS installer
- **macOS**: DMG and ZIP
- **Linux**: AppImage and DEB

## Ollama Setup

### Windows (Native)

1. Install Ollama from [ollama.com](https://ollama.com)
2. Ollama runs as a Windows service automatically
3. Pull a model: `ollama pull llama3.2`
4. The app auto-detects Ollama at `http://127.0.0.1:11434`

### WSL (Windows Subsystem for Linux)

The app auto-detects WSL and connects to Ollama running on Windows:

1. Install Ollama on **Windows** (not inside WSL)
2. Start Ollama on Windows
3. Pull a model: `ollama pull llama3.2`
4. The app automatically detects the Windows host IP

**If auto-detection fails**, set `OLLAMA_HOST`:

```bash
# Get your Windows host IP
cat /etc/resolv.conf | grep nameserver | awk '{print $2}'

# Set OLLAMA_HOST
export OLLAMA_HOST=http://<windows-ip>:11434

# Run from Windows PowerShell/CMD (not WSL)
cd C:\Users\<your-user>\js_projects\ai-job-hunter-assistant-app
pnpm dev
```

### Linux/Mac

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Start Ollama
ollama serve

# Pull a model
ollama pull llama3.2

# Run the app
pnpm dev
```

## Development

```bash
pnpm dev          # Development mode with HMR
pnpm build        # Build all packages
pnpm typecheck    # TypeScript type checking across workspace
pnpm lint         # Lint all packages
pnpm test         # Run unit tests
pnpm clean        # Clean build artifacts and node_modules
```

## Project Status

**Phase 1 Complete**: Architectural scaffold and core infrastructure are implemented. Runtime implementations include typed stubs ready for feature development.

**Implemented**:

- ✅ Workspace structure and build system
- ✅ Core infrastructure (Event Bus, Job Queue, Scheduler, Runtime Manager)
- ✅ IPC layer with typed contracts
- ✅ AI runtime with Ollama integration
- ✅ Data runtime with SQLite and LanceDB
- ✅ Basic UI structure with routing
- ✅ Ollama model detection and host configuration

**In Progress**:

- 🚧 Scraping implementations for job boards
- 🚧 Resume matching engine
- 🚧 Document processing pipeline
- 🚧 AI generation features
- 🚧 Job application automation

## Troubleshooting

### "No models available" in AI Settings

- Verify Ollama is running: `ollama list`
- Check browser console (F12) for `[Ollama]` log messages
- If in WSL, set `OLLAMA_HOST` manually (see above)
- Ensure Ollama is listening on port 11434

### "Electron uninstall" Error

If you encounter an "Electron uninstall" error when running `pnpm dev`, the electron binary may not have been downloaded properly during installation. This can happen in pnpm workspaces.

**Solution**: Manually trigger the electron binary download:

```bash
node node_modules/electron/install.js
```

Then run the dev server again:

```bash
pnpm dev
```

### "Electron uninstall" Error (WSL)

Electron requires a display server and won't run in headless WSL. Run from Windows instead:

```powershell
# In Windows PowerShell or CMD
cd C:\Users\<your-user>\js_projects\ai-job-hunter-assistant-app
pnpm dev
```

### Build Errors

```bash
# Clean everything and reinstall
pnpm clean
pnpm install
pnpm build
```

### IPC Errors

- Verify preload script is built: `apps/desktop/out/preload/index.js`
- Rebuild if missing: `rm -rf apps/desktop/out && pnpm dev`

## License

UNLICENSED
