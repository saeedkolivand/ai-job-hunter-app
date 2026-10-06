import { Star } from 'lucide-react';

import type { GitHubRepo } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

interface RepoRowProps {
  repo: GitHubRepo;
  checked: boolean;
  onToggle: (name: string) => void;
}

/** One selectable repository row in the import list. */
export function RepoRow({ repo, checked, onToggle }: RepoRowProps) {
  const { t } = useTranslation();
  const checkId = `github-repo-${repo.name}`;
  return (
    <li>
      <label
        htmlFor={checkId}
        className="flex cursor-pointer items-start gap-3 rounded-xl border border-foreground/10 bg-foreground/[0.03] p-3 transition-colors hover:bg-foreground/[0.06] has-[:checked]:border-brand/40 has-[:checked]:bg-brand/5"
      >
        {/* Checkbox — allowed raw input per lint exception */}
        <input
          type="checkbox"
          id={checkId}
          checked={checked}
          onChange={() => onToggle(repo.name)}
          className="mt-0.5 h-4 w-4 shrink-0 accent-[color:var(--color-brand)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand"
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-foreground/85">{repo.name}</span>
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
            <p className="mt-0.5 line-clamp-2 text-xs text-foreground/60">{repo.description}</p>
          )}
        </div>
      </label>
    </li>
  );
}
