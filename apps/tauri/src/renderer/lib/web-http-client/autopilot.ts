import { createHttpClientHelpers, type WebHttpClientOptions } from './utils.js';

export function autopilot(opts: WebHttpClientOptions) {
  const { cmd } = createHttpClientHelpers(opts);
  return {
    list: () => cmd('autopilot', 'list'),
    get: ({ autopilotId }: { autopilotId: string }) => cmd('autopilot', 'get', { autopilotId }),
    create: (req: unknown) => cmd('autopilot', 'create', req),
    update: ({ autopilotId, ...data }: { autopilotId: string } & Record<string, unknown>) =>
      cmd('autopilot', 'update', { autopilotId, ...data }),
    remove: ({ autopilotId }: { autopilotId: string }) =>
      cmd('autopilot', 'remove', { autopilotId }),
    run: ({ autopilotId }: { autopilotId: string }) => cmd('autopilot', 'run', { autopilotId }),
    pause: ({ autopilotId }: { autopilotId: string }) => cmd('autopilot', 'pause', { autopilotId }),
    resume: ({ autopilotId }: { autopilotId: string }) =>
      cmd('autopilot', 'resume', { autopilotId }),
  };
}
