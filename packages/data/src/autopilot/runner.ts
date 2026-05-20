/**
 * Autopilot runner — scrape → filter → apply loop.
 */
import type { JobPosting, Autopilot, AutopilotRun } from '@ajh/shared';
import type { ScraperRegistry } from '../scraping/registry.js';
import type { ApplierRegistry } from '../applying/registry.js';
import type { BrowserController } from '../scraping/browser.js';
import type { AutopilotStore } from './store.js';

export interface RunnerCredentials {
  get(boardId: string): Promise<{ username: string; password: string } | null>;
  storageStatePath(boardId: string): string;
}

export interface RunnerContext {
  signal: AbortSignal;
  stream(data: unknown): void;
  setProgress(p: number): void;
  logger: {
    info(obj: object, msg: string): void;
    error(obj: object, msg: string): void;
    warn(obj: object, msg: string): void;
  };
}

function passesKeywords(posting: JobPosting, filter: Autopilot['filter']): boolean {
  const text = `${posting.title} ${posting.description}`.toLowerCase();
  if (filter.keywords?.length) {
    if (!filter.keywords.every((kw) => text.includes(kw.toLowerCase()))) return false;
  }
  if (filter.excludeKeywords?.length) {
    if (filter.excludeKeywords.some((kw) => text.includes(kw.toLowerCase()))) return false;
  }
  return true;
}

function simpleMatchScore(resumeText: string, posting: JobPosting): number {
  if (!resumeText) return 100;
  const resumeWords = new Set(resumeText.toLowerCase().match(/\b\w{4,}\b/g) ?? []);
  const jobWords =
    (posting.description + ' ' + (posting.requirements ?? []).join(' '))
      .toLowerCase()
      .match(/\b\w{4,}\b/g) ?? [];
  if (!jobWords.length) return 50;
  const hits = jobWords.filter((w) => resumeWords.has(w)).length;
  return Math.round((hits / jobWords.length) * 100);
}

export async function runAutopilot(
  autopilot: Autopilot,
  scrapers: ScraperRegistry,
  appliers: ApplierRegistry,
  browser: BrowserController,
  store: AutopilotStore,
  credentials: RunnerCredentials,
  jobId: string,
  ctx: RunnerContext
): Promise<AutopilotRun> {
  const run: AutopilotRun = {
    autopilotId: autopilot._id,
    jobId,
    startedAt: Date.now(),
    found: 0,
    matched: 0,
    applied: 0,
    skipped: 0,
    errors: [],
  };

  const emit = (kind: string, payload: object) =>
    ctx.stream({ kind, autopilotId: autopilot._id, ...payload });

  emit('autopilot.started', { name: autopilot.name });
  ctx.logger.info(
    { autopilotId: autopilot._id, board: autopilot.target.board },
    'autopilot run started'
  );

  // ── 1. Scrape ──────────────────────────────────────────────────────────────
  const scraper = scrapers.get(autopilot.target.board);
  if (!scraper) {
    const msg = `No scraper for board: ${autopilot.target.board}`;
    run.errors.push(msg);
    emit('autopilot.error', { message: msg });
    return run;
  }

  ctx.setProgress(0.05);
  emit('autopilot.phase', { phase: 'scraping', message: `Searching ${autopilot.target.board}…` });

  const credAccessor = {
    get: (id: string) => credentials.get(id),
    storageStatePath: (id: string) => credentials.storageStatePath(id),
  };

  const postings: JobPosting[] = [];
  try {
    await scraper.search(
      {
        query: autopilot.target.query,
        ...(autopilot.target.location ? { location: autopilot.target.location } : {}),
        pages: autopilot.target.pages,
        ...(autopilot.target.dateFilter ? { dateFilter: autopilot.target.dateFilter } : {}),
      },
      {
        signal: ctx.signal,
        onProgress: (p) => ctx.setProgress(0.05 + p * 0.45),
        onItem: (item) => {
          postings.push(item as JobPosting);
          emit('autopilot.posting_found', { title: item.title, company: item.company });
        },
        browser,
        credentials: credAccessor,
      }
    );
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    run.errors.push(`Scrape failed: ${msg}`);
    ctx.logger.error({ err }, 'autopilot scrape failed');
    emit('autopilot.error', { message: `Scrape failed: ${msg}` });
  }

  run.found = postings.length;
  ctx.setProgress(0.5);
  emit('autopilot.phase', {
    phase: 'filtering',
    message: `Found ${run.found} postings, filtering…`,
  });

  // ── 2. Filter ──────────────────────────────────────────────────────────────
  const qualifying: JobPosting[] = [];
  for (const posting of postings) {
    if (ctx.signal.aborted) break;
    if (!passesKeywords(posting, autopilot.filter)) {
      run.skipped++;
      continue;
    }
    const score = simpleMatchScore(autopilot.resumeText ?? '', posting);
    if (score < autopilot.filter.minMatchScore) {
      run.skipped++;
      emit('autopilot.posting_skipped', {
        title: posting.title,
        reason: `score ${score} < ${autopilot.filter.minMatchScore}`,
      });
      continue;
    }
    run.matched++;
    qualifying.push(posting);
    emit('autopilot.posting_matched', { title: posting.title, company: posting.company, score });
  }

  ctx.setProgress(0.6);
  emit('autopilot.phase', {
    phase: 'acting',
    message: `${run.matched} matched, action: ${autopilot.action}`,
  });

  // ── 3. Act ─────────────────────────────────────────────────────────────────
  if (autopilot.action !== 'save') {
    const applier = appliers.get(autopilot.target.board);
    if (!applier) {
      const msg = `No applier for board: ${autopilot.target.board}`;
      run.errors.push(msg);
      emit('autopilot.error', { message: msg });
    } else {
      const total = qualifying.length;
      for (let i = 0; i < qualifying.length; i++) {
        if (ctx.signal.aborted) break;
        const posting = qualifying[i];
        if (!posting) continue;
        ctx.setProgress(0.6 + (i / total) * 0.38);
        emit('autopilot.applying', {
          title: posting.title,
          company: posting.company,
          index: i + 1,
          total,
        });
        try {
          const cred = await credentials.get(autopilot.target.board);
          await applier.apply(posting.url, {
            signal: ctx.signal,
            browser,
            storageStatePath: credentials.storageStatePath(autopilot.target.board),
            credentials: cred,
            coverLetter: autopilot.coverLetter,
            autoSubmit: autopilot.action === 'auto_apply' && autopilot.autoSubmit,
            onProgress: (p, stage) =>
              emit('autopilot.apply_progress', { title: posting.title, stage, p }),
            onStep: (step) => emit('autopilot.apply_step', { title: posting.title, ...step }),
          });
          run.applied++;
          emit('autopilot.applied', { title: posting.title, company: posting.company });
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          run.errors.push(`Apply failed for "${posting.title}": ${msg}`);
          ctx.logger.error({ err, url: posting.url }, 'autopilot apply step failed');
          emit('autopilot.apply_error', { title: posting.title, message: msg });
        }
      }
    }
  }

  ctx.setProgress(1);
  await store.recordRun(autopilot._id, jobId, run.found, run.applied);
  emit('autopilot.completed', {
    found: run.found,
    matched: run.matched,
    applied: run.applied,
    errors: run.errors.length,
  });
  ctx.logger.info({ ...run }, 'autopilot run completed');
  return run;
}
