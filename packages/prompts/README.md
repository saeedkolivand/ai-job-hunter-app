# `@ajh/prompts`

Pure, dependency-free TypeScript that builds the prompt strings and output
validators for the app's AI features. It **constructs strings and repairs model
output** — it never calls an LLM or the network.

## Provider-aware

Every prompt builder accepts a `PromptTarget` **additively** — either a legacy
tier string (`'large' | 'medium' | 'small'`) or a `ProviderProfile`:

```ts
interface ProviderProfile {
  kind: 'ollama' | 'cloud' | 'cli';
  model?: string; // raw model / tag name
  contextWindow?: number; // tokens, if known
  supportsStructuredOutput?: boolean; // native JSON schema / tool use
  sizeHint?: 'large' | 'medium' | 'small'; // ollama sub-tiering
}
```

`resolveProfile(target)` turns that into the four decisions a builder needs —
prompt **depth**, **schema** variant, **truncation** strategy, and whether the
caller can request **native structured output**:

| kind     | depth    | prompt                                          | truncation               | structured output           |
| -------- | -------- | ----------------------------------------------- | ------------------------ | --------------------------- |
| `ollama` | `brief`¹ | shortest, imperative, compact schema + one-shot | aggressive (by size)     | no                          |
| `cloud`  | `full`   | full multi-perspective + rich schema            | minimal (context window) | yes (`structuredOutputFor`) |
| `cli`    | `task`   | self-verifying task brief + acceptance checks   | moderate                 | no                          |

¹ ollama uses `full` only for a large local model. `detectModelSize` parses the
parameter size from the tag (`:1b`, `-3.2-1b`, `:7b`, `70b`, quant/instruct
suffixes) → `<4B small · 4–14B medium · >14B large`; unknown local models default
to the smaller/safer prompt.

`validateAndRepair` / `validateMetadata` remain the universal fallback for every
provider.

## Locale follows the job ad

All market behaviour keys off the **job-ad's detected locale** (`locale.ts`):
section-header lexicons (en, de, fr, es, it, nl, pt) for `detectSections`, resume
conventions (localized headers + date format), and a per-locale `estimateTokens`
factor. No default-to-German or default-to-English assumption.

## Layout

Every concern is a **folder** with an `index.ts` barrel (its `@ajh/prompts/<name>`
entry point — see `package.json` `exports`), its focused submodules, and a
colocated test. `src/index.ts` aggregates them into `@ajh/prompts`.

```
src/
  index.ts               aggregates everything → @ajh/prompts
  analyze/               schema · system-prompt · analysis-prompt · validate
  generate/              modes · emphasis · links · metadata · resume · cover-letter · text
  context-manager/       tokens · sections · truncation · model-size · multi-pass
  provider/              provider profiles, resolveProfile, JSON schemas
  locale/                section lexicons, resume conventions, token factors
  workspace/             workspace chat assistant system prompt
  fixtures/              shared test fixtures
```

## Scripts

```bash
pnpm --filter @ajh/prompts typecheck
pnpm --filter @ajh/prompts test
```
