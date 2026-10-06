import { GitBranch } from 'lucide-react';
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

import { extractGitHubUsername } from './extractGitHubUsername';
import { RepoRow } from './RepoRow';

interface GitHubImportModalProps {
  open: boolean;
  onClose: () => void;
  /** Called with each generated project entry to append to the field array. */
  onAppend: (entry: {
    name: string;
    description: string;
    link: string;
    technologies: string;
  }) => void;
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

  const prefill = contact?.github ? extractGitHubUsername(contact.github) : '';

  const [username, setUsername] = useState('');
  const [repos, setRepos] = useState<GitHubRepo[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [fetchState, setFetchState] = useState<FetchState>('idle');
  const [fetchError, setFetchError] = useState<string | null>(null);
  const [generating, setGenerating] = useState(false);
  const [generateError, setGenerateError] = useState<string | null>(null);

  // Effect 1: reset transient fetch/generation state ONLY on open transition.
  // Keyed on [open] alone so an async prefill update mid-session never wipes
  // a fetched repo list or in-progress selection.
  const seededRef = useRef(false);
  useEffect(() => {
    if (!open) {
      // Modal closed — arm the seed so the next open re-seeds from profile.
      seededRef.current = false;
      return;
    }
    setRepos([]);
    setSelected(new Set());
    setFetchState('idle');
    setFetchError(null);
    setGenerateError(null);
  }, [open]);

  // Effect 2: seed username from the contact profile exactly once per open-cycle,
  // and ONLY while the field is still empty (user hasn't typed yet).
  // Keyed on [open, prefill] so a late-arriving profile fires the effect, but the
  // seededRef + empty-field guard means it never clobbers a user-typed value.
  useEffect(() => {
    if (open && prefill && !seededRef.current) {
      seededRef.current = true;
      // setUsername with a functional updater: only apply if the current value is
      // still empty — if the user already typed something, leave it alone.
      setUsername((cur) => (cur === '' ? prefill : cur));
    }
  }, [open, prefill]);

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
    const controller = new AbortController();
    abortRef.current = controller;

    try {
      // generateGitHubProjects self-falls-back (raw description per repo) on
      // provider/offline issues — it only throws on a genuinely unexpected error.
      const generated = await generateGitHubProjects({
        repos: chosenRepos,
        model,
        signal: controller.signal,
      });
      // Guard: if the user cancelled mid-generation, handleClose already called
      // onClose() and aborted the controller. generateGitHubProjects resolves with
      // a populated fallback array even on abort (it does NOT throw), so without
      // this guard we would silently append all selected repos and double-close.
      if (controller.signal.aborted) return;
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
              {repos.map((repo) => (
                <RepoRow
                  key={repo.name}
                  repo={repo}
                  checked={selected.has(repo.name)}
                  onToggle={toggleRepo}
                />
              ))}
            </ul>
          </div>
        )}
      </div>
    </ModalShell>
  );
}
