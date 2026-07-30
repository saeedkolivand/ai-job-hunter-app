import {
  DIVIDER_TOGETHER,
  SAVE_JOB_PROMPT_COPY,
  SAVE_JOB_PROMPT_TEXT,
  WORK_TOGETHER_TEAMS,
} from '@/data/agent-fleet';
import { copyChip, copyChipOnKeyDown } from '@/lib/agent-system/clipboard';

import { Divider } from '../Divider';

export function TogetherSection() {
  return (
    <>
      <p className="scrawl reveal">all at once →</p>
      <h2 className="section reveal">Make Them Work Together</h2>
      <p className="section-sub reveal">one cross-layer ask, the whole fleet in parallel.</p>

      <div className="big-prompt reveal">
        <span
          className="copy-cmd"
          role="button"
          tabIndex={0}
          title="click to copy"
          data-copy={SAVE_JOB_PROMPT_COPY}
          onClick={copyChip}
          onKeyDown={copyChipOnKeyDown}
        >
          {SAVE_JOB_PROMPT_TEXT}
        </span>
        <div className="teams">
          {WORK_TOGETHER_TEAMS.map((team) => (
            <div className="team" key={team.title}>
              <h4>{team.title}</h4>
              <ul>
                {team.items.map((segments, i) => (
                  <li key={i}>
                    {segments.map((seg, j) =>
                      'code' in seg ? <code key={j}>{seg.code}</code> : seg.text
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </div>

      <Divider d={DIVIDER_TOGETHER} />
    </>
  );
}
