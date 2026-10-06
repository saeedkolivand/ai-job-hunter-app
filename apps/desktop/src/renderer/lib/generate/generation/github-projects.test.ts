import { describe, expect, it, vi } from 'vitest';

import type { GitHubRepo } from '@ajh/shared';

import { _registerClient } from '../../app-client';
import { createMockClient } from '../../mock-client';
import { generateGitHubProjects } from './generation';
import {
  done,
  emit,
  flushUntilStreaming,
  installGenerationHooks,
  register,
  streamThrough,
} from './test-support';

installGenerationHooks();

describe('generateGitHubProjects', () => {
  const REPOS: GitHubRepo[] = [
    {
      name: 'merry-oasis',
      description: 'A local-first task planner.',
      htmlUrl: 'https://github.com/me/merry-oasis',
      language: 'TypeScript',
      topics: ['offline-first'],
      stars: 42,
    },
    {
      name: 'tiny-parser',
      description: 'Zero-dependency JSON parser.',
      htmlUrl: 'https://github.com/me/tiny-parser',
      language: 'Rust',
      topics: [],
      stars: 0,
    },
  ];

  it('parses AI entries and re-attaches each repo url as link (AI never writes the url)', async () => {
    const client = register();
    const out = await streamThrough(
      generateGitHubProjects({ repos: REPOS, model: 'llama3' }),
      [
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner • Implemented offline sync',
        '',
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser in Rust',
      ].join('\n')
    );

    expect(out).toHaveLength(2);
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'Built a local-first task planner • Implemented offline sync',
      link: 'https://github.com/me/merry-oasis',
      technologies: 'TypeScript · offline-first',
    });
    // Link is the repo's verbatim htmlUrl, in input order.
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
    // The repo metadata is fenced as untrusted in the prompt.
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<github_repos>'),
          }),
        ]),
      })
    );
  });

  it('falls back to the raw repo description + link for every repo when streaming throws', async () => {
    // generatePipeline rejects → streamGenerate throws → fallback path.
    const client = createMockClient({
      ai: { generatePipeline: vi.fn().mockRejectedValue(new Error('no provider configured')) },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);

    const out = await generateGitHubProjects({ repos: REPOS, model: 'llama3' });

    expect(out).toHaveLength(2);
    // Each entry falls back to the de-slugged name + raw description, with the link.
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'A local-first task planner.',
      link: 'https://github.com/me/merry-oasis',
      technologies: 'TypeScript · offline-first',
    });
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
  });

  it('builds technologies from repo metadata only — deduped, capped, never from the AI', async () => {
    const client = createMockClient({
      ai: { generatePipeline: vi.fn().mockRejectedValue(new Error('no provider configured')) },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);

    const noisy: GitHubRepo[] = [
      {
        name: 'kitchen-sink',
        description: 'Everything.',
        htmlUrl: 'https://github.com/me/kitchen-sink',
        language: 'Rust',
        topics: [
          '  rust  ', // duplicate of the language, case- and space-insensitive
          'cli',
          'a'.repeat(60), // over the per-item cap
          '',
          't4',
          't5',
          't6',
          't7',
          't8',
          't9',
          't10',
          't11', // past the 10-item cap
        ],
        stars: 0,
      },
    ];

    const [out] = await generateGitHubProjects({ repos: noisy, model: 'llama3' });
    const items = (out?.technologies ?? '').split(' · ');

    expect(items).toEqual([
      'Rust',
      'cli',
      'a'.repeat(40),
      't4',
      't5',
      't6',
      't7',
      't8',
      't9',
      't10',
    ]);
    // The model was never asked for, and never supplied, a technology list.
    expect(out?.technologies).not.toContain('t11');
  });

  it('fills missing entries from the fallback when the model returns fewer than the repos', async () => {
    register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    // Only the first repo gets an AI entry.
    emit('NAME: Merry Oasis\nDESC: Built a local-first task planner');
    done();
    const out = await p;

    expect(out).toHaveLength(2);
    expect(out[0]?.description).toBe('Built a local-first task planner');
    // Second repo falls back to its raw description, link still attached.
    expect(out[1]).toEqual({
      name: 'Tiny Parser',
      description: 'Zero-dependency JSON parser.',
      link: 'https://github.com/me/tiny-parser',
      technologies: 'Rust',
    });
  });

  it('survives a whole-response wrapped in one code fence (does not fall back)', async () => {
    // A local model wraps its entire answer in a single ``` block. extractPlainText
    // would delete it, dropping every AI entry to the raw-description fallback; the
    // wrapper parses the raw stream so the AI entries survive.
    register();
    const out = await streamThrough(
      generateGitHubProjects({ repos: REPOS, model: 'llama3' }),
      [
        '```',
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner',
        '',
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser',
        '```',
      ].join('\n')
    );

    // AI output survived — NOT the raw GitHub descriptions.
    expect(out[0]?.description).toBe('Built a local-first task planner');
    expect(out[1]?.description).toBe('Wrote a zero-dependency JSON parser');
    expect(out[0]?.link).toBe('https://github.com/me/merry-oasis');
  });

  it('matches each AI entry to its repo by name so links never cross when blocks reorder', async () => {
    register();
    const p = generateGitHubProjects({ repos: REPOS, model: 'llama3' });
    await flushUntilStreaming();
    // Model emits the blocks in REVERSE order (tiny-parser first).
    emit(
      [
        'NAME: Tiny Parser',
        'DESC: Wrote a zero-dependency JSON parser',
        '',
        'NAME: Merry Oasis',
        'DESC: Built a local-first task planner',
      ].join('\n')
    );
    done();
    const out = await p;

    // Output stays in INPUT order, and each description lands on the repo whose
    // link it belongs to — positional pairing would have crossed them.
    expect(out[0]).toEqual({
      name: 'Merry Oasis',
      description: 'Built a local-first task planner',
      link: 'https://github.com/me/merry-oasis',
      technologies: 'TypeScript · offline-first',
    });
    expect(out[1]).toEqual({
      name: 'Tiny Parser',
      description: 'Wrote a zero-dependency JSON parser',
      link: 'https://github.com/me/tiny-parser',
      technologies: 'Rust',
    });
  });

  it('returns [] without calling the provider for an empty repo list', async () => {
    const client = register();
    expect(await generateGitHubProjects({ repos: [], model: 'llama3' })).toEqual([]);
    expect(client.ai.generatePipeline).not.toHaveBeenCalled();
  });

  // ── item 5: fallbackProject with absent description ───────────────────────

  it('fallback uses de-slugged repo name when description is absent', async () => {
    // A repo with NO description field — not even null — must fall back to the
    // de-slugged name, not '' or undefined. "my-cool-app" → "My Cool App".
    const repoNoDesc: GitHubRepo = {
      name: 'my-cool-app',
      htmlUrl: 'https://github.com/u/my-cool-app',
      language: 'TypeScript',
      topics: [],
      stars: 0,
      // description intentionally absent
    };
    const client = createMockClient({
      ai: { generatePipeline: vi.fn().mockRejectedValue(new Error('no provider')) },
      jobs: { get: vi.fn().mockResolvedValue(null), cancel: vi.fn() },
    });
    _registerClient(client);

    const out = await generateGitHubProjects({ repos: [repoNoDesc], model: 'llama3' });

    expect(out).toHaveLength(1);
    // Description falls back to de-slugged name (not '' or undefined).
    expect(out[0]?.description).toBe('My Cool App');
    // Link is always the repo's own htmlUrl.
    expect(out[0]?.link).toBe('https://github.com/u/my-cool-app');
  });

  // ── item 6: name-match dedup guard — two repos normalizing to same key ────

  it('dedup guard: two repos with same normalized key each keep their own link', async () => {
    // "my-app" and "My App" both normalize to "myapp". The model returns ONE
    // matching block ("My App"). The second repo must NOT claim the first entry
    // twice — it must fall back to its own link via the positional or fallback path.
    const repoA: GitHubRepo = {
      name: 'my-app',
      description: 'Slug version',
      htmlUrl: 'https://github.com/u/my-app',
      language: 'TypeScript',
      topics: [],
      stars: 5,
    };
    const repoB: GitHubRepo = {
      name: 'My App',
      description: 'Space version',
      htmlUrl: 'https://github.com/u/My-App',
      language: 'Rust',
      topics: [],
      stars: 2,
    };

    register();
    const p = generateGitHubProjects({ repos: [repoA, repoB], model: 'llama3' });
    await flushUntilStreaming();
    // Model emits only ONE block that matches the normalized key "myapp".
    emit('NAME: My App\nDESC: AI-generated bullet for the app');
    done();
    const out = await p;

    expect(out).toHaveLength(2);
    // First repo claimed the name-match entry — gets the AI description.
    expect(out[0]?.link).toBe('https://github.com/u/my-app');
    // Second repo must NOT re-claim the same entry — gets its own link via fallback.
    expect(out[1]?.link).toBe('https://github.com/u/My-App');
    // Links must not cross.
    expect(out[0]?.link).not.toBe(out[1]?.link);
  });

  // ── item 7: pre-aborted AbortSignal returns fallback array, no throw ──────

  it('pre-aborted signal returns raw-description fallback array without throwing', async () => {
    // An already-cancelled AbortSignal: generateGitHubProjects must return one
    // fallback entry per repo (with link), not throw.
    register();

    const controller = new AbortController();
    controller.abort();

    const out = await generateGitHubProjects({
      repos: REPOS,
      model: 'llama3',
      signal: controller.signal,
    });

    // Must return one entry per repo.
    expect(out).toHaveLength(2);
    // Each entry has the repo's own htmlUrl as link.
    expect(out[0]?.link).toBe('https://github.com/me/merry-oasis');
    expect(out[1]?.link).toBe('https://github.com/me/tiny-parser');
    // Both entries have non-empty descriptions (fallback to raw description).
    expect(out[0]?.description).toBeTruthy();
    expect(out[1]?.description).toBeTruthy();
  });
});
