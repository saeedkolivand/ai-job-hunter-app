import {
  buildGitHubProjectsPrompt,
  buildGitHubProjectsSystemPrompt,
  parseGitHubProjects,
} from '@ajh/prompts/generate';
import type { GitHubRepo } from '@ajh/shared';

import { buildProviderProfile } from '../provider-context';
import { streamGenerate } from './stream';

/** A résumé-ready project entry produced from one GitHub repo. Exactly the shape
 *  the resume builder's `projects` field array appends. `link` is the repo's
 *  canonical URL, re-attached verbatim post-parse — NEVER written by the AI. */
export interface GeneratedGitHubProject {
  name: string;
  description: string;
  link: string;
  /** `·`-joined tech list, built from the repo's own metadata — see
   *  {@link repoTechnologies}. Never written by the AI. */
  technologies: string;
}

/** Cap mirroring `buildGitHubReposBlock`'s `MAX_TOPICS` / `MAX_TOPIC`
    (`packages/prompts/src/generate/github-projects`), so a repo with a hundred
    hostile topics cannot flood the résumé line either. */
const MAX_TECH_ITEMS = 10;
const MAX_TECH_ITEM_CHARS = 40;

/**
 * The project's technology list, taken STRAIGHT from the repo's own metadata
 * (primary language first, then topics) rather than from the model: the prompt
 * forbids the model from emitting anything except NAME/DESC, so asking it for
 * the tech list would add a fabrication surface for data the caller already
 * holds. Repo topics are untrusted, attacker-influenceable text, so they are
 * capped and land in an editable form field the candidate reviews.
 */
function repoTechnologies(repo: GitHubRepo): string {
  const seen = new Set<string>();
  const items: string[] = [];
  for (const raw of [repo.language, ...(repo.topics ?? [])]) {
    const item = (raw ?? '').replace(/\s+/g, ' ').trim().slice(0, MAX_TECH_ITEM_CHARS);
    const key = item.toLowerCase();
    if (!item || seen.has(key)) continue;
    seen.add(key);
    items.push(item);
    if (items.length === MAX_TECH_ITEMS) break;
  }
  return items.join(' · ');
}

/** De-slug a repo name for the offline fallback title ("my-cool-app" → "My Cool App"). */
function deslugRepoName(name: string): string {
  return name
    .replace(/[-_./]+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** Normalize a title/repo name to a match key: de-slug, lowercase, drop every
 *  non-alphanumeric. So "my-cool-app", "My Cool App", and "My_Cool_App" all key
 *  to "mycoolapp" — robust to the model de-slugging or re-spacing the title. */
function projectNameKey(name: string): string {
  return deslugRepoName(name)
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '');
}

/**
 * Turn selected GitHub repos into résumé-ready project entries via the AI provider.
 * Routes through the same streaming pipeline as the other generators and the
 * untrusted-data fence (a hostile repo description can't steer the model). The
 * model writes the title + bullets only; each repo's `htmlUrl` is re-attached as
 * `link` AFTER parsing — the AI never writes a URL.
 *
 * Resilient by design: if streaming/parsing throws, or yields fewer entries than
 * repos, the missing repos fall back to their raw `description` (or de-slugged
 * name) so import ALWAYS works — even offline or with no provider configured.
 *
 * Each parsed entry is matched back to its repo by de-slugged NAME (case/space/
 * hyphen-insensitive), so a correct bullet lands on the right repo's link even if
 * the model reorders or renames blocks; only entries with no name match fall back
 * to positional pairing. Output is one entry per repo, in input order.
 */
export async function generateGitHubProjects(params: {
  repos: GitHubRepo[];
  model: string;
  signal?: AbortSignal;
  onToken?: (tok: string) => void;
}): Promise<GeneratedGitHubProject[]> {
  const { repos, model, signal, onToken } = params;
  if (!repos.length) return [];

  let parsed: { name: string; description: string }[] = [];
  try {
    const profile = buildProviderProfile(model);
    const system = buildGitHubProjectsSystemPrompt();
    // Map the IPC repo shape → the prompt's URL-free input (the AI never sees a link).
    const user = buildGitHubProjectsPrompt(
      repos.map((r) => ({
        name: r.name,
        description: r.description,
        language: r.language,
        topics: r.topics,
        stars: r.stars,
        pushedAt: r.pushedAt,
      })),
      profile
    );
    // Factual bullets extracted from real repo data, landing in the résumé's
    // `projects` field — `deterministic`, keyed off `resume`.
    const raw = await streamGenerate(model, system, user, 'resume', 'deterministic', {
      onToken,
      locale: 'en',
      signal,
    });
    // Parse the RAW stream, NOT extractPlainText(raw): extractPlainText deletes a
    // whole ```-fenced answer entirely, which a local model often emits — that
    // would silently drop every AI entry to the fallback. The parser strips
    // fences + inline markdown itself.
    parsed = parseGitHubProjects(raw);
  } catch {
    // No provider / offline / aborted-after-partial — fall back for every repo.
    parsed = [];
  }

  // name → entry index, so each entry is consumed at most once.
  const byName = new Map<string, number>();
  parsed.forEach((entry, i) => {
    const key = projectNameKey(entry.name);
    if (key && !byName.has(key)) byName.set(key, i);
  });
  const used = new Array<boolean>(parsed.length).fill(false);

  return repos.map((repo, i) => {
    // Prefer a name match; fall back to the positional entry only if it is not
    // already claimed by another repo's name match.
    const nameIdx = byName.get(projectNameKey(repo.name));
    let entry: { name: string; description: string } | undefined;
    if (nameIdx !== undefined && !used[nameIdx]) {
      entry = parsed[nameIdx];
      used[nameIdx] = true;
    } else if (!used[i]) {
      entry = parsed[i];
      if (entry) used[i] = true;
    }

    const description = entry?.description.trim();
    const matched = Boolean(description);
    return {
      name: matched ? entry?.name.trim() || deslugRepoName(repo.name) : deslugRepoName(repo.name),
      // Offline fallback: the real description, or the de-slugged name when empty.
      description: description || repo.description?.trim() || deslugRepoName(repo.name),
      // Link is ALWAYS the repo's own URL — never the AI, never the matched entry.
      link: repo.htmlUrl,
      technologies: repoTechnologies(repo),
    };
  });
}
