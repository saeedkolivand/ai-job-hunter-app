# Installation

Install a released build, or build from source. For a first run in a few lines, see the [root README](../README.md#quick-start).

<details open>
<summary><strong>Download a released build</strong> (recommended)</summary>

Grab the latest installer for your OS from the **<a href="https://github.com/saeedkolivand/ai-job-hunter-app/releases" target="_blank" rel="noopener noreferrer">Releases</a>** page.

<a href="https://apps.microsoft.com/detail/9nc5kdjv0btm" target="_blank" rel="noopener noreferrer">
  <img src="https://get.microsoft.com/images/en-us%20dark.svg" width="200" alt="Download from the Microsoft Store" />
</a>
<a href="https://snapcraft.io/ai-job-hunter" target="_blank" rel="noopener noreferrer">
  <img src="https://snapcraft.io/en/dark/install.svg" width="182" alt="Get it from the Snap Store" />
</a>

**macOS**: open the `.dmg` and drag the app into Applications. Because the app isn't notarized by Apple, Gatekeeper may refuse to open it the first time ("app is damaged and can't be opened"). Clear the quarantine attribute once:

```bash
xattr -cr "/Applications/AI Job Hunter.app"
```

**Windows / Linux**: run the installer / AppImage from the Releases page.

</details>

<details>
<summary><strong>Homebrew (macOS)</strong></summary>

Releases ship signed macOS `.dmg`s, and the repo doubles as its own Homebrew tap (the <a href="../Casks/ai-job-hunter.rb" target="_blank" rel="noopener noreferrer">`Casks/ai-job-hunter.rb`</a> cask). Install with:

```bash
brew tap saeedkolivand/ai-job-hunter-app https://github.com/saeedkolivand/ai-job-hunter-app
brew install --cask ai-job-hunter
```

The cask clears the Gatekeeper quarantine flag for you (the app isn't notarized). Prefer a one-off download? Grab the `.dmg` straight from the <a href="https://github.com/saeedkolivand/ai-job-hunter-app/releases" target="_blank" rel="noopener noreferrer">Releases</a> page.

> The cask pins per-arch `sha256` checksums for a verified install, tracking the latest release that ships macOS `.dmg`s. Since the installer build is manual, bump the cask `version` + both checksums when a newer build publishes dmgs (`brew bump-cask-pr`).

</details>

<details>
<summary><strong>Build from source</strong></summary>

Prerequisites (Node.js, pnpm, Rust, Ollama, and the Windows build tools), clone and install, starting Ollama, and running the app are all in **[DEVELOPMENT.md](DEVELOPMENT.md)**. The short version is the root README's [Quick Start](../README.md#quick-start).

</details>

<details>
<summary><strong>Troubleshooting</strong></summary>

| Symptom                                       | Fix                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| macOS: _"app is damaged and can't be opened"_ | Not notarized. Clear quarantine once: `xattr -cr "/Applications/AI Job Hunter.app"`                                                                                                                                                                                                                                                                                                   |
| No models in the picker / "select a model"    | Start Ollama and `ollama pull <model>`, or add a cloud key in **Settings → AI**                                                                                                                                                                                                                                                                                                       |
| Company research does nothing                 | It's opt-in and uses your AI provider's own web search; on **Ollama** add an ollama.com account key (Settings → AI). No company brief: set a SearXNG-compatible server or an Exa key in the same place. "Search server refused JSON": enable `json` in that server's `formats`. Without a usable search the toggle reads off and generation proceeds without a brief                  |
| Scraping can't find a browser                 | The app auto-detects an installed Chromium browser (`detect_system_chrome`). If none is found, install Chrome or Edge, or set the `CHROME` env var to the browser executable path.                                                                                                                                                                                                    |
| `pnpm dev` fails to build the Rust core       | Ensure the stable Rust toolchain is installed (`rustup install stable`) and re-run                                                                                                                                                                                                                                                                                                    |
| Need to report a crash or bug                 | Open **Support → Contact/Feedback → Export Bundle**: saves `ajh-diagnostics-<date>.zip` (system info plus redacted crash/app logs; no résumés, API keys, job data, or database). Attach it to a new [GitHub issue](https://github.com/saeedkolivand/ai-job-hunter-app/issues/new). For security vulnerabilities use [SECURITY.md](../SECURITY.md) instead; do not post them publicly. |

</details>
