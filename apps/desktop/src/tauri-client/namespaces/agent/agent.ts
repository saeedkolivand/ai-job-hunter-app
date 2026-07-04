import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { type AgentStepEvent, EVENT_CHANNELS } from '@ajh/shared';
import type { AgentRunRequest } from '@ajh/shared/schemas';

import { asyncUnsub } from '../../utils.js';

export const agent = {
  run: (req: AgentRunRequest) => invoke('agent_run', { req }),
  onStep: (handler: (event: AgentStepEvent) => void) =>
    asyncUnsub(() => listen<AgentStepEvent>(EVENT_CHANNELS.agent.step, (e) => handler(e.payload))),
};
