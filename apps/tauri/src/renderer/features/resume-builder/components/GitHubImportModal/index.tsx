import { GitBranch, Star } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';

import type { GitHubRepo } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import {
  Button,
  EmptyState,
  ErrorState,
  Input,
  ModalShell,
  RowSkeleton,
  useNotification,
} from '@ajh/ui';

import { useSelectedModel } from '@/components/ui/ModelSelector';
import { generateGitHubProjects } from '@/lib/generate';
import { useContactProfile } from '@/services/use-contact-profile';
import { useGitHubImport } from '@/services/use-github-import';

/** Extract a bare GitHub username from a full profile URL or return the input as-is. */
function extractGitHubUsername(value: string): string {
  try {
    const url = new URL(value);
    if (url.hostname === 'github.com' || url.hostname === 'www.github.com') {
      const seg = url.pathname.replace(/^\//, '').split('/')[0];
      return seg ?? value;
    }
  } catch {
    // not a URL — treat as bare username
  }
  return value;
}

interface GitHubImportModalProps {
  open: boolean;
  onClose: () => void;
  /** Called with each generated project entry to append to the field array. */
  onAppend: (entry: { name: string; description: string; link: string }) => void;
}

type FetchState = 'idle' | 'fetching' | 'done' | 'error';

const MODAL_TITLE_ID = 'github-import-modal-title';

/**
 * Modal for the resume-builder Projects step: fetch the user's public GitHub
 * repos, multi-select, generate AI bullets, and append to the field array.
 * Ports & Adapters: only touches the service hook — never window.api.
 *
 * ponytail: onToken/StreamingText streaming intentionally omitted — the
 * generator emits a delimited NAME:/DESC: internal format, not user-presentable.
 */
export function GitHubImportModal({ open, onClose, onAppend }: GitHubImportModalProps) {
  const { t } = useTranslation();
  const notify = useNotification();
  const model = useSelectedModel();
  const { data: contact } = useContactProfile();

  // Seed username once from the contact profile when it resolves. A plain
  // `useState(prefill)` traps the user: if prefill arrives after mount the field
  // stays empty, and if it's non-empty the user can't clear the field because
  // `resolvedUsername = username || prefill` keeps snapping back to it.
  // A seededRef avoids reading `username` inside the effect (exhaustive-deps safe).
  const prefill = contact?.github ? extractGitHubUsername(contact.github) : '';
  const [username, setUsername] = useState('');
  const seededRef = useRef(false);
  useEffect(() => {
    if (prefill && !seededRef.current) {
      seededRef.current = true;
      setUsername(prefill);
    }
  }, [prefill]);

  const [repos, setRepos] = useState<GitHubRepo[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [fetchState, setFetchState] = useState<FetchState>('idle');
  const [fetchError, setFetchError] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [generateError, setGenerateError] = useState<string | null>(null);

  const { mutateAsync: importRepos } = useGitHubImport();
  const abortRef = useRef<AbortController | null>(null);
  const inputId = useId();

  const handleFetch = async () => {
    const trimmed = username.trim();
    if (!trimmed) return;
    setFetchState('fetching');
    setFetchError(null);
    setGenerateError(null);
    setRepos([]);
    setSelected(new Set());
    try {
      const result = await importRepos(trimmed);
      setRepos(result);
      setSelected(new Set(result.map((r) => r.name)));
      setFetchState('done');
    } catch (err) {
      setFetchError(err instanceof Error ? err.message : String(err));
      setFetchState('error');
    }
  };

  const toggleRepo = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const allSelected = repos.length > 0 && selected.size === repos.length;

  const toggleAll = () => {
    if (allSelected) {
      setSelected(new Set());
    } else {
      setSelected(new Set(repos.map((r) => r.name)));
    }
  };

  const handleAdd = async () => {
    const chosenRepos = repos.filter((r) => selected.has(r.name));
    if (!chosenRepos.length) return;

    setGenerating(true);
    setGenerateError(null);
    abortRef.current = new AbortController();

    try {
      // generateGitHubProjects self-falls-back (raw description per repo) on
      // provider/offline issues — it only throws on a genuinely unexpected error.
      const generated = await generateGitHubProjects({
        repos: chosenRepos,
        model,
        signal: abortRef.current.signal,
      });
      // Success (includes internal per-repo fallbacks): append + close.
      for (const entry of generated) {
        onAppend(entry);
      }
      onClose();
    } catch {
      // Hard unexpected error: keep modal open, preserve selection, show inline
      // message so the user can retry or close deliberately (do NOT append partial).
      setGenerateError(t('build.extras.projects.github.generateError'));
      notify.error({ message: t('build.extras.projects.github.generateError') });
    } finally {
      setGenerating(false);
      abortRef.current = null;
    }
  };

  const handleClose = () => {
    abortRef.current?.abort();
    onClose();
  };

  const selectedCount = selected.size;

  return (
    <ModalShell
      open={open}
      onClose={handleClose}
      maxWidth="max-w-xl"
      ariaLabelledby={MODAL_TITLE_ID}
      header={
        <div className="flex items-start gap-2 border-b border-foreground/10 px-6 py-5">
          <GitBranch size={16} className="mt-0.5 shrink-0 text-brand-soft" aria-hidden={true} />
          <div className="flex flex-col gap-1">
            <span id={MODAL_TITLE_ID} className="text-sm font-semibold text-foreground/85">
              {t('build.extras.projects.github.modalTitle')}
            </span>
            <p className="text-xs text-foreground/60">
              {t('build.extras.projects.github.modalDescription')}
            </p>
          </div>
        </div>
      }
      footer={
        <div className="flex items-center justify-between gap-2 border-t border-foreground/10 px-6 py-4">
          <Button variant="ghost" onClick={handleClose} disabled={generating}>
            {t('build.extras.projects.github.cancel')}
          </Button>
          <Button
            variant="primary"
            onClick={() => void handleAdd()}
            disabled={selectedCount === 0 || generating || fetchState !== 'done'}
          >
            {generating
              ? t('build.extras.projects.github.generating')
              : t('build.extras.projects.github.addSelected', { count: selectedCount })}
          </Button>
        </div>
      }
    >
      {/* Persistent live regions — always in the DOM so the browser's AT
          buffer is ready before the state transitions fire. */}
      <p className="sr-only" role="status" aria-live="polite" aria-atomic={true}>
        {fetchState === 'fetching' ? t('build.extras.projects.github.loading') : ''}
      </p>
      <p className="sr-only" role="status" aria-live="polite" aria-atomic={true}>
        {generating ? t('build.extras.projects.github.generating') : ''}
      </p>

      <div className="flex flex-col gap-4 px-6 py-5">
        {/* Username input + fetch */}
        <div className="flex gap-2">
          <label htmlFor={inputId} className="sr-only">
            {t('build.extras.projects.github.usernamePlaceholder')}
          </label>
          <Input
            id={inputId}
            className="flex-1"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void handleFetch();
            }}
            placeholder={t('build.extras.projects.github.usernamePlaceholder')}
            disabled={fetchState === 'fetching' || generating}
          />
          <Button
            type="button"
            onClick={() => void handleFetch()}
            disabled={!username.trim() || fetchState === 'fetching' || generating}
          >
            {t('build.extras.projects.github.fetchButton')}
          </Button>
        </div>

        {/* Fetch loading */}
        {fetchState === 'fetching' && (
          <div className="space-y-2">
            <RowSkeleton />
            <RowSkeleton />
            <RowSkeleton />
          </div>
        )}

        {/* Fetch error */}
        {fetchState === 'error' && fetchError && <ErrorState title={fetchError} />}

        {/* Generation error — inline, keeps modal open */}
        {generateError && (
          <p className="rounded-lg bg-red-500/10 px-3 py-2 text-xs text-red-400" role="alert">
            {generateError}
          </p>
        )}

        {/* Empty */}
        {fetchState === 'done' && repos.length === 0 && (
          <EmptyState icon={GitBranch} title={t('build.extras.projects.github.noRepos')} />
        )}

        {/* Repo list */}
        {fetchState === 'done' && repos.length > 0 && (
          <div className="flex flex-col gap-3">
            {/* Select-all toggle + repo count */}
            <div className="flex items-center justify-between">
              <span className="text-fine-print text-foreground/60">
                {t('build.extras.projects.github.repoCount', { count: repos.length })}
              </span>
              <Button
                type="button"
                variant="ghost"
                className="h-auto p-0 text-xs text-brand"
                onClick={toggleAll}
              >
                {allSelected
                  ? t('build.extras.projects.github.deselectAll')
                  : t('build.extras.projects.github.selectAll')}
              </Button>
            </div>

            {/* Repo rows */}
            <ul
              className="flex max-h-72 flex-col gap-2 overflow-y-auto"
              role="list"
              aria-label={t('build.extras.projects.github.modalTitle')}
            >
              {repos.map((repo) => {
                const isChecked = selected.has(repo.name);
                const checkId = `github-repo-${repo.name}`;
                return (
                  <li key={repo.name}>
                    <label
                      htmlFor={checkId}
                      className="flex cursor-pointer items-start gap-3 rounded-xl border border-foreground/10 bg-foreground/[0.03] p-3 transition-colors hover:bg-foreground/[0.06] has-[:checked]:border-brand/40 has-[:checked]:bg-brand/5"
                    >
                      {/* Checkbox — allowed raw input per lint exception */}
                      <input
                        type="checkbox"
                        id={checkId}
                        checked={isChecked}
                        onChange={() => toggleRepo(repo.name)}
                        className="mt-0.5 h-4 w-4 shrink-0 accent-[color:var(--color-brand)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
                      />
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="truncate text-sm font-semibold text-foreground/85">
                            {repo.name}
                          </span>
                          {repo.language && (
                            <span className="shrink-0 rounded-full bg-foreground/[0.08] px-1.5 py-0.5 text-fine-print text-foreground/60">
                              {repo.language}
                            </span>
                          )}
                          <span className="ml-auto flex shrink-0 items-center gap-0.5 text-fine-print text-foreground/60">
                            <Star size={11} aria-hidden={true} />
                            {t('build.extras.projects.github.stars', { count: repo.stars })}
                          </span>
                        </div>
                        {repo.description && (
                          <p className="mt-0.5 line-clamp-2 text-xs text-foreground/60">
                            {repo.description}
                          </p>
                        )}
                      </div>
                    </label>
                  </li>
                );
              })}
            </ul>
          </div>
        )}
      </div>
    </ModalShell>
  );
}
