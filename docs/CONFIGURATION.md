# Configuration

AI providers and app settings. For the full list of capabilities, see [FEATURES.md](FEATURES.md).

## AI Provider Flexibility

Switch providers at any time in **Settings → AI**:

| Provider               | Models                                           | Notes                                                                     |
| ---------------------- | ------------------------------------------------ | ------------------------------------------------------------------------- |
| **Ollama** (local)     | mistral, llama3.2, deepseek-r1, any Ollama model | No API key needed; fully offline; per-model context/output limits         |
| **OpenAI**             | GPT-4o, o-series, GPT-4 Turbo                    | Requires API key                                                          |
| **Anthropic**          | Claude (Sonnet / Opus), extended thinking        | Requires API key; reasoning streamed to the thinking view                 |
| **Google Gemini**      | Gemini 2.5 / 1.5 (Pro, Flash)                    | Requires API key; thinking models supported                               |
| **OpenAI-compatible**  | Any (LM Studio, vLLM, remote Ollama, …)          | Custom base URL                                                           |
| **CLI agents** (local) | Claude Code, Codex, Gemini CLI                   | Run headless via the installed CLI: no API key (uses the CLI's own login) |

API keys are stored encrypted in the OS keychain. CLI agents run as a headless subprocess and reuse whatever login that CLI already has, so they need no key in the app.

## Configuration

The app uses the OS keychain for secrets instead of `.env` files. Keys and credentials are set in the UI and encrypted via Tauri's keychain plugin.

| Setting                 | Location               | Description                                                                                                                                                                                                  |
| ----------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| AI provider + key       | Settings → AI          | Ollama / OpenAI / Anthropic / Gemini / compatible                                                                                                                                                            |
| Local model limits      | Settings → AI          | Context window + max output, per Ollama model                                                                                                                                                                |
| Ollama account key      | Settings → AI          | Optional: Ollama Cloud models + company research                                                                                                                                                             |
| **Adzuna/JSearch keys** | **Settings → Jobs**    | **Optional: provider registry with Adzuna (primary, free) and JSearch (paid fallback). Covers anti-bot boards like Indeed, Glassdoor, Xing, and more. See `docs/knowledge/scraping-domain.md` for details.** |
| Performance mode        | Settings → Performance | Low / Balanced / Performance                                                                                                                                                                                 |
| Language                | Settings → General     | UI and generation locale                                                                                                                                                                                     |
