import { MC_CONFIG } from './config';
import { ghWrite, type WriteResult } from './github';

// The safe tier of write actions. EVERY action runs behind an explicit confirm
// dialog (see performWriteAction) and requires a signed-in token. There is
// deliberately NO pull-request merge action anywhere in this registry, and none
// is reachable from the UI — the dashboard never merges.

export interface WriteActionContext {
  issueNumber?: number;
  runId?: number;
  comment?: string;
  label?: string;
}

type WriteActionInput = keyof WriteActionContext;

export interface WriteAction {
  id: string;
  label: string;
  needs: readonly WriteActionInput[];
  confirmMessage: (ctx: WriteActionContext) => string;
  method: 'POST' | 'PATCH';
  path: (ctx: WriteActionContext) => string;
  body?: (ctx: WriteActionContext) => unknown;
}

export const WRITE_ACTIONS: readonly WriteAction[] = [
  {
    id: 'rerun-failed',
    label: 'Re-run failed jobs',
    needs: ['runId'],
    confirmMessage: (c) => `Re-run the failed jobs for workflow run #${c.runId}?`,
    method: 'POST',
    path: (c) => `/actions/runs/${c.runId}/rerun-failed-jobs`,
  },
  {
    id: 'dispatch-release',
    label: 'Dispatch the release workflow',
    needs: [],
    confirmMessage: () => `Dispatch ${MC_CONFIG.releaseWorkflow} on main? This can cut a release.`,
    method: 'POST',
    path: () => `/actions/workflows/${MC_CONFIG.releaseWorkflow}/dispatches`,
    body: () => ({ ref: 'main' }),
  },
  {
    id: 'dispatch-pages',
    label: 'Dispatch the pages deploy',
    needs: [],
    confirmMessage: () => `Dispatch ${MC_CONFIG.pagesWorkflow} on main to redeploy the site?`,
    method: 'POST',
    path: () => `/actions/workflows/${MC_CONFIG.pagesWorkflow}/dispatches`,
    body: () => ({ ref: 'main' }),
  },
  {
    id: 'close-issue',
    label: 'Close issue',
    needs: ['issueNumber'],
    confirmMessage: (c) => `Close issue #${c.issueNumber}?`,
    method: 'PATCH',
    path: (c) => `/issues/${c.issueNumber}`,
    body: () => ({ state: 'closed' }),
  },
  {
    id: 'reopen-issue',
    label: 'Reopen issue',
    needs: ['issueNumber'],
    confirmMessage: (c) => `Reopen issue #${c.issueNumber}?`,
    method: 'PATCH',
    path: (c) => `/issues/${c.issueNumber}`,
    body: () => ({ state: 'open' }),
  },
  {
    id: 'label-issue',
    label: 'Add a label',
    needs: ['issueNumber', 'label'],
    confirmMessage: (c) => `Add the label "${c.label}" to issue #${c.issueNumber}?`,
    method: 'POST',
    path: (c) => `/issues/${c.issueNumber}/labels`,
    body: (c) => ({ labels: [c.label] }),
  },
  {
    id: 'comment-issue',
    label: 'Post a comment',
    needs: ['issueNumber', 'comment'],
    confirmMessage: (c) => `Post a comment on issue #${c.issueNumber}?`,
    method: 'POST',
    path: (c) => `/issues/${c.issueNumber}/comments`,
    body: (c) => ({ body: c.comment }),
  },
];

export interface PerformDeps {
  token: string;
  // Resolves true only after the user explicitly confirms in the dialog.
  confirm: (message: string) => Promise<boolean>;
  // Injectable transport (defaults to ghWrite) so tests can assert gating.
  request?: (
    path: string,
    method: 'POST' | 'PATCH',
    body: unknown,
    token: string
  ) => Promise<WriteResult>;
}

export type PerformOutcome = { status: 'cancelled' } | { status: 'done'; result: WriteResult };

// The single choke point for every mutation: confirm FIRST, only then request.
// If the user declines, no network call is ever made.
export async function performWriteAction(
  action: WriteAction,
  ctx: WriteActionContext,
  deps: PerformDeps
): Promise<PerformOutcome> {
  const approved = await deps.confirm(action.confirmMessage(ctx));
  if (!approved) return { status: 'cancelled' };

  const request = deps.request ?? ghWrite;
  const result = await request(action.path(ctx), action.method, action.body?.(ctx), deps.token);
  return { status: 'done', result };
}
