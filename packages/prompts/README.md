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

| kind     | depth    | prompt                                          | truncation               | structured output² |
| -------- | -------- | ----------------------------------------------- | ------------------------ | ------------------ |
| `ollama` | `brief`¹ | shortest, imperative, compact schema + one-shot | aggressive (by size)     | no                 |
| `cloud`  | `full`   | full multi-perspective + rich schema            | minimal (context window) | yes                |
| `cli`    | `task`   | self-verifying task brief + acceptance checks   | moderate                 | no                 |

¹ ollama uses `full` only for a large local model. `detectModelSize` parses the
parameter size from the tag (`:1b`, `-3.2-1b`, `:7b`, `70b`, quant/instruct
suffixes) → `<4B small · 4–14B medium · >14B large`; unknown local models default
to the smaller/safer prompt.

² The **structured output** column is the _default_ per `kind`; `resolveProfile`
computes it as `profile.supportsStructuredOutput ?? profile.kind === 'cloud'`, so any
profile can override it — a cloud model without native JSON-schema/tool-use → `false`, a
capable local model → `true`.

`validateAndRepair` / `validateMetadata` remain the universal fallback for every
provider.

## Locale follows the job ad

All market behaviour keys off the **job-ad's detected locale** (`locale.ts`):
section-header lexicons (en, de, fr, es, it, nl, pt) for `detectSections`, resume
conventions (localized headers + date format), and a per-locale `estimateTokens`
factor. No default-to-German or default-to-English assumption.

## Layout

Each concern is a **folder** with focused submodules and a colocated test.
`src/index.ts` aggregates the public API into `@ajh/prompts`.

Public entry points (exported in `package.json` `exports`):

| Import                         | Folder                 |
| ------------------------------ | ---------------------- |
| `@ajh/prompts`                 | `src/index.ts`         |
| `@ajh/prompts/generate`        | `src/generate/`        |
| `@ajh/prompts/analyze`         | `src/analyze/`         |
| `@ajh/prompts/builder`         | `src/builder/`         |
| `@ajh/prompts/context-manager` | `src/context-manager/` |
| `@ajh/prompts/provider`        | `src/provider/`        |

Internal folders (not exported — no `@ajh/prompts/<name>` entry point):

```
src/
  locale/                section lexicons, resume conventions, token factors
  workspace/             workspace chat assistant system prompt
  fixtures/              shared test fixtures
```

## Scripts

```bash
pnpm --filter @ajh/prompts typecheck
pnpm --filter @ajh/prompts test
```
