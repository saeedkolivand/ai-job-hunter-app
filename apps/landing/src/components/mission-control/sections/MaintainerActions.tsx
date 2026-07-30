import type { Model } from '@/lib/mission-control/model';
import {
  actionById,
  type WriteAction,
  type WriteActionContext,
} from '@/lib/mission-control/write-actions';

import { Section } from '../Section';

export function MaintainerActions({
  failedGatingRun,
  runAction,
}: {
  failedGatingRun: Model['work']['failedGatingRun'];
  runAction: (action: WriteAction, ctx: WriteActionContext, danger?: boolean) => void;
}) {
  return (
    <Section
      label="Maintainer actions"
      eyebrow="every one asks first, none of them merge"
      title="Maintainer actions"
    >
      <div className="mc-maintainer">
        <button
          type="button"
          className="mc-btn is-primary"
          onClick={() => void runAction(actionById('dispatch-release'), {}, true)}
        >
          Dispatch release workflow
        </button>
        <button
          type="button"
          className="mc-btn is-danger"
          onClick={() => void runAction(actionById('dispatch-pages'), {}, true)}
        >
          Dispatch pages deploy
        </button>
        {failedGatingRun ? (
          <button
            type="button"
            className="mc-btn is-danger"
            onClick={() =>
              void runAction(actionById('rerun-failed'), {
                runId: failedGatingRun?.id,
              })
            }
          >
            Re-run failed CI (#{failedGatingRun.id})
          </button>
        ) : null}
      </div>
    </Section>
  );
}
